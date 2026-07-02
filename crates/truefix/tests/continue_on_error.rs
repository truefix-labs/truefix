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
