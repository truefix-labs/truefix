//! The AT scenario model and runner.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use truefix_core::{decode, frame_length, Field, Message};
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
}

impl ExpectMsg {
    /// Expect a message of `msg_type` with no specific field requirements.
    pub fn of(msg_type: &str) -> Self {
        Self {
            msg_type: msg_type.to_owned(),
            fields: Vec::new(),
        }
    }

    /// Require a field value.
    #[must_use]
    pub fn field(mut self, tag: u32, value: &str) -> Self {
        self.fields.push((tag, value.to_owned()));
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
            if let Some(monitor) = &self.monitor {
                if message.msg_type() == Some("D") {
                    // A sentinel ClOrdID lets a scenario trigger an acceptor-initiated logout.
                    let clordid = message.body.get(11).and_then(|f| f.as_str().ok());
                    if clordid == Some("LOGOUT") {
                        monitor.force_logout(id).await;
                    } else {
                        monitor.send_app(id, execution_report(message)).await;
                    }
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

    let mut template = SessionConfig::new(version, "SERVER", "CLIENT", Role::Acceptor);
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
    let validator = match version {
        "FIX.4.2" => truefix_dict::load_fix42().ok(),
        "FIX.4.4" => truefix_dict::load_fix44().ok(),
        _ => None,
    }
    .map(|dict| (dict, validation_opts));
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
                let msg = read_message(&mut stream, &mut buf, Duration::from_secs(3))
                    .await
                    .ok_or_else(|| {
                        format!("step {i}: expected {} but got nothing", expect.msg_type)
                    })?;
                check_match(&msg, expect).map_err(|e| format!("step {i}: {e}"))?;
            }
            Step::ExpectDisconnect => {
                if read_message(&mut stream, &mut buf, Duration::from_secs(3))
                    .await
                    .is_some()
                {
                    return Err(format!("step {i}: expected disconnect but got a message"));
                }
            }
        }
    }
    Ok(())
}

/// Run the full matrix (each scenario against each of its target versions), starting a fresh
/// acceptor per version. Returns one [`ScenarioResult`] per (scenario, version).
pub async fn run_report(scenarios: &[Scenario]) -> Vec<ScenarioResult> {
    let mut results = Vec::new();
    // A fresh acceptor per (scenario, version) isolates session state and lets each scenario
    // request its own acceptor feature toggles.
    for s in scenarios {
        for version in &s.versions {
            let outcome = match start_acceptor(version, &s.tweaks).await {
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

async fn read_message(
    stream: &mut TcpStream,
    buf: &mut Vec<u8>,
    wait: Duration,
) -> Option<Message> {
    loop {
        if let Ok(Some(total)) = frame_length(buf) {
            let raw: Vec<u8> = buf.drain(..total).collect();
            return decode(&raw).ok();
        }
        let mut chunk = [0u8; 4096];
        match timeout(wait, stream.read(&mut chunk)).await {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return None,
            Ok(Ok(n)) => {
                if let Some(slice) = chunk.get(..n) {
                    buf.extend_from_slice(slice);
                }
            }
        }
    }
}

/// Helper to build a client-side message with the standard header.
pub fn client_message(version: &str, msg_type: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, version));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}
