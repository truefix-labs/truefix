//! T062 / T063 — multi-session routing, dynamic sessions, and allow-listing (SC-003).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{AcceptorBuilder, SessionHandle, connect_initiator};

struct CountApp {
    logons: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Application for CountApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
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

fn acc_with_sub_id(
    sender: &str,
    target: &str,
    sender_sub_id: &str,
    target_sub_id: &str,
) -> SessionConfig {
    let mut c = acc(sender, target);
    c.sender_sub_id = Some(sender_sub_id.to_owned());
    c.target_sub_id = Some(target_sub_id.to_owned());
    c
}

fn init_with_sub_id(
    sender: &str,
    target: &str,
    sender_sub_id: &str,
    target_sub_id: &str,
) -> SessionConfig {
    let mut c = init(sender, target);
    c.sender_sub_id = Some(sender_sub_id.to_owned());
    c.target_sub_id = Some(target_sub_id.to_owned());
    c
}

// --- T023 (US2, feature 006): SubID/LocationID routing on a grouped acceptor (BUG-07/FR-010) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_sessions_sharing_one_port_distinguished_by_sub_id_each_route_correctly() {
    let acc_logons = Arc::new(AtomicUsize::new(0));
    // Two acceptor sessions sharing BeginString/SenderCompID/TargetCompID *and* one
    // SocketAcceptPort (grouped via AcceptorBuilder), distinguished only by SubID.
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(CountApp {
            logons: acc_logons.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc_with_sub_id("SERVER", "CLIENT", "ACCBOOK1", "CLIBOOK1"))
    .with_session(acc_with_sub_id("SERVER", "CLIENT", "ACCBOOK2", "CLIBOOK2"));
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let flag1 = Arc::new(AtomicBool::new(false));
    let flag2 = Arc::new(AtomicBool::new(false));
    let h1 = connect_initiator(
        addr,
        init_with_sub_id("CLIENT", "SERVER", "CLIBOOK1", "ACCBOOK1"),
        Arc::new(FlagApp { on: flag1.clone() }),
    )
    .await
    .unwrap();
    let h2 = connect_initiator(
        addr,
        init_with_sub_id("CLIENT", "SERVER", "CLIBOOK2", "ACCBOOK2"),
        Arc::new(FlagApp { on: flag2.clone() }),
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || acc_logons.load(Ordering::SeqCst) >= 2
                && flag1.load(Ordering::SeqCst)
                && flag2.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both SubID-distinguished sessions sharing one port should each route and log on \
         correctly -- before the fix, the routing key ignored SubID entirely and neither \
         connection could ever match a registered session"
    );

    drop(h1);
    drop(h2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn static_and_dynamic_sessions_route_correctly() {
    let acc_logons = Arc::new(AtomicUsize::new(0));
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(CountApp {
            logons: acc_logons.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc("SERVER1", "CLIENT1"))
    .with_session(acc("SERVER2", "CLIENT2"))
    .with_dynamic_template(acc("TEMPLATE", "TEMPLATE"));
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut handles: Vec<SessionHandle> = Vec::new();
    let flags: Vec<Arc<AtomicBool>> = (0..3).map(|_| Arc::new(AtomicBool::new(false))).collect();

    // Two static (CLIENT1->SERVER1, CLIENT2->SERVER2) and one dynamic (CLIENT3->SERVER3).
    let routes = [
        ("CLIENT1", "SERVER1"),
        ("CLIENT2", "SERVER2"),
        ("CLIENT3", "SERVER3"),
    ];
    for (i, (sender, target)) in routes.iter().enumerate() {
        let app = Arc::new(FlagApp {
            on: flags[i].clone(),
        });
        handles.push(
            connect_initiator(addr, init(sender, target), app)
                .await
                .unwrap(),
        );
    }

    assert!(
        wait_until(
            || acc_logons.load(Ordering::SeqCst) >= 3
                && flags.iter().all(|f| f.load(Ordering::SeqCst)),
            Duration::from_secs(5),
        )
        .await,
        "all three sessions (2 static + 1 dynamic) should log on"
    );

    drop(handles);
}

struct CapturingApp {
    seen: std::sync::Mutex<Option<SessionId>>,
    flag: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for CapturingApp {
    async fn on_logon(&self, s: &SessionId) {
        *self.seen.lock().unwrap() = Some(s.clone());
        self.flag.store(true, Ordering::SeqCst);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dynamic_session_template_carries_through_sub_id_and_location_id() {
    // T080 (US8, feature 006): GAP-19, a dynamic-session template match must carry the inbound
    // Logon's SubID/LocationID into the resulting per-connection SessionId, not just
    // BeginString/SenderCompID/TargetCompID.
    let flag = Arc::new(AtomicBool::new(false));
    let app = Arc::new(CapturingApp {
        seen: std::sync::Mutex::new(None),
        flag: flag.clone(),
    });
    let acceptor = AcceptorBuilder::bind("127.0.0.1:0".parse().unwrap(), app.clone())
        .await
        .unwrap()
        .with_dynamic_template(acc("TEMPLATE", "TEMPLATE"));
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let init_flag = Arc::new(AtomicBool::new(false));
    let init_app = Arc::new(FlagApp {
        on: init_flag.clone(),
    });
    let _handle = connect_initiator(
        addr,
        init_with_sub_id("CLIENT9", "SERVER9", "CLIBOOK9", "ACCBOOK9"),
        init_app,
    )
    .await
    .unwrap();

    assert!(
        wait_until(|| flag.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "dynamic session should log on"
    );
    let seen = app.seen.lock().unwrap().clone().expect("on_logon fired");
    assert_eq!(seen.sender_comp_id, "SERVER9");
    assert_eq!(seen.target_comp_id, "CLIENT9");
    assert_eq!(seen.sender_sub_id.as_deref(), Some("ACCBOOK9"));
    assert_eq!(seen.target_sub_id.as_deref(), Some("CLIBOOK9"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connection_from_disallowed_remote_is_refused() {
    let acc_logons = Arc::new(AtomicUsize::new(0));
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(CountApp {
            logons: acc_logons.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc("SERVER1", "CLIENT1"))
    .allow_remotes(vec!["10.0.0.1".parse().unwrap()]); // loopback is NOT allowed
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let flag = Arc::new(AtomicBool::new(false));
    let _h = connect_initiator(
        addr,
        init("CLIENT1", "SERVER1"),
        Arc::new(FlagApp { on: flag.clone() }),
    )
    .await
    .unwrap();

    // Give it time; the connection must be refused, so no logon ever completes.
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(!flag.load(Ordering::SeqCst), "initiator must not log on");
    assert_eq!(
        acc_logons.load(Ordering::SeqCst),
        0,
        "acceptor must not log on"
    );
}
