//! T077 — TLS handshake (initiator + acceptor) over rustls with a self-signed cert.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, RootCertStore, ServerConfig};

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{connect_initiator_tls, AcceptorBuilder, Services};

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
async fn logon_over_tls() {
    let (server_cfg, client_cfg) = make_tls_configs();

    let acc_flag = Arc::new(AtomicBool::new(false));
    let init_flag = Arc::new(AtomicBool::new(false));

    let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc.heartbeat_interval = 30;
    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.heartbeat_interval = 30;

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            on: acc_flag.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc)
    .with_tls(server_cfg);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let _handle = connect_initiator_tls(
        addr,
        init,
        Arc::new(FlagApp {
            on: init_flag.clone(),
        }),
        Services::default(),
        client_cfg,
        ServerName::try_from("localhost").unwrap(),
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on over TLS"
    );
}
