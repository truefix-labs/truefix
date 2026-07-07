use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use truefix_at::runner::{ReadMessageOutcome, client_message, read_message, run_report};
use truefix_at::scenarios::server_suite;
use truefix_core::Field;
use truefix_session::{Application, Role, SessionConfig, SessionId};

struct DisconnectApp {
    logged_on: Arc<AtomicBool>,
    cancels: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Application for DisconnectApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }

    async fn on_cancel_on_disconnect(&self, _s: &SessionId) {
        self.cancels.fetch_add(1, Ordering::SeqCst);
    }
}

async fn wait_until<F: Fn() -> bool>(cond: F, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    cond()
}

fn logon() -> truefix_core::Message {
    let mut msg = client_message("FIX.4.4", "A", 1);
    msg.body.set(Field::int(98, 0));
    msg.body.set(Field::int(108, 30));
    msg
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_audit006_protocol_scenarios_pass() {
    let wanted = [
        "B5_DictionaryFailureDisconnectSendsLogout",
        "10_MsgSeqNumLess",
        "11b_NewSeqNoEqual",
        "RejectResentMessage",
        "5_AcceptorInitiatedLogout",
    ];
    let scenarios: Vec<_> = server_suite()
        .into_iter()
        .filter(|scenario| {
            wanted.contains(&scenario.name.as_str()) && scenario.versions == ["FIX.4.4"]
        })
        .collect();
    assert_eq!(
        scenarios.len(),
        wanted.len(),
        "the focused audit006 protocol scenario set should not silently shrink"
    );

    let results = run_report(&scenarios).await;
    let failed: Vec<_> = results
        .iter()
        .filter(|result| result.outcome.is_err())
        .collect();
    assert!(
        failed.is_empty(),
        "failed audit006 AT scenarios: {failed:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_on_disconnect_hook_runs_after_logged_on_peer_drops() {
    let logged_on = Arc::new(AtomicBool::new(false));
    let cancels = Arc::new(AtomicU32::new(0));
    let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc.check_latency = false;
    let acceptor = truefix_transport::Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc,
        Arc::new(DisconnectApp {
            logged_on: logged_on.clone(),
            cancels: cancels.clone(),
        }),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    let handle = acceptor.serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(&logon().encode()).await.unwrap();
    let mut buf = Vec::new();
    match read_message(&mut stream, &mut buf, Duration::from_secs(3)).await {
        ReadMessageOutcome::Message(msg) => assert_eq!(msg.msg_type(), Some("A")),
        other => panic!("expected Logon response, got {other:?}"),
    }
    assert!(
        wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(3)).await,
        "acceptor should observe the logged-on session before disconnect"
    );

    drop(stream);
    assert!(
        wait_until(
            || cancels.load(Ordering::SeqCst) == 1,
            Duration::from_secs(3)
        )
        .await,
        "logged-on unclean disconnect should invoke cancel-on-disconnect exactly once"
    );
    handle.abort();
}
