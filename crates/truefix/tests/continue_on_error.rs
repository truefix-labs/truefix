//! T020 (US4, feature 004) — a multi-session `.cfg` acceptor with `ContinueInitializationOnError=Y`
//! and one misconfigured session starts every other, validly-configured session; the same file with
//! the flag unset fails startup entirely, exactly as today (FR-005, SC-004).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use truefix::config::SessionSettings;
use truefix::{Application, Engine, SessionId};

struct App {
    logons: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// A misconfigured session: `UseDataDictionary=Y` but no `DataDictionary`/`AppDataDictionary`/
/// `TransportDataDictionary` value — a real, typed `ConfigError::MissingRequired` at resolve time
/// (T010's `use_data_dictionary_without_a_dictionary_key_is_a_typed_error`).
fn misconfigured_session_block() -> &'static str {
    "\n[SESSION]\n\
     ConnectionType=acceptor\n\
     SenderCompID=BROKEN\n\
     TargetCompID=CLIENT\n\
     SocketAcceptAddress=127.0.0.1\n\
     SocketAcceptPort=1\n\
     UseDataDictionary=Y\n"
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn continue_on_error_y_starts_the_other_sessions() {
    let good_port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=30\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={good_port}\n\
         {broken}\
         ContinueInitializationOnError=Y\n",
        broken = misconfigured_session_block(),
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let logons = Arc::new(AtomicU32::new(0));
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
        }),
    )
    .await
    .expect("engine starts, skipping the misconfigured session");

    // The valid acceptor session should be listening (proving it started, despite the other
    // session in the same `.cfg` file being unresolvable).
    tokio::net::TcpStream::connect(("127.0.0.1", good_port))
        .await
        .expect("the valid acceptor session should be listening");

    engine.shutdown();
}

// --- T047 (US5, feature 006): partial-start cleanup on failure (BUG-11/FR-021) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_start_failure_stops_already_started_sessions_instead_of_orphaning_them() {
    let good_port_1 = free_port();
    let good_port_2 = free_port();
    let good_port_3 = free_port();
    // Bind a listener externally on this port first, so the 4th session's own bind attempt fails
    // at RUNTIME (address already in use) -- distinct from continue_on_error_unset_fails_startup_
    // entirely_unchanged's config-RESOLUTION failure above.
    let conflict_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let conflict_port = conflict_listener.local_addr().unwrap().port();

    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=30\n\
         SocketAcceptAddress=127.0.0.1\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER1\n\
         TargetCompID=CLIENT1\n\
         SocketAcceptPort={good_port_1}\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER2\n\
         TargetCompID=CLIENT2\n\
         SocketAcceptPort={good_port_2}\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER3\n\
         TargetCompID=CLIENT3\n\
         SocketAcceptPort={good_port_3}\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER4\n\
         TargetCompID=CLIENT4\n\
         SocketAcceptPort={conflict_port}\n"
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let logons = Arc::new(AtomicU32::new(0));
    let result = Engine::start(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
        }),
    )
    .await;
    assert!(
        result.is_err(),
        "the 4th session's port conflict should fail startup (ContinueInitializationOnError unset)"
    );

    // The first three sessions must have been cleanly stopped, not left orphaned and running --
    // give the abort a brief moment to actually tear down the listening socket, then confirm each
    // port refuses new connections.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    for port in [good_port_1, good_port_2, good_port_3] {
        let connect = tokio::net::TcpStream::connect(("127.0.0.1", port)).await;
        assert!(
            connect.is_err(),
            "session on port {port} should have been stopped by the partial-start cleanup, \
             not left orphaned and still listening"
        );
    }

    drop(conflict_listener);
}

#[tokio::test]
async fn continue_on_error_unset_fails_startup_entirely_unchanged() {
    let good_port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=30\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={good_port}\n\
         {broken}",
        broken = misconfigured_session_block(),
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let logons = Arc::new(AtomicU32::new(0));
    let result = Engine::start(&settings, Arc::new(App { logons })).await;
    assert!(
        result.is_err(),
        "without ContinueInitializationOnError, startup should fail entirely, exactly as today"
    );
}

// --- T048 (US5, feature 006): group-level tolerance is strictest-member-wins (BUG-16/FR-022) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grouped_acceptor_with_conflicting_continue_on_error_values_uses_strictest_member() {
    // A group of two sessions sharing one SocketAcceptPort: the FIRST-listed member sets Y, the
    // SECOND leaves it unset (N). Before the fix, only the first member's value was consulted, so
    // the group's own startup failure below would have been (incorrectly) tolerated. With
    // strictest-member-wins, the group must fail regardless of member order.
    let conflict_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let conflict_port = conflict_listener.local_addr().unwrap().port();

    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=30\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={conflict_port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER1\n\
         TargetCompID=CLIENT1\n\
         ContinueInitializationOnError=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER2\n\
         TargetCompID=CLIENT2\n"
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let logons = Arc::new(AtomicU32::new(0));
    let result = Engine::start(&settings, Arc::new(App { logons })).await;
    assert!(
        result.is_err(),
        "the group's bind failure must NOT be tolerated -- the second member's stricter (unset/N) \
         preference must win over the first member's Y, not be silently overridden by it"
    );

    drop(conflict_listener);
}
