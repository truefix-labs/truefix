//! T071/T072 (US2, feature 009, `NEW-26`): a reconnecting initiator resolves its hostname for
//! every connection attempt. The resolver deliberately changes its answer after the first
//! connection; reaching both listeners proves the reconnect loop did not retain the first IP.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use truefix_config::SocketEndpoint;
use truefix_session::{Application, Role, SessionConfig};
use truefix_transport::{AddressResolver, Services, connect_initiator_reconnecting_endpoints};

struct NoopApp;

#[async_trait::async_trait]
impl Application for NoopApp {}

struct ChangingResolver {
    calls: AtomicUsize,
    answers: [std::net::SocketAddr; 2],
}

#[async_trait::async_trait]
impl AddressResolver for ChangingResolver {
    async fn resolve(&self, endpoint: &SocketEndpoint) -> io::Result<Vec<std::net::SocketAddr>> {
        assert_eq!(endpoint.host(), "dynamic.example.test");
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![self.answers[call.min(1)]])
    }
}

async fn accept_one_and_drop(listener: TcpListener, accepted: Arc<AtomicUsize>) {
    let (mut stream, _) = listener.accept().await.expect("accept");
    accepted.fetch_add(1, Ordering::SeqCst);
    let mut buf = [0u8; 512];
    let _ = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut buf)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnect_resolves_the_hostname_again_and_uses_the_changed_answer() {
    let first = TcpListener::bind("127.0.0.1:0").await.expect("first bind");
    let second = TcpListener::bind("127.0.0.1:0").await.expect("second bind");
    let resolver = Arc::new(ChangingResolver {
        calls: AtomicUsize::new(0),
        answers: [
            first.local_addr().expect("first address"),
            second.local_addr().expect("second address"),
        ],
    });
    let accepted = Arc::new(AtomicUsize::new(0));
    let first_task = tokio::spawn(accept_one_and_drop(first, accepted.clone()));
    let second_task = tokio::spawn(accept_one_and_drop(second, accepted.clone()));

    let mut config = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    config.reconnect_interval = 1;
    let handle = connect_initiator_reconnecting_endpoints(
        vec![SocketEndpoint::new("dynamic.example.test", 5001)],
        config,
        Arc::new(NoopApp),
        Services {
            address_resolver: Some(resolver.clone()),
            ..Services::default()
        },
    );

    tokio::time::timeout(Duration::from_secs(5), async {
        while accepted.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("initiator must connect to both resolver answers");

    handle.stop();
    handle.join().await;
    first_task.await.expect("first listener task");
    second_task.await.expect("second listener task");
    assert!(
        resolver.calls.load(Ordering::SeqCst) >= 2,
        "resolver must be invoked once per connection attempt"
    );
}
