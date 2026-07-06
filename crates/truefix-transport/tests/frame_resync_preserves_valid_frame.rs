use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use truefix_core::Field;
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::Acceptor;

struct LogonFlag(Arc<AtomicBool>);

#[async_trait::async_trait]
impl Application for LogonFlag {
    async fn on_logon(&self, _session_id: &SessionId) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn logon_bytes() -> Vec<u8> {
    let mut message = truefix_core::Message::new();
    message.header.set(Field::string(8, "FIX.4.4"));
    message.header.set(Field::string(35, "A"));
    message.header.set(Field::int(34, 1));
    message.header.set(Field::string(49, "CLIENT"));
    message.header.set(Field::string(56, "SERVER"));
    message.header.set(Field::string(52, "20240101-00:00:00"));
    message.body.set(Field::int(98, 0));
    message.body.set(Field::int(108, 30));
    message.encode()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn valid_frame_after_non_soh_terminated_garbage_survives_resync() {
    let logged_on = Arc::new(AtomicBool::new(false));
    let mut config = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    config.check_latency = false;

    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        config,
        Arc::new(LogonFlag(logged_on.clone())),
    )
    .await
    .unwrap();
    let address = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut bytes = b"garbage-without-soh".to_vec();
    bytes.extend_from_slice(&logon_bytes());
    TcpStream::connect(address)
        .await
        .unwrap()
        .write_all(&bytes)
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), async {
        while !logged_on.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the valid Logon following garbage must not be discarded");
}
