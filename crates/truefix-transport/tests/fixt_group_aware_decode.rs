//! T018/T020 (US1, feature 009, `NEW-58`): under a FIXT 1.1 dual-dictionary configuration
//! (`Services.fixt_dictionaries` set, `Services.validator` unset), `classify_buffered` only
//! checked `services.validator` to decide whether to engage group-aware decoding
//! (`decode_with_groups`/`HeaderTrailerGroupsOnly`) -- so a header/trailer repeating group (e.g.
//! `NoHops`, tag 627) never got structured under FIXT 1.1, contradicting GAP-26/FR-032's intent.
//! Depends on `NEW-55` (`fixt_header_tags.rs`) for `1128`/etc. to route to the header first.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use truefix_core::{Field, Group, Message, Reject};
use truefix_dict::{FixtDictionaries, load_fixt11};
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
async fn a_no_hops_header_group_is_structured_under_fixt_dual_dictionary_config() {
    let logged_on = Arc::new(AtomicBool::new(false));
    let last_logon: Arc<Mutex<Option<Message>>> = Arc::new(Mutex::new(None));

    let mut acc_cfg = SessionConfig::new("FIXT.1.1", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.check_latency = false;
    let transport_dict = load_fixt11().expect("FIXT11 transport dictionary");
    let dicts = FixtDictionaries::new(transport_dict);
    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(CapturingApp {
            logged_on: logged_on.clone(),
            last_logon: last_logon.clone(),
        }),
        Services {
            // Deliberately `fixt_dictionaries`, not `validator` -- the FIXT dual-dictionary
            // configuration this fix is about.
            fixt_dictionaries: Some((dicts, truefix_dict::ValidationOptions::default())),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    // A Logon carrying a NoHops (627) header group with two hops.
    let mut logon = Message::new();
    logon.header.set(Field::string(8, "FIXT.1.1"));
    logon.header.set(Field::string(35, "A"));
    logon.header.set(Field::int(34, 1));
    logon.header.set(Field::string(49, "CLIENT"));
    logon.header.set(Field::string(56, "SERVER"));
    logon.header.set(Field::string(52, "20240101-00:00:00"));
    let mut hops = Group::new(627);
    let mut hop1 = truefix_core::FieldMap::new();
    hop1.add_field(Field::string(628, "HOP1"));
    hops.add_entry(hop1);
    logon.header.add_group(hops);
    logon.body.set(Field::int(98, 0));
    logon.body.set(Field::int(108, 30));
    logon.body.set(Field::string(1137, "9")); // DefaultApplVerID = FIX50SP2

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
        "NoHops must be structured as a group under FIXT dual-dictionary config, not left as \
         flat repeated 628 fields (NEW-58)",
    );
    assert_eq!(group.len(), 1);
    assert_eq!(group[0].get(628).unwrap().as_str().unwrap(), "HOP1");
}
