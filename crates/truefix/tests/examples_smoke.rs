//! T101 — smoke tests proving the example scenarios run end-to-end.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use truefix_core::{BusinessReject, Field, Message};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{connect_initiator, AcceptorBuilder, Monitor, Services};

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

// --- executor (acceptor fills orders) <-> banzai (initiator) ---

struct ExecutorApp {
    monitor: Monitor,
}

#[async_trait::async_trait]
impl Application for ExecutorApp {
    async fn from_app(&self, message: &Message, id: &SessionId) -> Result<(), BusinessReject> {
        if message.msg_type() == Some("D") {
            let mut exec = Message::new();
            exec.header.set(Field::string(35, "8"));
            exec.body.set(Field::string(150, "0"));
            self.monitor.send_app(id, exec).await;
        }
        Ok(())
    }
}

struct ClientApp {
    got_exec: Arc<AtomicBool>,
    logged_on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for ClientApp {
    async fn on_logon(&self, _id: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
    async fn from_app(&self, message: &Message, _id: &SessionId) -> Result<(), BusinessReject> {
        if message.msg_type() == Some("8") {
            self.got_exec.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_and_banzai_order_to_execution_report() {
    let monitor = Monitor::new();
    let services = Services {
        monitor: Some(monitor.clone()),
        ..Services::default()
    };
    let mut template = SessionConfig::new("FIX.4.4", "EXEC", "CLIENT", Role::Acceptor);
    template.check_latency = false;

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(ExecutorApp { monitor }),
    )
    .await
    .unwrap()
    .with_dynamic_template(template)
    .with_services(services);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let got_exec = Arc::new(AtomicBool::new(false));
    let logged_on = Arc::new(AtomicBool::new(false));
    let mut cfg = SessionConfig::new("FIX.4.4", "CLIENT", "EXEC", Role::Initiator);
    cfg.check_latency = false;
    let handle = connect_initiator(
        addr,
        cfg,
        Arc::new(ClientApp {
            got_exec: got_exec.clone(),
            logged_on: logged_on.clone(),
        }),
    )
    .await
    .unwrap();

    assert!(
        wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "client should log on"
    );

    let mut order = Message::new();
    order.header.set(Field::string(35, "D"));
    order.body.set(Field::string(11, "ORDER-1"));
    order.body.set(Field::string(55, "AAPL"));
    order.body.set(Field::string(54, "1"));
    handle.send(order).await;

    assert!(
        wait_until(|| got_exec.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "client should receive an ExecutionReport"
    );
}

// --- multi_acceptor serves multiple sessions ---

struct CountApp {
    logons: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Application for CountApp {
    async fn on_logon(&self, _id: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_acceptor_serves_multiple_sessions() {
    let logons = Arc::new(AtomicUsize::new(0));
    let mut s1 = SessionConfig::new("FIX.4.4", "SERVER1", "CLIENT1", Role::Acceptor);
    s1.check_latency = false;
    let mut s2 = SessionConfig::new("FIX.4.4", "SERVER2", "CLIENT2", Role::Acceptor);
    s2.check_latency = false;

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(CountApp {
            logons: logons.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(s1)
    .with_session(s2);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut handles = Vec::new();
    for (sender, target) in [("CLIENT1", "SERVER1"), ("CLIENT2", "SERVER2")] {
        let mut c = SessionConfig::new("FIX.4.4", sender, target, Role::Initiator);
        c.check_latency = false;
        handles.push(
            connect_initiator(
                addr,
                c,
                Arc::new(CountApp {
                    logons: logons.clone(),
                }),
            )
            .await
            .unwrap(),
        );
    }

    assert!(
        wait_until(
            || logons.load(Ordering::SeqCst) >= 4,
            Duration::from_secs(5)
        )
        .await,
        "two sessions (both sides) should log on"
    );
    drop(handles);
}
