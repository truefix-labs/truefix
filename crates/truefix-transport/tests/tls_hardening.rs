//! T063 (US12) — TLS establishes from inline PEM bytes (no file path involved); a restricted
//! cipher-suite list is honored in the handshake (FR-017).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{TlsSpec, TlsVersion, build_client_config, build_server_config};

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

/// A self-signed cert+key pair as in-memory PEM bytes: `(combined_cert_and_key, cert_only)`.
fn identity_bytes(san: &str) -> (Vec<u8>, Vec<u8>) {
    let issued = rcgen::generate_simple_self_signed(vec![san.to_string()]).unwrap();
    let cert_pem = issued.cert.pem();
    let key_pem = issued.key_pair.serialize_pem();
    let combined = format!("{cert_pem}\n{key_pem}").into_bytes();
    (combined, cert_pem.into_bytes())
}

fn base_spec() -> TlsSpec {
    TlsSpec {
        key_store_path: None,
        key_store_bytes: None,
        trust_store_path: None,
        trust_store_bytes: None,
        need_client_auth: false,
        min_version: None,
        server_name: None,
        cipher_suites: Vec::new(),
    }
}

async fn run_session_pair(
    server_cfg: Arc<rustls::ServerConfig>,
    client_cfg: Arc<rustls::ClientConfig>,
) -> (Arc<AtomicBool>, Arc<AtomicBool>) {
    let acc_flag = Arc::new(AtomicBool::new(false));
    let init_flag = Arc::new(AtomicBool::new(false));

    let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc.heartbeat_interval = 5;
    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.heartbeat_interval = 5;

    let acceptor = truefix_transport::Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc,
        Arc::new(FlagApp {
            on: acc_flag.clone(),
        }),
    )
    .await
    .unwrap()
    .with_tls(server_cfg);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let handle = truefix_transport::connect_initiator_tls(
        addr,
        init,
        Arc::new(FlagApp {
            on: init_flag.clone(),
        }),
        truefix_transport::Services::default(),
        client_cfg,
        server_name,
    )
    .await;
    let _ = handle; // dropped on handshake failure; both flags stay false in that case
    (acc_flag, init_flag)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inline_pem_bytes_establish_tls_without_a_file_path() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let (server_combined, server_cert_only) = identity_bytes("localhost");

    let mut server_spec = base_spec();
    server_spec.key_store_bytes = Some(server_combined);

    let mut client_spec = base_spec();
    client_spec.trust_store_bytes = Some(server_cert_only);
    client_spec.server_name = Some("localhost".to_owned());

    // Neither spec's key_store_path/trust_store_path is ever set — proves the bytes path alone
    // is sufficient (Acceptance Scenario 3).
    let server_cfg = build_server_config(&server_spec).expect("server tls config from bytes");
    let client_cfg = build_client_config(&client_spec).expect("client tls config from bytes");

    let (acc_flag, init_flag) = run_session_pair(server_cfg, client_cfg).await;
    assert!(
        wait_until(
            || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on using inline-PEM-bytes TLS"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn matching_restricted_cipher_suites_still_handshake() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let (server_combined, server_cert_only) = identity_bytes("localhost");

    let mut server_spec = base_spec();
    server_spec.key_store_bytes = Some(server_combined);
    server_spec.min_version = Some(TlsVersion::Tls13);
    server_spec.cipher_suites = vec!["TLS13_AES_128_GCM_SHA256".to_owned()];

    let mut client_spec = base_spec();
    client_spec.trust_store_bytes = Some(server_cert_only);
    client_spec.server_name = Some("localhost".to_owned());
    client_spec.min_version = Some(TlsVersion::Tls13);
    client_spec.cipher_suites = vec!["TLS13_AES_128_GCM_SHA256".to_owned()];

    let server_cfg = build_server_config(&server_spec).expect("server tls config");
    let client_cfg = build_client_config(&client_spec).expect("client tls config");

    let (acc_flag, init_flag) = run_session_pair(server_cfg, client_cfg).await;
    assert!(
        wait_until(
            || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "a shared cipher suite should still let the handshake succeed"
    );
}

// --- T055 (US6, feature 006): unrecognized CipherSuites is a config-time error (GAP-53) ---

#[tokio::test]
async fn a_cipher_suites_typo_matching_nothing_is_a_clean_config_error() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let (server_combined, _server_cert_only) = identity_bytes("localhost");

    let mut server_spec = base_spec();
    server_spec.key_store_bytes = Some(server_combined);
    server_spec.cipher_suites = vec!["TLS13_NOT_A_REAL_SUITE".to_owned()];

    let err = match build_server_config(&server_spec) {
        Ok(_) => panic!(
            "expected a config-time error for a CipherSuites name matching no recognized suite, \
             not a silently-empty (non-functional) suite set"
        ),
        Err(e) => e,
    };
    assert!(
        format!("{err}").contains("TLS13_NOT_A_REAL_SUITE"),
        "expected the error to name the unrecognized suite, got: {err}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disjoint_cipher_suites_fail_the_handshake() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let (server_combined, server_cert_only) = identity_bytes("localhost");

    let mut server_spec = base_spec();
    server_spec.key_store_bytes = Some(server_combined);
    server_spec.min_version = Some(TlsVersion::Tls13);
    server_spec.cipher_suites = vec!["TLS13_AES_128_GCM_SHA256".to_owned()];

    let mut client_spec = base_spec();
    client_spec.trust_store_bytes = Some(server_cert_only);
    client_spec.server_name = Some("localhost".to_owned());
    client_spec.min_version = Some(TlsVersion::Tls13);
    client_spec.cipher_suites = vec!["TLS13_CHACHA20_POLY1305_SHA256".to_owned()]; // disjoint

    let server_cfg = build_server_config(&server_spec).expect("server tls config");
    let client_cfg = build_client_config(&client_spec).expect("client tls config");

    let (acc_flag, init_flag) = run_session_pair(server_cfg, client_cfg).await;
    // Neither side should complete a logon: the handshake itself never completes.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!acc_flag.load(Ordering::SeqCst));
    assert!(!init_flag.load(Ordering::SeqCst));
}
