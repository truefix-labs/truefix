//! T083 — acceptor-and-initiator-side coverage confirming Constitution Principle VI parity
//! (acceptor and initiator are equally first-class) for the correctness-affecting stories US2
//! (session-owned durable resend), US3 (field-order validation), and US4 (remaining session
//! switches), extending `initiator_validation.rs`'s pattern from feature 002.
//!
//! `initiator_validation.rs` already proved *dictionary/enum* validation is role-agnostic (an
//! initiator rejects a bad-enum message exactly like an acceptor does) and `config_switches.rs`
//! already proved `ForceResendWhenCorruptedStore`/`RefreshOnLogon` work for a switch applied to
//! the *acceptor*. This file fills the remaining, previously-untested combinations: an out-of-order
//! field rejected by an *acceptor* (US3's other direction), `ForceResendWhenCorruptedStore` applied
//! to an *initiator* (US4's other direction), and — a scenario no existing test exercised for
//! either role — an initiator actually *serving* a `ResendRequest` from its own durable store with
//! `PossDupFlag=Y` (US2; `restart_continuity.rs` only proved the store persists across a session,
//! not that a peer's resend request against it round-trips correctly).
//!
//! US10 (`on_before_reset`/`Reject.session_status`) is deliberately *not* duplicated here: those
//! hooks are exercised by `crates/truefix-session/tests/application_hooks.rs` at the sans-IO
//! `Session` level, and `Session` never branches on `Role` for callback-hook invocation timing (the
//! same structural argument `initiator_validation.rs`'s doc comment makes) — a transport-level
//! two-process duplicate would prove nothing `application_hooks.rs` doesn't already cover for both
//! roles via direct `Role::Acceptor`/`Role::Initiator` construction.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use truefix_core::{Field, Message, decode};
use truefix_dict::{ValidationOptions, load_fix44};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_store::{FileStore, MessageStore};
use truefix_transport::{Acceptor, Monitor, Services, connect_initiator_with};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("truefix-rp-{}-{nanos}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

#[derive(Default)]
struct Flags {
    logged_on: AtomicBool,
}

struct App {
    f: Arc<Flags>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.f.logged_on.store(true, Ordering::SeqCst);
    }
}

fn new_order(clordid: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "D"));
    m.body.set(Field::string(11, clordid));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::int(54, 1));
    m.body.set(Field::int(38, 100));
    m.body.set(Field::string(40, "2"));
    m
}

/// US2 (FR-003/004) — an *initiator* replays a stored application message with `PossDupFlag=Y`
/// when its peer sends a real `ResendRequest`, proving "session-owned durable resend" works the
/// same regardless of which side owns the store. Uses a hand-rolled TCP peer (rather than a real
/// `Acceptor`) so the test controls exactly when the `ResendRequest` is sent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_serves_a_resend_request_from_its_own_durable_store() {
    let init_dir = unique_dir();
    let init_store = Arc::new(FileStore::open(&init_dir).unwrap());
    let monitor = Monitor::new();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 30;
    init_cfg.check_latency = false;
    init_cfg.reset_on_logon = false;

    let init_flags = Arc::new(Flags::default());
    let _handle = connect_initiator_with(
        addr,
        init_cfg,
        Arc::new(App {
            f: init_flags.clone(),
        }),
        Services {
            store: Some(init_store as Arc<dyn MessageStore>),
            monitor: Some(monitor.clone()),
            ..Services::default()
        },
    )
    .await
    .unwrap();

    let (mut peer, _) = listener.accept().await.unwrap();

    // Read the initiator's Logon (seq 1), then reply with our own — a minimal hand-rolled acceptor
    // side, in full control of when the ResendRequest below gets sent.
    let mut buf = [0u8; 512];
    let n = peer.read(&mut buf).await.unwrap();
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("A"));

    let mut reply_logon = Message::new();
    reply_logon.header.set(Field::string(8, "FIX.4.4"));
    reply_logon.header.set(Field::string(35, "A"));
    reply_logon.header.set(Field::int(34, 1));
    reply_logon.header.set(Field::string(49, "SERVER"));
    reply_logon.header.set(Field::string(56, "CLIENT"));
    reply_logon
        .header
        .set(Field::string(52, "20240101-00:00:00"));
    reply_logon.body.set(Field::int(98, 0));
    reply_logon.body.set(Field::int(108, 30));
    peer.write_all(&reply_logon.encode()).await.unwrap();

    assert!(
        wait_until(
            || init_flags.logged_on.load(Ordering::SeqCst),
            Duration::from_secs(3)
        )
        .await
    );

    // The initiator sends one order (seq 2), persisted to its store. Drain it off the wire (the
    // live send, no PossDup) before requesting the resend below, so the next read can't pick up
    // this same bytes instead of the actual resend reply.
    let id = SessionId::new("FIX.4.4", "CLIENT", "SERVER");
    assert!(monitor.send_app(&id, new_order("ORDER-1")).await);
    let n = tokio::time::timeout(Duration::from_secs(3), peer.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("D"));

    // Request a resend of exactly that message.
    let mut resend_request = Message::new();
    resend_request.header.set(Field::string(8, "FIX.4.4"));
    resend_request.header.set(Field::string(35, "2"));
    resend_request.header.set(Field::int(34, 2));
    resend_request.header.set(Field::string(49, "SERVER"));
    resend_request.header.set(Field::string(56, "CLIENT"));
    resend_request
        .header
        .set(Field::string(52, "20240101-00:00:00"));
    resend_request.body.set(Field::int(7, 2)); // BeginSeqNo
    resend_request.body.set(Field::int(16, 2)); // EndSeqNo
    peer.write_all(&resend_request.encode()).await.unwrap();

    let n = tokio::time::timeout(Duration::from_secs(5), peer.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    let resent = decode(&buf[..n]).unwrap();
    assert_eq!(resent.msg_type(), Some("D"), "expected the resent order");
    assert_eq!(
        resent.header.get(34).and_then(|f| f.as_int().ok()),
        Some(2),
        "a PossDup resend replays the ORIGINAL MsgSeqNum, not a new one"
    );
    assert_eq!(
        resent.header.get(43).and_then(|f| f.as_str().ok()),
        Some("Y"),
        "PossDupFlag must be set on a resend"
    );
    assert_eq!(
        resent.body.get(11).and_then(|f| f.as_str().ok()),
        Some("ORDER-1"),
        "the resent order must be the one originally sent, from the initiator's own store"
    );

    let _ = std::fs::remove_dir_all(&init_dir);
}

fn raw_message(rest: &[(u32, &str)]) -> Vec<u8> {
    let body: String = rest.iter().map(|(t, v)| format!("{t}={v}\x01")).collect();
    let prefix = format!("8=FIX.4.4\x019={}\x01", body.len());
    let pre_checksum = format!("{prefix}{body}");
    let checksum: u32 = pre_checksum.bytes().map(u32::from).sum::<u32>() & 0xFF;
    format!("{pre_checksum}10={checksum:03}\x01").into_bytes()
}

/// US3 (FR-006) — an *acceptor* rejects a field-order violation exactly like the dictionary-level
/// contract (`crates/truefix-dict/tests/field_order.rs::out_of_order_fields_rejected_when_toggle_enabled`,
/// `SessionRejectReason=14`) says it should, when `ValidateFieldsOutOfOrder` is enabled — the
/// complement of `initiator_validation.rs`'s existing "initiator rejects a bad-enum message" case
/// (a different check, and the other role).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acceptor_rejects_a_field_order_violation_when_toggle_enabled() {
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    acc_cfg.check_latency = false;

    let acc_flags = Arc::new(Flags::default());
    let opts = ValidationOptions {
        validate_fields_out_of_order: true,
        ..ValidationOptions::default()
    };
    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(App {
            f: acc_flags.clone(),
        }),
        Services {
            validator: Some((load_fix44().unwrap(), opts)),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(&raw_message(&[
            (35, "A"),
            (49, "CLIENT"),
            (56, "SERVER"),
            (34, "1"),
            (52, "20240101-00:00:00"),
            (98, "0"),
            (108, "30"),
        ]))
        .await
        .unwrap();

    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("A"));
    assert!(
        wait_until(
            || acc_flags.logged_on.load(Ordering::SeqCst),
            Duration::from_secs(3)
        )
        .await
    );

    // Same shape as `field_order.rs::OUT_OF_ORDER`: 55 (a body field) arrives before the header
    // section (49/56/34/52) is finished.
    stream
        .write_all(&raw_message(&[
            (35, "D"),
            (55, "AAPL"),
            (49, "CLIENT"),
            (56, "SERVER"),
            (34, "2"),
            (52, "20240101-00:00:00"),
            (11, "O1"),
            (21, "1"),
            (54, "1"),
            (60, "20240101-00:00:00"),
            (40, "2"),
        ]))
        .await
        .unwrap();

    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    let reject = decode(&buf[..n]).unwrap();
    assert_eq!(reject.msg_type(), Some("3"), "expected a session Reject");
    assert_eq!(
        reject.body.get(373).and_then(|f| f.as_int().ok()),
        Some(14),
        "SessionRejectReason should be TagOutOfRequiredOrder, matching the dictionary-level contract"
    );
}

/// US4 (FR-008) — `ForceResendWhenCorruptedStore` applied to an *initiator* (rather than
/// `config_switches.rs`'s acceptor-side coverage): a recovered corruption in the initiator's own
/// store forces a full reset instead of trusting the recovered state.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn force_resend_when_corrupted_store_forces_a_reset_on_the_initiator_side() {
    let acc_dir = unique_dir();
    let init_dir = unique_dir();

    // Pre-populate and then corrupt the *initiator's* store this time.
    {
        let s = FileStore::open(&init_dir).unwrap();
        s.save(1, b"x").await.unwrap();
        s.set_next_sender_seq(2).await.unwrap();
    }
    let body = init_dir.join("body");
    let mut bytes = std::fs::read(&body).unwrap();
    bytes.extend_from_slice(&[9, 9, 9]);
    std::fs::write(&body, bytes).unwrap();

    let init_store = Arc::new(FileStore::open(&init_dir).unwrap());
    assert!(init_store.was_corrupted());
    let acc_store = Arc::new(FileStore::open(&acc_dir).unwrap());

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 5;
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 5;
    init_cfg.force_resend_when_corrupted_store = true; // the behavior under test, initiator-side

    let acc_flags = Arc::new(Flags::default());
    let init_flags = Arc::new(Flags::default());

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(App {
            f: acc_flags.clone(),
        }),
        Services {
            store: Some(acc_store.clone() as Arc<dyn MessageStore>),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let handle = connect_initiator_with(
        addr,
        init_cfg,
        Arc::new(App {
            f: init_flags.clone(),
        }),
        Services {
            store: Some(init_store.clone() as Arc<dyn MessageStore>),
            ..Services::default()
        },
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || acc_flags.logged_on.load(Ordering::SeqCst)
                && init_flags.logged_on.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on after the initiator's forced reset"
    );

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !init_store.was_corrupted(),
        "the forced reset should clear the corruption flag along with the store"
    );

    handle.logout().await;
    handle.join().await;
    let _ = std::fs::remove_dir_all(&acc_dir);
    let _ = std::fs::remove_dir_all(&init_dir);
}
