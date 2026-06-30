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
    struct PassiveApp;
    #[async_trait::async_trait]
    impl Application for PassiveApp {
        async fn on_logon(&self, _s: &SessionId) {}
    }

    let mut template = SessionConfig::new(version, "SERVER", "CLIENT", Role::Acceptor);
    template.heartbeat_interval = 30;
    // Scenarios use fixed timestamps, so CheckLatency is off unless a scenario opts in.
    template.check_latency = tweaks.check_latency;
    template.enable_next_expected_msg_seq_num = tweaks.enable_next_expected;
    template.enable_last_msg_seq_num_processed = tweaks.enable_last_processed;

    // Enable dictionary validation so field-level reject scenarios produce Reject messages.
    let validator = match version {
        "FIX.4.2" => truefix_dict::load_fix42().ok(),
        "FIX.4.4" => truefix_dict::load_fix44().ok(),
        _ => None,
    }
    .map(|dict| (dict, truefix_dict::ValidationOptions::default()));
    let services = truefix_transport::Services {
        validator,
        ..truefix_transport::Services::default()
    };

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap_or_else(|_| unreachable_addr()),
        Arc::new(PassiveApp),
    )
    .await?
    .with_dynamic_template(template)
    .with_services(services);
    let addr = acceptor.local_addr()?;
    let handle = acceptor.serve();
    Ok((addr, handle))
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
                let msg = read_message(&mut stream, &mut buf, Duration::from_secs(2))
                    .await
                    .ok_or_else(|| {
                        format!("step {i}: expected {} but got nothing", expect.msg_type)
                    })?;
                check_match(&msg, expect).map_err(|e| format!("step {i}: {e}"))?;
            }
            Step::ExpectDisconnect => {
                if read_message(&mut stream, &mut buf, Duration::from_secs(2))
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
