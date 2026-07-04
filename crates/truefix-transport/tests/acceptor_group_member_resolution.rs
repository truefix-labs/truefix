//! T057/T058 (US2, feature 007): an acceptor group's log/dictionary-validator/socket-options
//! configuration is resolved per connected session, not silently inherited from the group's
//! `Services` (built, pre-fix, only from the first registered member) (BUG-61/FR-022) — two
//! sessions sharing one port with *different* per-session `Log`s must each see their own outgoing
//! traffic logged to their own `Log`, not to whichever member happened to be resolved first.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{AcceptorBuilder, Services, connect_initiator};

struct CollectingLog {
    events: Mutex<Vec<String>>,
}
impl CollectingLog {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            events: Mutex::new(Vec::new()),
        })
    }
}
impl truefix_log::Log for CollectingLog {
    fn on_incoming(&self, message: &str) {
        self.events.lock().unwrap().push(format!("IN:{message}"));
    }
    fn on_outgoing(&self, message: &str) {
        self.events.lock().unwrap().push(format!("OUT:{message}"));
    }
    fn on_event(&self, text: &str) {
        self.events.lock().unwrap().push(format!("EVENT:{text}"));
    }
}

struct FlagApp {
    on: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl Application for FlagApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.on.store(true, Ordering::SeqCst);
    }
}

async fn wait_until<F: Fn() -> bool>(cond: F, timeout: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    cond()
}

fn acc(sender: &str, target: &str) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", sender, target, Role::Acceptor);
    c.heartbeat_interval = 30;
    c
}

fn init(sender: &str, target: &str) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", sender, target, Role::Initiator);
    c.heartbeat_interval = 30;
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_group_member_logs_to_its_own_per_session_log_not_the_primarys() {
    let session1 = acc("SERVER1", "CLIENT1");
    let session2 = acc("SERVER2", "CLIENT2");
    let sid1 = session1.session_id();
    let sid2 = session2.session_id();

    let log1 = CollectingLog::new();
    let log2 = CollectingLog::new();

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            on: Arc::new(AtomicBool::new(false)),
        }),
    )
    .await
    .unwrap()
    .with_session(session1)
    .with_session(session2)
    .with_session_services(
        sid1.clone(),
        Services {
            log: Some(log1.clone()),
            ..Services::default()
        },
    )
    .with_session_services(
        sid2.clone(),
        Services {
            log: Some(log2.clone()),
            ..Services::default()
        },
    );
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let flag1 = Arc::new(AtomicBool::new(false));
    let flag2 = Arc::new(AtomicBool::new(false));
    let _h1 = connect_initiator(
        addr,
        init("CLIENT1", "SERVER1"),
        Arc::new(FlagApp { on: flag1.clone() }),
    )
    .await
    .unwrap();
    let _h2 = connect_initiator(
        addr,
        init("CLIENT2", "SERVER2"),
        Arc::new(FlagApp { on: flag2.clone() }),
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || flag1.load(Ordering::SeqCst) && flag2.load(Ordering::SeqCst),
            std::time::Duration::from_secs(5),
        )
        .await,
        "both sessions should log on"
    );
    // Give the acceptor side's own Logon-response send a moment to be logged.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let events1 = log1.events.lock().unwrap().clone();
    let events2 = log2.events.lock().unwrap().clone();
    assert!(
        !events1.is_empty(),
        "session1's own per-session Log must have received its own traffic"
    );
    assert!(
        !events2.is_empty(),
        "session2's own per-session Log must have received its own traffic"
    );
    assert!(
        events1
            .iter()
            .any(|e| e.contains("SERVER1") || e.contains("CLIENT1")),
        "session1's log should reflect session1's own identity, got {events1:?}"
    );
    assert!(
        events2
            .iter()
            .any(|e| e.contains("SERVER2") || e.contains("CLIENT2")),
        "session2's log should reflect session2's own identity, got {events2:?}"
    );
    // The core BUG-61 regression: before the fix, EVERY session's traffic (including session2's)
    // was logged to whichever member's Log was resolved as the group-wide "primary" -- meaning
    // session2's own distinct `log2` would stay entirely empty (session2's traffic all going to
    // `log1` instead, or vice versa depending on registration order).
    assert!(
        !events2
            .iter()
            .any(|e| e.contains("SERVER1") || e.contains("CLIENT1")),
        "session2's log must not contain session1's traffic (that would mean they're sharing one \
         group-wide Log instead of each having its own) -- got {events2:?}"
    );
}
