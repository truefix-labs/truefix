//! T065 / T071 — initiator reconnect, and socket-option application.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{
    Services, SocketOptions, connect_initiator_reconnecting, connect_initiator_with,
};

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

// --- T040 (US6, feature 005): reconnect_interval_steps wiring end-to-end (GAP-14/FR-014) ---
// The stepping/sticking logic itself is unit-tested directly in
// `truefix-transport/src/lib.rs::reconnect_delay_tests`; this proves the `.cfg`-adjacent
// `SessionConfig` field actually reaches the reconnect loop, not just that the math is right.

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnect_interval_steps_are_honored_by_the_reconnect_loop() {
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
    cfg.reconnect_interval = 30; // must be ignored: reconnect_interval_steps takes priority
    cfg.reconnect_interval_steps = vec![1, 1];

    let handle = connect_initiator_reconnecting(addr, cfg, Arc::new(NoopApp), Services::default());

    // 3 accepts within 5s is only possible at ~1s spacing, not the 30s scalar interval.
    let reconnected = wait_until(
        || connections.load(Ordering::SeqCst) >= 3,
        Duration::from_secs(5),
    )
    .await;
    handle.stop();
    handle.join().await;
    assert!(
        reconnected,
        "reconnect_interval_steps should drive the reconnect loop, not the ignored scalar interval"
    );
}

// --- T041 (US6, feature 005): local-bind address (GAP-15/FR-015) ---

#[tokio::test]
async fn initiator_connects_from_the_configured_local_bind_address() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Reserve a specific local port, then release it for the initiator to bind — a strong
    // assertion (the peer-observed source *port* must match exactly) that the bind actually
    // took effect, unlike merely checking the source IP (which would be 127.0.0.1 regardless).
    let local_bind_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_port = local_bind_listener.local_addr().unwrap().port();
    drop(local_bind_listener);

    let mut cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    cfg.local_bind_addr = Some(([127, 0, 0, 1], local_port).into());

    let (accepted, handle) = tokio::join!(listener.accept(), async {
        connect_initiator_with(addr, cfg, Arc::new(NoopApp), Services::default())
            .await
            .unwrap()
    });
    let (server_stream, _) = accepted.unwrap();
    let peer = server_stream.peer_addr().unwrap();
    assert_eq!(peer.port(), local_port);
    handle.logout().await;
}

#[tokio::test]
async fn socket_options_apply_nodelay() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (client, _server) = tokio::join!(TcpStream::connect(addr), listener.accept());
    let stream = client.unwrap();

    SocketOptions {
        tcp_no_delay: true,
        ..Default::default()
    }
    .apply(&stream);
    assert!(stream.nodelay().unwrap());
}
