//! T052 (US7) — TLS established purely from a `.cfg` (`SocketUseSSL=Y` + key/trust-store paths),
//! via `Engine::start`, with no code-level TLS construction at all (FR-017; SC-008).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use truefix::config::SessionSettings;
use truefix::{Application, Engine, SessionId};

struct App {
    logons: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn scratch_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("truefix-cfgtls-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_server_identity(dir: &std::path::Path) -> (PathBuf, PathBuf) {
    let issued = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_pem = issued.cert.pem();
    let key_pem = issued.key_pair.serialize_pem();
    let combined = dir.join("server.combined.pem");
    std::fs::write(&combined, format!("{cert_pem}\n{key_pem}")).unwrap();
    let cert_only = dir.join("server.cert.pem");
    std::fs::write(&cert_only, &cert_pem).unwrap();
    (combined, cert_only)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_establishes_tls_purely_from_cfg() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let dir = scratch_dir();
    let (server_combined, server_cert_only) = write_server_identity(&dir);
    let port = free_port();

    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         SocketUseSSL=Y\n\
         SocketKeyStore={}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n\
         SocketUseSSL=Y\n\
         SocketKeyStore={}\n\
         SocketTrustStore={}\n\
         UseSNI=Y\n\
         SNIHostName=localhost\n",
        server_combined.display(),
        dir.join("unused.pem").display(),
        server_cert_only.display(),
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let logons = Arc::new(AtomicU32::new(0));
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
        }),
    )
    .await
    .expect("engine starts TLS purely from cfg");

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        logons.load(Ordering::SeqCst),
        2,
        "both sessions should log on over TLS configured purely via .cfg"
    );

    engine.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}
