use std::io::ErrorKind;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio::net::{TcpListener, TcpStream};

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{Acceptor, Services, connect_initiator_tls};

struct FlagApp {
    logged_on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for FlagApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
}

fn make_tls_configs() -> (Arc<ServerConfig>, Arc<ClientConfig>) {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let issued = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_tls_handshake_is_bounded_by_connect_timeout() {
    let (_server_cfg, client_cfg) = make_tls_configs();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let _server = tokio::spawn(async move {
        let _held = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.connect_timeout = Some(Duration::from_millis(200));

    let started = Instant::now();
    let result = connect_initiator_tls(
        addr,
        init,
        Arc::new(FlagApp {
            logged_on: Arc::new(AtomicBool::new(false)),
        }),
        Services::default(),
        client_cfg,
        ServerName::try_from("localhost").unwrap(),
    )
    .await;
    let err = match result {
        Ok(_) => panic!("expected the stalled TLS handshake to time out"),
        Err(err) => err,
    };

    assert_eq!(err.kind(), ErrorKind::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "TLS handshake should time out promptly, not wait for the peer forever"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acceptor_tls_timeout_releases_single_session_active_flag() {
    let (server_cfg, client_cfg) = make_tls_configs();
    let acc_logged_on = Arc::new(AtomicBool::new(false));
    let init_logged_on = Arc::new(AtomicBool::new(false));

    let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc.connect_timeout = Some(Duration::from_millis(200));
    acc.heartbeat_interval = 30;
    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc,
        Arc::new(FlagApp {
            logged_on: acc_logged_on.clone(),
        }),
    )
    .await
    .unwrap()
    .with_tls(server_cfg);
    let addr = acceptor.local_addr().unwrap();
    let _acceptor_task = acceptor.serve();

    let _stalled_tcp = TcpStream::connect(addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.connect_timeout = Some(Duration::from_secs(2));
    init.heartbeat_interval = 30;
    let _handle = connect_initiator_tls(
        addr,
        init,
        Arc::new(FlagApp {
            logged_on: init_logged_on.clone(),
        }),
        Services::default(),
        client_cfg,
        ServerName::try_from("localhost").unwrap(),
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || acc_logged_on.load(Ordering::SeqCst) && init_logged_on.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "a timed-out TLS pre-session must not keep the single-session acceptor active forever"
    );
}
