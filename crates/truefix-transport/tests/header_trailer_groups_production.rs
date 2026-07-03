//! T067 (US7, feature 006): a `NoHops` (627) header repeating group is decoded and structured on
//! the real production transport decode path (`classify_buffered`), not just the standalone
//! `decode_with_groups` primitive (already covered by
//! `crates/truefix-core/tests/header_trailer_groups.rs`) — GAP-26/FR-032.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use truefix_core::{Field, Group, Message, Reject};
use truefix_dict::load_fix44;
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{Acceptor, Services};

struct CapturingApp {
    logged_on: Arc<AtomicBool>,
    last_logon: Arc<Mutex<Option<Message>>>,
}

#[async_trait::async_trait]
impl Application for CapturingApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
    async fn from_admin(&self, msg: &Message, _s: &SessionId) -> Result<(), Reject> {
        if msg.msg_type() == Some("A") {
            *self.last_logon.lock().unwrap() = Some(msg.clone());
        }
        Ok(())
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_no_hops_header_group_is_structured_on_the_real_decode_path() {
    let logged_on = Arc::new(AtomicBool::new(false));
    let last_logon: Arc<Mutex<Option<Message>>> = Arc::new(Mutex::new(None));

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.check_latency = false;
    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(CapturingApp {
            logged_on: logged_on.clone(),
            last_logon: last_logon.clone(),
        }),
        Services {
            validator: Some((
                load_fix44().expect("FIX44 dictionary"),
                truefix_dict::ValidationOptions::default(),
            )),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    // A Logon carrying a NoHops (627) header group with two hops (628/629/630 each).
    let mut logon = Message::new();
    logon.header.set(Field::string(8, "FIX.4.4"));
    logon.header.set(Field::string(35, "A"));
    logon.header.set(Field::int(34, 1));
    logon.header.set(Field::string(49, "CLIENT"));
    logon.header.set(Field::string(56, "SERVER"));
    logon.header.set(Field::string(52, "20240101-00:00:00"));
    let mut hops = Group::new(627);
    let mut hop1 = truefix_core::FieldMap::new();
    hop1.add_field(Field::string(628, "HOP1"));
    let mut hop2 = truefix_core::FieldMap::new();
    hop2.add_field(Field::string(628, "HOP2"));
    hops.add_entry(hop1);
    hops.add_entry(hop2);
    logon.header.add_group(hops);
    logon.body.set(Field::int(98, 0));
    logon.body.set(Field::int(108, 30));

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut stream, &logon.encode())
        .await
        .unwrap();

    assert!(
        wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "session should log on despite the extra NoHops header group"
    );

    let captured = last_logon.lock().unwrap().clone().expect("captured Logon");
    let group = captured.header.group(627).expect(
        "NoHops must be structured as a group on the production decode path, not left as \
                 flat repeated 628 fields",
    );
    assert_eq!(group.len(), 2);
    assert_eq!(group[0].get(628).unwrap().as_str().unwrap(), "HOP1");
    assert_eq!(group[1].get(628).unwrap().as_str().unwrap(), "HOP2");
}
