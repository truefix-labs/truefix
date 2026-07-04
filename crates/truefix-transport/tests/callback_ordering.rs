//! T032/T033 (US1, feature 007): `from_admin`/`from_app` is invoked only *after* session-layer
//! validation (sequence/identity/latency/PossDup/dictionary, all inside `dispatch`/
//! `session.handle`) has accepted the message (BUG-34/FR-010) — previously the callback ran first,
//! so it could observe (and act on) a message the session layer was about to reject outright.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use truefix_core::{Field, Message, Reject};
use truefix_session::{Application, Event, Role, Session, SessionConfig, SessionId};
use truefix_transport::{Acceptor, Services};

#[derive(Default)]
struct Counts {
    from_admin: AtomicUsize,
    logons: AtomicUsize,
}

struct CountingApp {
    c: Arc<Counts>,
}

#[async_trait::async_trait]
impl Application for CountingApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.c.logons.fetch_add(1, Ordering::SeqCst);
    }
    async fn from_admin(&self, _msg: &Message, _s: &SessionId) -> Result<(), Reject> {
        self.c.from_admin.fetch_add(1, Ordering::SeqCst);
        Ok(())
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

fn raw_logon_bytes(init_cfg: SessionConfig) -> Vec<u8> {
    let mut session = Session::new(init_cfg);
    let actions = session.handle(Event::Connected);
    for action in actions {
        if let truefix_session::Action::Send(msg) = action {
            return msg.encode();
        }
    }
    panic!("Event::Connected on a fresh initiator session must produce a Logon Send action");
}

/// A hand-built Heartbeat carrying an explicitly too-low `MsgSeqNum` (no `PossDupFlag`) -- a
/// session-layer-fatal condition (`on_received`'s `Ordering::Less`/no-`poss_dup` branch: Logout +
/// Disconnect) that must never reach `from_admin` at all.
fn too_low_seq_heartbeat_bytes(sender: &str, target: &str, seq: u64) -> Vec<u8> {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "0")); // Heartbeat
    m.header.set(Field::string(49, sender));
    m.header.set(Field::string(56, target));
    m.header.set(Field::int(34, seq as i64));
    m.header.set(Field::string(52, "20260703-00:00:00.000"));
    m.encode()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn from_admin_never_fires_for_a_message_the_session_layer_rejects() {
    let acc_counts = Arc::new(Counts::default());
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    acc_cfg.check_latency = false;
    acc_cfg.reset_on_logon = false;

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(CountingApp {
            c: acc_counts.clone(),
        }),
        Services::default(),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 30;
    init_cfg.reset_on_logon = false;

    let mut raw = TcpStream::connect(addr).await.unwrap();
    raw.write_all(&raw_logon_bytes(init_cfg)).await.unwrap();
    raw.flush().await.unwrap();

    assert!(
        wait_until(
            || acc_counts.logons.load(Ordering::SeqCst) >= 1,
            Duration::from_secs(5),
        )
        .await,
        "the acceptor should log on after the valid initial Logon"
    );
    assert_eq!(
        acc_counts.from_admin.load(Ordering::SeqCst),
        1,
        "from_admin should have fired exactly once, for the valid Logon"
    );

    // Now send a Heartbeat carrying MsgSeqNum=1 again (the Logon already consumed seq 1; the
    // acceptor expects 2 next) with no PossDupFlag -- a session-layer-fatal too-low sequence,
    // which must trigger Logout+Disconnect *without* from_admin ever being invoked for it.
    raw.write_all(&too_low_seq_heartbeat_bytes("CLIENT", "SERVER", 1))
        .await
        .unwrap();
    raw.flush().await.unwrap();

    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(
        acc_counts.from_admin.load(Ordering::SeqCst),
        1,
        "from_admin must NOT fire for a message the session layer rejects at the sequence-check \
         level -- the count must stay at 1 (only the earlier valid Logon), not advance to 2"
    );
}
