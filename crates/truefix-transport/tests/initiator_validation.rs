//! T082 — initiator-side validation parity (Constitution VI: acceptor and initiator are equally
//! first-class). `Session::handle` (the sans-IO state machine in `truefix-session`) branches on
//! `Role` only for who sends Logon first and how Logon itself is answered
//! (`crates/truefix-session/src/state.rs`'s `on_connected`/Logon-handling); every other inbound
//! check (dictionary/group/enum validation, reject-layer selection, garbled-message handling) runs
//! through the exact same code path regardless of role, and `run_connection` in
//! `crates/truefix-transport/src/lib.rs` is the single function driving both
//! `Acceptor::bind_with` and `connect_initiator_with`. This test proves it directly, rather than
//! only asserting it: a real acceptor sends a dictionary-invalid application message to a real
//! initiator (with the same dictionary validator wired), and the initiator must produce the exact
//! same `SessionRejectReason` an acceptor produces for the identical input in
//! `crates/truefix-session/tests/validation_hook.rs::invalid_app_message_is_rejected_and_seq_advances`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use truefix_core::{Field, Message, Reject};
use truefix_dict::{load_fix44, ValidationOptions};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{connect_initiator_with, AcceptorBuilder, Monitor, Services};

/// The acceptor sends this once logged on: structurally valid, but `Side` (54) carries an enum
/// value ("9") the FIX.4.4 dictionary doesn't recognize — the same malformed input
/// `validation_hook.rs`'s `new_order("9", 2)` uses.
fn bad_side_new_order(seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(11, "ORD1"));
    m.body.set(Field::string(21, "1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::string(54, "9")); // Side: not a valid enum value
    m.body.set(Field::string(60, "20240101-00:00:00"));
    m.body.set(Field::string(40, "2"));
    m
}

/// Captures the `SessionRejectReason`(373)/`RefSeqNum`(45) of the first Reject(35=3) or
/// BusinessMessageReject(35=j) it receives as an admin/app message.
#[derive(Default, Clone)]
struct Capture {
    msg_type: Arc<Mutex<Option<String>>>,
    reason: Arc<Mutex<Option<i64>>>,
    ref_seq: Arc<Mutex<Option<i64>>>,
}

impl Capture {
    /// Only records Reject(35=3)/BusinessMessageReject(35=j); Logon/Heartbeat and other routine
    /// admin traffic during the handshake must not be mistaken for the reject under test.
    fn record(&self, msg: &Message) {
        if !matches!(msg.msg_type(), Some("3") | Some("j")) {
            return;
        }
        *self.msg_type.lock().unwrap() = msg.msg_type().map(str::to_owned);
        if let Some(r) = msg.body.get(373).and_then(|f| f.as_int().ok()) {
            *self.reason.lock().unwrap() = Some(r);
        }
        if let Some(s) = msg.body.get(45).and_then(|f| f.as_int().ok()) {
            *self.ref_seq.lock().unwrap() = Some(s);
        }
    }
}

struct AcceptorApp {
    capture: Capture,
    logged_on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for AcceptorApp {
    async fn on_logon(&self, _id: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
    async fn from_admin(&self, message: &Message, _id: &SessionId) -> Result<(), Reject> {
        self.capture.record(message);
        Ok(())
    }
    async fn from_app(
        &self,
        message: &Message,
        _id: &SessionId,
    ) -> Result<(), truefix_core::BusinessReject> {
        self.capture.record(message);
        Ok(())
    }
}

struct InitiatorApp {
    logged_on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for InitiatorApp {
    async fn on_logon(&self, _id: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
}

async fn wait_until<F: Fn() -> bool>(cond: F, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    cond()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_rejects_dictionary_invalid_message_like_an_acceptor_would() {
    let acc_logged = Arc::new(AtomicBool::new(false));
    let init_logged = Arc::new(AtomicBool::new(false));
    let capture = Capture::default();

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    acc_cfg.check_latency = false;
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 30;
    init_cfg.check_latency = false;

    let monitor = Monitor::new();
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(AcceptorApp {
            capture: capture.clone(),
            logged_on: acc_logged.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc_cfg)
    .with_services(Services {
        monitor: Some(monitor.clone()),
        ..Services::default()
    });
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    // The initiator is the one under test: it gets the same dictionary + validation options an
    // acceptor would (`Services.validator`), proving the check applies to either role.
    let init_services = Services {
        validator: Some((load_fix44().unwrap(), ValidationOptions::default())),
        ..Services::default()
    };
    let _init_handle = connect_initiator_with(
        addr,
        init_cfg,
        Arc::new(InitiatorApp {
            logged_on: init_logged.clone(),
        }),
        init_services,
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || acc_logged.load(Ordering::SeqCst) && init_logged.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sessions should log on"
    );

    // Find the acceptor's SessionId to target the send.
    let ids = monitor.sessions();
    let acc_id = ids.first().expect("acceptor session registered").clone();
    assert!(
        monitor.send_app(&acc_id, bad_side_new_order(2)).await,
        "acceptor should send the malformed order to the initiator"
    );

    assert!(
        wait_until(
            || capture.msg_type.lock().unwrap().is_some(),
            Duration::from_secs(5),
        )
        .await,
        "the initiator should reply with a reject the acceptor receives"
    );

    // Same outcome `validation_hook.rs::invalid_app_message_is_rejected_and_seq_advances` asserts
    // for an acceptor receiving the identical malformed input: a session-level Reject (35=3) with
    // SessionRejectReason=5 (value incorrect/out of range) and RefSeqNum=2.
    assert_eq!(capture.msg_type.lock().unwrap().as_deref(), Some("3"));
    assert_eq!(*capture.reason.lock().unwrap(), Some(5));
    assert_eq!(*capture.ref_seq.lock().unwrap(), Some(2));
}
