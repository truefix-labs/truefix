//! T005 (US1, feature 004) — `connect_initiator_reconnecting_multi_tls` reconnects to a backup TLS
//! endpoint after the primary fails, extending `reconnect.rs`'s existing plain-TCP
//! bare-listener-forces-a-reconnect pattern with a real TLS handshake on the backup (FR-001).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio::net::TcpListener;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{AcceptorBuilder, Services, connect_initiator_reconnecting_multi_tls};

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

fn make_tls_configs() -> (Arc<ServerConfig>, Arc<ClientConfig>) {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let issued = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_der: CertificateDer<'static> = issued.cert.der().clone();
    let key_der = PrivateKeyDer::try_from(issued.key_pair.serialize_der()).unwrap();

    let server = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key_der)
        .unwrap();

    let mut roots = RootCertStore::empty();
    roots.add(cert_der).unwrap();
    let client = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    (Arc::new(server), Arc::new(client))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnects_to_a_backup_tls_endpoint_after_the_primary_fails() {
    let (server_cfg, client_cfg) = make_tls_configs();

    // A bare listener standing in for the primary: accepts a connection and drops it
    // immediately (before any TLS handshake completes), forcing the reconnect loop to rotate to
    // the next endpoint — mirrors `reconnect.rs`'s existing plain-TCP pattern.
    let primary_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let primary_addr = primary_listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((stream, _)) = primary_listener.accept().await {
            drop(stream);
        }
    });

    // A real, TLS-configured acceptor standing in for the backup.
    let init_flag = Arc::new(AtomicBool::new(false));
    let backup_flag = Arc::new(AtomicBool::new(false));
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    let backup_acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            on: backup_flag.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc_cfg)
    .with_tls(server_cfg);
    let backup_addr = backup_acceptor.local_addr().unwrap();
    backup_acceptor.serve();

    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 30;
    init_cfg.reconnect_interval = 1;

    // Not joined at the end: once the backup logon succeeds, `run_connection` keeps the session
    // alive indefinitely (heartbeats etc.) since nothing ends it — `ReconnectHandle` has no
    // graceful-logout capability (unlike `SessionHandle`), so `stop()` only takes effect between
    // connection attempts, not mid-session. Matching `tls.rs`'s established pattern: just let the
    // handle drop and rely on the test runtime's teardown to cancel the outstanding task.
    let _handle = connect_initiator_reconnecting_multi_tls(
        vec![primary_addr, backup_addr],
        init_cfg,
        Arc::new(FlagApp {
            on: init_flag.clone(),
        }),
        Services::default(),
        client_cfg,
        ServerName::try_from("localhost").unwrap(),
    );

    assert!(
        wait_until(
            || init_flag.load(Ordering::SeqCst) && backup_flag.load(Ordering::SeqCst),
            Duration::from_secs(10),
        )
        .await,
        "initiator should reconnect to the backup TLS endpoint and complete logon"
    );
}
