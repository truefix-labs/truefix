//! NEW-115: the accept loop keeps accepting after an error instead of silently dying.
//! NEW-121: `SessionHandle`/`ReconnectHandle`/`ScheduledHandle` expose their `SessionId`.
//! NEW-132: `SocketOptions::apply()` failures no longer panic or abort the connection.
//! NEW-134: `run_scheduled_initiator_tls` provides a TLS variant of the scheduled initiator.
//! NEW-135: the listener backlog is configurable via `SocketOptions::backlog`.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, RootCertStore, ServerConfig};

use truefix_session::{Application, Role, Schedule, SessionConfig, SessionId};
use truefix_transport::{
    AcceptorBuilder, Services, SocketOptions, connect_initiator_with, run_scheduled_initiator_tls,
};

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

// --- NEW-115: the accept loop keeps accepting connections, one after another ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audit006_acceptor_serves_several_sequential_connections() {
    // A regression guard for the accept loop shape: the previous `while let Ok((stream, peer)) =
    // listener.accept().await` form would have silently ended the whole accept loop on the first
    // error. This drives several sequential connect/logout cycles through one long-lived acceptor
    // -- if the loop had already stopped serving after any hiccup, later connections would hang
    // and this test would time out.
    let mut template = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    template.check_latency = false;
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            on: Arc::new(AtomicBool::new(false)),
        }),
    )
    .await
    .unwrap()
    .with_dynamic_template(template);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    for i in 0..3 {
        let flag = Arc::new(AtomicBool::new(false));
        let mut init = SessionConfig::new("FIX.4.4", format!("CLIENT{i}"), "SERVER", Role::Initiator);
        init.check_latency = false;
        let handle = connect_initiator_with(
            addr,
            init,
            Arc::new(FlagApp { on: flag.clone() }),
            Services::default(),
        )
        .await
        .unwrap();
        assert!(
            wait_until(|| flag.load(Ordering::SeqCst), Duration::from_secs(5)).await,
            "connection {i} should log on -- the accept loop must still be serving"
        );
        handle.abort();
    }
}

// --- NEW-135: listener backlog is configurable ---

#[tokio::test]
async fn audit006_configurable_backlog_still_binds_and_accepts() {
    let mut template = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    template.check_latency = false;
    let services = Services {
        socket_options: SocketOptions {
            backlog: 16,
            ..SocketOptions::default()
        },
        ..Services::default()
    };
    let acceptor = AcceptorBuilder::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            on: Arc::new(AtomicBool::new(false)),
        }),
        services,
    )
    .await
    .unwrap()
    .with_dynamic_template(template);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let flag = Arc::new(AtomicBool::new(false));
    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.check_latency = false;
    let _handle = connect_initiator_with(
        addr,
        init,
        Arc::new(FlagApp { on: flag.clone() }),
        Services::default(),
    )
    .await
    .unwrap();
    assert!(
        wait_until(|| flag.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "a small but valid configured backlog must not prevent binding or accepting"
    );
}

// --- NEW-132: SocketOptions::apply() failures are surfaced, not silently swallowed ---

#[tokio::test]
async fn audit006_socket_option_apply_does_not_panic_on_unsupported_values() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (client, _server) = tokio::join!(tokio::net::TcpStream::connect(addr), listener.accept());
    let stream = client.unwrap();

    // A buffer size this large is rejected by the OS on every common platform -- NEW-132 changed
    // this failure from a silently discarded `let _ =` to a logged `tracing::warn!`, but the
    // connection must still proceed usably either way (best-effort semantics preserved).
    let opts = SocketOptions {
        recv_buffer_size: Some(usize::MAX / 2),
        send_buffer_size: Some(usize::MAX / 2),
        ..SocketOptions::default()
    };
    opts.apply(&stream);
    assert!(stream.nodelay().is_ok(), "the stream must remain usable");
}

// --- NEW-121: handles expose their SessionId ---

#[tokio::test]
async fn audit006_session_handle_exposes_session_id() {
    let mut template = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    template.check_latency = false;
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            on: Arc::new(AtomicBool::new(false)),
        }),
    )
    .await
    .unwrap()
    .with_dynamic_template(template);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.check_latency = false;
    let expected = init.session_id();
    let handle = connect_initiator_with(
        addr,
        init,
        Arc::new(FlagApp {
            on: Arc::new(AtomicBool::new(false)),
        }),
        Services::default(),
    )
    .await
    .unwrap();
    assert_eq!(handle.session_id(), &expected);
}

// --- NEW-134: TLS scheduled initiator ---

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
async fn audit006_scheduled_initiator_tls_connects_and_logs_on() {
    let (server_cfg, client_cfg) = make_tls_configs();
    let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc.check_latency = false;
    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.check_latency = false;

    let acc_flag = Arc::new(AtomicBool::new(false));
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
    let addr: SocketAddr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let init_flag = Arc::new(AtomicBool::new(false));
    let handle = run_scheduled_initiator_tls(
        addr,
        init,
        Arc::new(FlagApp {
            on: init_flag.clone(),
        }),
        Services::default(),
        Schedule::non_stop(),
        client_cfg,
        ServerName::try_from("localhost").unwrap(),
    );

    assert!(
        wait_until(
            || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "the TLS scheduled initiator should connect and log on over TLS while in session"
    );
    handle.stop();
    handle.join().await;
}
