//! The AT scenario model and runner.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use truefix_core::{Field, Message, decode, frame_length};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::AcceptorBuilder;

/// A message the scenario expects the server to send: a MsgType plus required field matches.
/// Volatile fields (SendingTime, BodyLength, CheckSum) are intentionally not matched.
#[derive(Debug, Clone)]
pub struct ExpectMsg {
    /// Expected MsgType (tag 35).
    pub msg_type: String,
    /// `(tag, value)` pairs that must be present (in header or body) with the given value.
    pub fields: Vec<(u32, String)>,
    /// Tags that must be absent entirely (T015, feature 007 — e.g. confirming a Logon response
    /// does NOT echo `ResetSeqNumFlag` when the inbound Logon never carried it).
    pub fields_absent: Vec<u32>,
    /// T136/T137 (feature 009, NEW-50): when set, `check_match` additionally fails if the actual
    /// message carries any tag beyond `fields` and [`ALWAYS_ALLOWED_TAGS`] — i.e. an unexpected
    /// extra field. Opt-in (default `false`, via [`ExpectMsg::exact`]) rather than the default for
    /// every `ExpectMsg`: most of this suite's ~90 existing scenarios only assert the handful of
    /// fields they actually care about, deliberately leaving every other legitimate field (e.g. a
    /// Logon response's own `EncryptMethod`/`HeartBtInt`) unlisted — checking exhaustively by
    /// default would require rewriting all of them, not just this one item's fix.
    pub exact: bool,
}

/// Header/trailer bookkeeping and session-identity fields present on essentially every FIX
/// message, which [`ExpectMsg::exact`] never flags as "extra" even when unlisted — matches the
/// doc comment above `ExpectMsg` calling out `SendingTime`/`BodyLength`/`CheckSum` as always
/// unmatched, plus the other framing/identity fields every message legitimately carries.
pub const ALWAYS_ALLOWED_TAGS: &[u32] = &[
    8,   // BeginString
    9,   // BodyLength
    35,  // MsgType
    34,  // MsgSeqNum
    49,  // SenderCompID
    56,  // TargetCompID
    52,  // SendingTime
    43,  // PossDupFlag
    122, // OrigSendingTime
    10,  // CheckSum
];

impl ExpectMsg {
    /// Expect a message of `msg_type` with no specific field requirements.
    pub fn of(msg_type: &str) -> Self {
        Self {
            msg_type: msg_type.to_owned(),
            fields: Vec::new(),
            fields_absent: Vec::new(),
            exact: false,
        }
    }

    /// Require a field value.
    #[must_use]
    pub fn field(mut self, tag: u32, value: &str) -> Self {
        self.fields.push((tag, value.to_owned()));
        self
    }

    /// Require a tag to be entirely absent from the message.
    #[must_use]
    pub fn without_field(mut self, tag: u32) -> Self {
        self.fields_absent.push(tag);
        self
    }

    /// Additionally fail if the actual message carries any tag beyond the ones already required
    /// via [`ExpectMsg::field`] and [`ALWAYS_ALLOWED_TAGS`] (T136/T137, NEW-50).
    #[must_use]
    pub fn exact(mut self) -> Self {
        self.exact = true;
        self
    }
}

/// A single scenario step.
#[derive(Debug, Clone)]
pub enum Step {
    /// Send a message to the server.
    Send(Message),
    /// Send raw bytes to the server (e.g. a deliberately garbled frame).
    SendRaw(Vec<u8>),
    /// Expect a matching message from the server.
    Expect(ExpectMsg),
    /// Expect the server to disconnect (no further message).
    ExpectDisconnect,
}

/// Optional acceptor session-feature toggles a scenario needs (special-category suites).
#[derive(Debug, Clone, Default)]
pub struct SessionTweaks {
    /// Enable NextExpectedMsgSeqNum (789) handling.
    pub enable_next_expected: bool,
    /// Enable LastMsgSeqNumProcessed (369) stamping.
    pub enable_last_processed: bool,
    /// Enable CheckLatency (stale SendingTime aborts the session).
    pub check_latency: bool,
    /// ResendRequest chunk size (0 = request the whole range at once).
    pub resend_chunk_size: u32,
    /// Use an active application that replies to each NewOrderSingle with an ExecutionReport.
    pub executor_app: bool,
    /// Enable RejectGarbledMessage (a garbled frame draws a session Reject instead of a silent drop).
    pub reject_garbled: bool,
    /// Enable `ValidateFieldsOutOfOrder` on the acceptor's dictionary validator (FR-006).
    pub validate_fields_out_of_order: bool,
    /// Disconnect (in addition to rejecting) on an inbound dictionary-validation failure
    /// (006/US1, B5/FR-008).
    pub disconnect_on_error: bool,
    /// T133/T134/T135 (feature 009, NEW-83): when set, `run_report` starts this scenario's
    /// acceptor via [`start_fixed_identity_acceptor`] (fixed `SenderCompID`/`TargetCompID`,
    /// `(server, client)`) instead of [`start_acceptor`]'s dynamic-template mode — required to
    /// exercise an inbound Logon whose identity doesn't match a pre-existing session at all,
    /// which the dynamic template has nothing to "mismatch" against.
    pub fixed_identity: Option<(String, String)>,
}

/// A scripted acceptance-test scenario.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// Scenario name (matches the QuickFIX AT scenario it reproduces).
    pub name: String,
    /// FIX versions this scenario targets.
    pub versions: Vec<String>,
    /// The scripted steps.
    pub steps: Vec<Step>,
    /// Acceptor session-feature toggles (default: all off, CheckLatency off).
    pub tweaks: SessionTweaks,
}

/// The result of running one scenario against one version.
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    /// Scenario name.
    pub name: String,
    /// FIX version.
    pub version: String,
    /// `Ok` on pass, `Err(reason)` on failure.
    pub outcome: Result<(), String>,
}

/// A one-line-per-run PASS/FAIL report (T139/T140, feature 009, NEW-52) — distinct from a single
/// all-or-nothing boolean: a caller (or a human reading test output) can see every scenario's own
/// result, not just whether *something* in the whole suite failed, without re-running anything.
#[must_use]
pub fn per_scenario_report(results: &[ScenarioResult]) -> String {
    results
        .iter()
        .map(|r| match &r.outcome {
            Ok(()) => format!("PASS  {} [{}]", r.name, r.version),
            Err(reason) => format!("FAIL  {} [{}]: {reason}", r.name, r.version),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The bundled dictionary matching a `SUITE_VERSIONS` entry, for the acceptor's field validator
/// (T128/T129, feature 009) — previously only `FIX.4.2`/`FIX.4.4` had one, so a field-validation
/// scenario against any other pre-FIXT version silently ran with no dictionary checks at all.
///
/// Deliberately `None` for `FIX.5.0`/`SP1`/`SP2`/`FIX.Latest`: those versions split admin messages
/// (Logon, Heartbeat, ...) into the separate FIXT 1.1 transport dictionary, so the bundled
/// `FIX50*`/`FIXLATEST` sources are application-message-only. `Services::validator` applies its one
/// dictionary to *every* inbound message, admin included (`Session::validate_app`'s flat-dictionary
/// branch, unlike its `fixt_validator`/`FixtDictionaries` branch, has no admin/app split) — handing
/// it an app-only dictionary would make a plain Logon fail as an unregistered MsgType, breaking
/// every scenario for that version. A merge attempt (`DataDictionary::extend`, folding FIXT11's
/// admin definitions into the app dict) was tried and rejected: tag 35 (MsgType)'s enum differs
/// between the transport and application dictionaries, so `extend` correctly reports a field
/// conflict rather than silently picking one side. Properly supporting these four versions needs
/// `start_acceptor` to wire a real `FixtDictionaries` dual-dictionary instead of the flat
/// `validator` field — the same "substantial harness-level work" already deferred at
/// T019/T022/T029/T054/T159, not attempted here.
/// The `SUITE_VERSIONS` subset [`dictionary_for_version`] returns `Some` for.
pub const FLAT_DICTIONARY_VERSIONS: &[&str] =
    &["FIX.4.0", "FIX.4.1", "FIX.4.2", "FIX.4.3", "FIX.4.4"];

pub fn dictionary_for_version(version: &str) -> Option<truefix_dict::DataDictionary> {
    match version {
        "FIX.4.0" => truefix_dict::load_fix40().ok(),
        "FIX.4.1" => truefix_dict::load_fix41().ok(),
        "FIX.4.2" => truefix_dict::load_fix42().ok(),
        "FIX.4.3" => truefix_dict::load_fix43().ok(),
        "FIX.4.4" => truefix_dict::load_fix44().ok(),
        _ => None,
    }
}

/// Start a black-box acceptor that serves any session via a dynamic template for `version`.
/// Returns the bound address and the accept-loop join handle.
pub async fn start_acceptor(
    version: &str,
    tweaks: &SessionTweaks,
) -> std::io::Result<(SocketAddr, JoinHandle<()>)> {
    // One app type: when it holds a Monitor it acts as an executor, replying to each
    // NewOrderSingle(35=D) with an ExecutionReport(35=8); otherwise it is passive.
    struct AtApp {
        monitor: Option<truefix_transport::Monitor>,
    }
    #[async_trait::async_trait]
    impl Application for AtApp {
        async fn on_logon(&self, _s: &SessionId) {}
        async fn from_app(
            &self,
            message: &Message,
            id: &SessionId,
        ) -> Result<(), truefix_core::BusinessReject> {
            if let Some(monitor) = &self.monitor
                && message.msg_type() == Some("D")
            {
                // A sentinel ClOrdID lets a scenario trigger an acceptor-initiated logout.
                let clordid = message.body.get(11).and_then(|f| f.as_str().ok());
                if clordid == Some("LOGOUT") {
                    monitor.force_logout(id).await;
                } else {
                    monitor.send_app(id, execution_report(message)).await;
                }
            }
            Ok(())
        }
        // T021 (US3, feature 005, GAP-07/FR-007): a sentinel ClOrdID ("VETO-RESEND", carried over
        // from the originating NewOrderSingle onto its ExecutionReport by `execution_report`'s
        // existing field-copy) lets a scenario prove a resend-originated (PossDupFlag=Y) send gets
        // vetoed and replaced by a GapFill — unconditional, not gated by a tweak, matching the
        // existing "LOGOUT" sentinel's pattern above. The *original* live send is never vetoed
        // (only its later resend is), matching `resend_veto.rs`'s transport-level test.
        async fn to_app(
            &self,
            message: &mut Message,
            _id: &SessionId,
        ) -> Result<(), truefix_core::DoNotSend> {
            let is_veto_sentinel =
                message.body.get(11).and_then(|f| f.as_str().ok()) == Some("VETO-RESEND");
            let is_resend = message.header.get(43).and_then(|f| f.as_str().ok()) == Some("Y");
            if is_veto_sentinel && is_resend {
                Err(truefix_core::DoNotSend)
            } else {
                Ok(())
            }
        }
    }

    let mut template = SessionConfig::new(
        wire_begin_string(version),
        "SERVER",
        "CLIENT",
        Role::Acceptor,
    );
    template.heartbeat_interval = 30;
    // Scenarios use fixed timestamps, so CheckLatency is off unless a scenario opts in.
    template.check_latency = tweaks.check_latency;
    template.enable_next_expected_msg_seq_num = tweaks.enable_next_expected;
    template.enable_last_msg_seq_num_processed = tweaks.enable_last_processed;
    template.resend_request_chunk_size = tweaks.resend_chunk_size;
    template.reject_garbled_message = tweaks.reject_garbled;
    template.disconnect_on_error = tweaks.disconnect_on_error;

    // Enable dictionary validation so field-level reject scenarios produce Reject messages.
    let validation_opts = truefix_dict::ValidationOptions {
        validate_fields_out_of_order: tweaks.validate_fields_out_of_order,
        ..truefix_dict::ValidationOptions::default()
    };
    let validator = dictionary_for_version(version).map(|dict| (dict, validation_opts));
    let monitor = tweaks.executor_app.then(truefix_transport::Monitor::new);
    let services = truefix_transport::Services {
        validator,
        monitor: monitor.clone(),
        ..truefix_transport::Services::default()
    };

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap_or_else(|_| unreachable_addr()),
        Arc::new(AtApp { monitor }),
    )
    .await?
    .with_dynamic_template(template)
    .with_services(services);
    let addr = acceptor.local_addr()?;
    let handle = acceptor.serve();
    Ok((addr, handle))
}

/// Start a black-box acceptor with a fixed session identity (T133/T134, feature 009, NEW-83) —
/// additive alongside [`start_acceptor`]'s dynamic-template mode, which adopts whatever identity
/// the first Logon claims and so has nothing to "mismatch" against (see the note above
/// `logon_response_carries_reset_flag` on why `1c_InvalidSenderCompID`/`1c_InvalidTargetCompID`/a
/// wrong-`BeginString` Logon couldn't be represented until now). A Logon whose SenderCompID/
/// TargetCompID/BeginString doesn't match `sender`/`target`/`version` never resolves to a
/// registered session (`route_and_run`'s `registry.sessions.get(&sid)` lookup, no template
/// fallback here) — the connection is simply dropped, not answered with a Reject/Logout.
pub async fn start_fixed_identity_acceptor(
    version: &str,
    sender: &str,
    target: &str,
    tweaks: &SessionTweaks,
) -> std::io::Result<(SocketAddr, JoinHandle<()>)> {
    struct FixedIdentityApp;
    #[async_trait::async_trait]
    impl Application for FixedIdentityApp {
        async fn on_logon(&self, _s: &SessionId) {}
    }

    let mut config = SessionConfig::new(wire_begin_string(version), sender, target, Role::Acceptor);
    config.heartbeat_interval = 30;
    config.check_latency = tweaks.check_latency;
    config.enable_next_expected_msg_seq_num = tweaks.enable_next_expected;
    config.enable_last_msg_seq_num_processed = tweaks.enable_last_processed;
    config.resend_request_chunk_size = tweaks.resend_chunk_size;
    config.reject_garbled_message = tweaks.reject_garbled;
    config.disconnect_on_error = tweaks.disconnect_on_error;

    let validation_opts = truefix_dict::ValidationOptions {
        validate_fields_out_of_order: tweaks.validate_fields_out_of_order,
        ..truefix_dict::ValidationOptions::default()
    };
    let validator = dictionary_for_version(version).map(|dict| (dict, validation_opts));
    let services = truefix_transport::Services {
        validator,
        ..truefix_transport::Services::default()
    };

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap_or_else(|_| unreachable_addr()),
        Arc::new(FixedIdentityApp),
    )
    .await?
    .with_session(config)
    .with_services(services);
    let addr = acceptor.local_addr()?;
    let handle = acceptor.serve();
    Ok((addr, handle))
}

/// Build a minimal ExecutionReport (35=8) acknowledging `order`, echoing its key fields. The
/// engine stamps the session header (BeginString/CompIDs/MsgSeqNum/SendingTime) on send.
fn execution_report(order: &Message) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "8"));
    m.body.set(Field::string(37, "ORDER-1")); // OrderID
    m.body.set(Field::string(17, "EXEC-1")); // ExecID
    m.body.set(Field::string(150, "0")); // ExecType = New
    m.body.set(Field::string(39, "0")); // OrdStatus = New
    for tag in [11u32, 55, 54, 38] {
        if let Some(f) = order.body.get(tag) {
            m.body.set(Field::new(tag, f.value_bytes().to_vec()));
        }
    }
    m
}

fn unreachable_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 0))
}

/// Run one scenario against an already-started acceptor at `addr`.
pub async fn run_scenario(scenario: &Scenario, addr: SocketAddr) -> Result<(), String> {
    let mut stream = TcpStream::connect(addr)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let mut buf: Vec<u8> = Vec::new();

    for (i, step) in scenario.steps.iter().enumerate() {
        match step {
            Step::Send(msg) => {
                stream
                    .write_all(&msg.encode())
                    .await
                    .map_err(|e| format!("step {i}: send: {e}"))?;
            }
            Step::SendRaw(bytes) => {
                stream
                    .write_all(bytes)
                    .await
                    .map_err(|e| format!("step {i}: send raw: {e}"))?;
            }
            Step::Expect(expect) => {
                // 3s tolerates the 1s tick granularity used by timer-driven scenarios.
                let msg = match read_message(&mut stream, &mut buf, Duration::from_secs(3)).await {
                    ReadMessageOutcome::Message(msg) => msg,
                    ReadMessageOutcome::TimedOut => {
                        return Err(format!(
                            "step {i}: expected {} but timed out",
                            expect.msg_type
                        ));
                    }
                    ReadMessageOutcome::DecodeFailed(error) => {
                        return Err(format!(
                            "step {i}: expected {} but got an undecodable message: {error}",
                            expect.msg_type
                        ));
                    }
                    ReadMessageOutcome::CleanEof => {
                        return Err(format!(
                            "step {i}: expected {} but the peer disconnected",
                            expect.msg_type
                        ));
                    }
                    ReadMessageOutcome::ReadFailed(error) => {
                        return Err(format!("step {i}: read failed: {error}"));
                    }
                };
                check_match(&msg, expect).map_err(|e| format!("step {i}: {e}"))?;
            }
            Step::ExpectDisconnect => {
                match read_message(&mut stream, &mut buf, Duration::from_secs(3)).await {
                    ReadMessageOutcome::CleanEof => {}
                    ReadMessageOutcome::Message(_) => {
                        return Err(format!("step {i}: expected disconnect but got a message"));
                    }
                    ReadMessageOutcome::TimedOut => {
                        return Err(format!("step {i}: expected disconnect but timed out"));
                    }
                    ReadMessageOutcome::DecodeFailed(error) => {
                        return Err(format!(
                            "step {i}: expected disconnect but got an undecodable message: {error}"
                        ));
                    }
                    ReadMessageOutcome::ReadFailed(error) => {
                        return Err(format!(
                            "step {i}: expected disconnect but read failed: {error}"
                        ));
                    }
                }
            }
        }
    }
    // T131/T132 (feature 009, NEW-30): a scenario that only checks its own scripted steps can
    // silently pass even when the server sent something extra it never asked about (e.g. a
    // spurious duplicate response) — a short grace wait for anything left over (already-buffered
    // or arriving shortly after the last step) catches that instead of leaving it unnoticed.
    match read_message(&mut stream, &mut buf, Duration::from_millis(25)).await {
        ReadMessageOutcome::TimedOut | ReadMessageOutcome::CleanEof => Ok(()),
        ReadMessageOutcome::Message(msg) => Err(format!(
            "scenario complete but an extra, unrequested message arrived: {msg:?}"
        )),
        ReadMessageOutcome::DecodeFailed(error) => Err(format!(
            "scenario complete but extra, undecodable bytes arrived: {error}"
        )),
        ReadMessageOutcome::ReadFailed(error) => Err(format!(
            "scenario complete but the trailing read failed: {error}"
        )),
    }
}

/// Run the full matrix (each scenario against each of its target versions), starting a fresh
/// acceptor per version. Returns one [`ScenarioResult`] per (scenario, version).
pub async fn run_report(scenarios: &[Scenario]) -> Vec<ScenarioResult> {
    let mut results = Vec::new();
    // A fresh acceptor per (scenario, version) isolates session state and lets each scenario
    // request its own acceptor feature toggles.
    for s in scenarios {
        for version in &s.versions {
            let started = match &s.tweaks.fixed_identity {
                Some((sender, target)) => {
                    start_fixed_identity_acceptor(version, sender, target, &s.tweaks).await
                }
                None => start_acceptor(version, &s.tweaks).await,
            };
            let outcome = match started {
                Ok((addr, handle)) => {
                    let outcome = run_scenario(s, addr).await;
                    handle.abort();
                    outcome
                }
                Err(e) => Err(format!("could not start acceptor: {e}")),
            };
            results.push(ScenarioResult {
                name: s.name.clone(),
                version: version.clone(),
                outcome,
            });
        }
    }
    results
}

fn check_match(msg: &Message, expect: &ExpectMsg) -> Result<(), String> {
    if msg.msg_type() != Some(expect.msg_type.as_str()) {
        return Err(format!(
            "expected MsgType {:?}, got {:?}",
            expect.msg_type,
            msg.msg_type()
        ));
    }
    for (tag, want) in &expect.fields {
        let got = field_value(msg, *tag);
        if got.as_deref() != Some(want.as_str()) {
            return Err(format!("tag {tag}: expected {want:?}, got {got:?}"));
        }
    }
    for tag in &expect.fields_absent {
        if let Some(got) = field_value(msg, *tag) {
            return Err(format!("tag {tag}: expected absent, got {got:?}"));
        }
    }
    if expect.exact {
        let allowed = |tag: u32| {
            ALWAYS_ALLOWED_TAGS.contains(&tag) || expect.fields.iter().any(|(t, _)| *t == tag)
        };
        for field in msg
            .header
            .fields()
            .chain(msg.body.fields())
            .chain(msg.trailer.fields())
        {
            if !allowed(field.tag()) {
                return Err(format!(
                    "unexpected extra tag {}: {:?}",
                    field.tag(),
                    field.as_str().ok()
                ));
            }
        }
    }
    Ok(())
}

fn field_value(msg: &Message, tag: u32) -> Option<String> {
    let field = msg
        .header
        .get(tag)
        .or_else(|| msg.body.get(tag))
        .or_else(|| msg.trailer.get(tag))?;
    field.as_str().ok().map(str::to_owned)
}

/// Result of reading one message in the acceptance-test harness.
#[derive(Debug)]
pub enum ReadMessageOutcome {
    /// A complete, decoded FIX message.
    Message(Message),
    /// No bytes arrived before the requested wait elapsed.
    TimedOut,
    /// A complete frame arrived but failed FIX decoding.
    DecodeFailed(truefix_core::DecodeError),
    /// The peer cleanly closed the stream.
    CleanEof,
    /// The socket read itself failed.
    ReadFailed(std::io::Error),
}

/// Read one framed message from `stream`, retaining incomplete bytes in `buf`.
pub async fn read_message(
    stream: &mut TcpStream,
    buf: &mut Vec<u8>,
    wait: Duration,
) -> ReadMessageOutcome {
    loop {
        if let Ok(Some(total)) = frame_length(buf) {
            let raw: Vec<u8> = buf.drain(..total).collect();
            return match decode(&raw) {
                Ok(message) => ReadMessageOutcome::Message(message),
                Err(error) => ReadMessageOutcome::DecodeFailed(error),
            };
        }
        let mut chunk = [0u8; 4096];
        match timeout(wait, stream.read(&mut chunk)).await {
            Ok(Ok(0)) => return ReadMessageOutcome::CleanEof,
            Ok(Err(error)) => return ReadMessageOutcome::ReadFailed(error),
            Err(_) => return ReadMessageOutcome::TimedOut,
            Ok(Ok(n)) => {
                if let Some(slice) = chunk.get(..n) {
                    buf.extend_from_slice(slice);
                }
            }
        }
    }
}

/// `SUITE_VERSIONS`' `"FIX.Latest"` entry is a version-agnostic-testing label, not a real wire
/// `BeginString` (feature 007, BUG-79/FR-048's stricter `frame_length` format check would
/// otherwise reject it outright, unlike every real value in `SUITE_VERSIONS`) — this maps it to
/// a real, well-formed `BeginString` for anywhere actual wire bytes get constructed, leaving every
/// other version untouched. The bundled `FIXLATEST` dictionary's own `.version` field is a
/// separate, unrelated label (only ever compared via `version_meta`, which no bundled dictionary
/// populates) — not affected by this translation.
pub fn wire_begin_string(version: &str) -> &str {
    if version == "FIX.Latest" {
        "FIX.5.0SP2"
    } else {
        version
    }
}

/// Helper to build a client-side message with the standard header.
pub fn client_message(version: &str, msg_type: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, wire_begin_string(version)));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}
