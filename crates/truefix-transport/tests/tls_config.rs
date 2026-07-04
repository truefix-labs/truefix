//! T052 (US7) — TLS/mTLS built entirely from configuration (a `TlsSpec` of PEM file paths), not
//! code-level `rustls::{Server,Client}Config` construction (FR-017; SC-008).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{TlsConfigError, TlsSpec, build_client_config, build_server_config};

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

fn scratch_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("truefix-tlscfg-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write a self-signed cert+key pair to `dir` and return `(combined_pem_path, cert_only_pem_path)`.
fn write_identity(dir: &std::path::Path, name: &str, san: &str) -> (PathBuf, PathBuf) {
    let issued = rcgen::generate_simple_self_signed(vec![san.to_string()]).unwrap();
    let cert_pem = issued.cert.pem();
    let key_pem = issued.key_pair.serialize_pem();

    let combined = dir.join(format!("{name}.combined.pem"));
    std::fs::write(&combined, format!("{cert_pem}\n{key_pem}")).unwrap();
    let cert_only = dir.join(format!("{name}.cert.pem"));
    std::fs::write(&cert_only, &cert_pem).unwrap();
    (combined, cert_only)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tls_from_config_alone_establishes_a_session() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let dir = scratch_dir();
    let (server_combined, server_cert_only) = write_identity(&dir, "server", "localhost");

    let server_spec = TlsSpec {
        key_store_path: Some(server_combined),
        trust_store_path: None,
        key_store_bytes: None,
        trust_store_bytes: None,
        need_client_auth: false,
        min_version: None,
        server_name: None,
        cipher_suites: Vec::new(),
    };
    let client_spec = TlsSpec {
        key_store_path: Some(dir.join("unused.pem")), // client presents no cert (no_client_auth)
        trust_store_path: Some(server_cert_only),
        key_store_bytes: None,
        trust_store_bytes: None,
        need_client_auth: false,
        min_version: None,
        server_name: Some("localhost".to_owned()),
        cipher_suites: Vec::new(),
    };

    let server_cfg = build_server_config(&server_spec).expect("server tls config from files");
    let client_cfg = build_client_config(&client_spec).expect("client tls config from files");

    let acc_flag = Arc::new(AtomicBool::new(false));
    let init_flag = Arc::new(AtomicBool::new(false));

    let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc.heartbeat_interval = 30;
    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.heartbeat_interval = 30;

    let acceptor = truefix_transport::AcceptorBuilder::bind(
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

    let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let _handle = truefix_transport::connect_initiator_tls(
        addr,
        init,
        Arc::new(FlagApp {
            on: init_flag.clone(),
        }),
        truefix_transport::Services::default(),
        client_cfg,
        server_name,
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "TLS session built purely from TlsSpec file paths should log on"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mtls_requires_a_valid_client_certificate() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let dir = scratch_dir();
    let (server_combined, server_cert_only) = write_identity(&dir, "server-mtls", "localhost");
    let (client_combined, client_cert_only) = write_identity(&dir, "client-mtls", "client");
    // An unrelated identity: a client that is NOT in the server's trust store.
    let (untrusted_combined, _) = write_identity(&dir, "untrusted", "client");

    let server_spec = TlsSpec {
        key_store_path: Some(server_combined),
        trust_store_path: Some(client_cert_only), // trusts only the legitimate client cert
        key_store_bytes: None,
        trust_store_bytes: None,
        need_client_auth: true,
        min_version: None,
        server_name: None,
        cipher_suites: Vec::new(),
    };
    let good_client_spec = TlsSpec {
        key_store_path: Some(client_combined),
        trust_store_path: Some(server_cert_only.clone()),
        key_store_bytes: None,
        trust_store_bytes: None,
        need_client_auth: true,
        min_version: None,
        server_name: Some("localhost".to_owned()),
        cipher_suites: Vec::new(),
    };
    let bad_client_spec = TlsSpec {
        key_store_path: Some(untrusted_combined),
        trust_store_path: Some(server_cert_only),
        key_store_bytes: None,
        trust_store_bytes: None,
        need_client_auth: true,
        min_version: None,
        server_name: Some("localhost".to_owned()),
        cipher_suites: Vec::new(),
    };

    let server_cfg = build_server_config(&server_spec).expect("mTLS server config");
    let good_client_cfg = build_client_config(&good_client_spec).expect("trusted client config");
    let bad_client_cfg = build_client_config(&bad_client_spec).expect("untrusted client config");

    // A client presenting the trusted certificate logs on.
    {
        let acc_flag = Arc::new(AtomicBool::new(false));
        let init_flag = Arc::new(AtomicBool::new(false));
        let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
        acc.heartbeat_interval = 30;
        let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
        init.heartbeat_interval = 30;

        let acceptor = truefix_transport::AcceptorBuilder::bind(
            "127.0.0.1:0".parse().unwrap(),
            Arc::new(FlagApp {
                on: acc_flag.clone(),
            }),
        )
        .await
        .unwrap()
        .with_session(acc)
        .with_tls(server_cfg.clone());
        let addr = acceptor.local_addr().unwrap();
        acceptor.serve();

        let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
        let _handle = truefix_transport::connect_initiator_tls(
            addr,
            init,
            Arc::new(FlagApp {
                on: init_flag.clone(),
            }),
            truefix_transport::Services::default(),
            good_client_cfg,
            server_name,
        )
        .await
        .unwrap();

        assert!(
            wait_until(
                || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
                Duration::from_secs(5),
            )
            .await,
            "the trusted client certificate should be accepted"
        );
    }

    // A client presenting an untrusted certificate is refused (handshake fails; no logon).
    {
        let acc_flag = Arc::new(AtomicBool::new(false));
        let init_flag = Arc::new(AtomicBool::new(false));
        let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
        acc.heartbeat_interval = 30;
        let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
        init.heartbeat_interval = 30;

        let acceptor = truefix_transport::AcceptorBuilder::bind(
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

        let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
        let handle = truefix_transport::connect_initiator_tls(
            addr,
            init,
            Arc::new(FlagApp {
                on: init_flag.clone(),
            }),
            truefix_transport::Services::default(),
            bad_client_cfg,
            server_name,
        )
        .await
        .unwrap();

        // Give the handshake a chance to fail; no logon should ever occur.
        let logged_on = wait_until(
            || acc_flag.load(Ordering::SeqCst) || init_flag.load(Ordering::SeqCst),
            Duration::from_millis(800),
        )
        .await;
        assert!(
            !logged_on,
            "an untrusted client certificate must be refused"
        );
        handle.join().await;
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn need_client_auth_without_trust_store_is_a_typed_error() {
    let dir = scratch_dir();
    let (combined, _) = write_identity(&dir, "solo", "localhost");
    let spec = TlsSpec {
        key_store_path: Some(combined),
        trust_store_path: None,
        key_store_bytes: None,
        trust_store_bytes: None,
        need_client_auth: true,
        min_version: None,
        server_name: None,
        cipher_suites: Vec::new(),
    };
    assert!(matches!(
        build_server_config(&spec),
        Err(TlsConfigError::MissingTrustStoreForClientAuth)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
