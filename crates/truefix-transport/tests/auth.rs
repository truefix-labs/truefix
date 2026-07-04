//! T078 — Logon Username/Password authentication via toAdmin/fromAdmin.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use truefix_core::{Field, Message, Reject};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{AcceptorBuilder, connect_initiator};

/// Initiator that stamps Username(553)/Password(554) onto its Logon.
struct InitApp {
    password: String,
    logged_on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for InitApp {
    async fn to_admin(&self, message: &mut Message, _s: &SessionId) {
        if message.msg_type() == Some("A") {
            message.body.set(Field::string(553, "user"));
            message.body.set(Field::string(554, &self.password));
        }
    }
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
}

/// Acceptor that authenticates the Logon password.
struct AuthApp {
    expected: String,
    logged_on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for AuthApp {
    async fn from_admin(&self, message: &Message, _s: &SessionId) -> Result<(), Reject> {
        if message.msg_type() == Some("A") {
            let password = message.body.get(554).and_then(|f| f.as_str().ok());
            if password != Some(self.expected.as_str()) {
                return Err(Reject {
                    reason: 0,
                    ref_tag: None,
                    text: Some("authentication failed".to_owned()),
                    session_status: None,
                });
            }
        }
        Ok(())
    }
    async fn on_logon(&self, _s: &SessionId) {
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

async fn run_auth(password: &str) -> (bool, bool) {
    let acc_logged = Arc::new(AtomicBool::new(false));
    let init_logged = Arc::new(AtomicBool::new(false));

    let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc.heartbeat_interval = 30;
    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.heartbeat_interval = 30;

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(AuthApp {
            expected: "secret".to_owned(),
            logged_on: acc_logged.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let _handle = connect_initiator(
        addr,
        init,
        Arc::new(InitApp {
            password: password.to_owned(),
            logged_on: init_logged.clone(),
        }),
    )
    .await
    .unwrap();

    wait_until(
        || acc_logged.load(Ordering::SeqCst) && init_logged.load(Ordering::SeqCst),
        Duration::from_millis(1200),
    )
    .await;

    (
        acc_logged.load(Ordering::SeqCst),
        init_logged.load(Ordering::SeqCst),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn correct_password_logs_on() {
    let (acc, init) = run_auth("secret").await;
    assert!(acc && init, "valid credentials should log on");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_password_is_rejected() {
    let (acc, init) = run_auth("wrong").await;
    assert!(!acc && !init, "invalid credentials must not log on");
}
