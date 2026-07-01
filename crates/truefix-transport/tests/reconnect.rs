//! T065 / T071 — initiator reconnect, and socket-option application.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{connect_initiator_reconnecting, Services, SocketOptions};

struct NoopApp;

#[async_trait::async_trait]
impl Application for NoopApp {
    async fn on_logon(&self, _s: &SessionId) {}
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_reconnects_after_connection_drops() {
    // A bare listener that accepts a connection, reads a little, then drops it.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let connections = Arc::new(AtomicUsize::new(0));
    let c2 = connections.clone();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            c2.fetch_add(1, Ordering::SeqCst);
            let mut b = [0u8; 256];
            let _ = stream.read(&mut b).await;
            drop(stream); // force the initiator to reconnect
        }
    });

    let mut cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    cfg.reconnect_interval = 1;

    let handle = connect_initiator_reconnecting(addr, cfg, Arc::new(NoopApp), Services::default());

    let reconnected = wait_until(
        || connections.load(Ordering::SeqCst) >= 2,
        Duration::from_secs(6),
    )
    .await;
    handle.stop();
    handle.join().await;
    assert!(reconnected, "initiator should reconnect after a drop");
}

#[tokio::test]
async fn socket_options_apply_nodelay() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (client, _server) = tokio::join!(TcpStream::connect(addr), listener.accept());
    let stream = client.unwrap();

    SocketOptions { tcp_no_delay: true }.apply(&stream);
    assert!(stream.nodelay().unwrap());
}
