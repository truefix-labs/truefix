//! T007/T008 (US1, feature 011, FR-003): a body-level repeating group (`NoPartyIDs`(453), with a
//! nested `NoPartySubIDs`(802)) is decoded and structured on the real production transport decode
//! path (`classify_buffered`) for a single-`DataDictionary` session — not just header/trailer
//! groups (already covered by `header_trailer_groups_production.rs`), and not just the standalone
//! `decode_with_groups` primitive.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use truefix_core::{BusinessReject, Field, FieldMap, Group, Message};
use truefix_dict::load_fix44;
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{Acceptor, Services};

struct CapturingApp {
    logged_on: Arc<AtomicBool>,
    last_new_order: Arc<Mutex<Option<Message>>>,
}

#[async_trait::async_trait]
impl Application for CapturingApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
    async fn from_app(&self, msg: &Message, _s: &SessionId) -> Result<(), BusinessReject> {
        if msg.msg_type() == Some("D") {
            *self.last_new_order.lock().unwrap() = Some(msg.clone());
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

fn logon(seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

/// A `NewOrderSingle` whose body carries a two-entry `NoPartyIDs`(453) group, the second entry of
/// which carries a nested one-entry `NoPartySubIDs`(802) group — built via `add_group` (not flat
/// `add_field`) and then encoded to raw wire bytes, exactly as a counterparty would send it.
fn new_order_single_with_body_group(seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(11, "ORD1"));
    m.body.set(Field::string(54, "1"));
    m.body.set(Field::string(60, "20240101-00:00:00"));
    m.body.set(Field::string(40, "2"));

    let mut entry1 = FieldMap::new();
    entry1.add_field(Field::string(448, "PARTY1"));
    entry1.add_field(Field::string(447, "D"));
    entry1.add_field(Field::int(452, 1));

    let mut entry2 = FieldMap::new();
    entry2.add_field(Field::string(448, "PARTY2"));
    entry2.add_field(Field::string(447, "D"));
    entry2.add_field(Field::int(452, 3));
    let mut sub_ids = Group::new(802);
    let mut sub_entry = FieldMap::new();
    sub_entry.add_field(Field::string(523, "SUB1"));
    sub_entry.add_field(Field::string(803, "1"));
    sub_ids.add_entry(sub_entry);
    entry2.add_group(sub_ids);

    let mut party_ids = Group::new(453);
    party_ids.add_entry(entry1);
    party_ids.add_entry(entry2);
    m.body.add_group(party_ids);
    m
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_body_level_group_with_a_nested_group_is_structured_on_the_real_decode_path() {
    let logged_on = Arc::new(AtomicBool::new(false));
    let last_new_order: Arc<Mutex<Option<Message>>> = Arc::new(Mutex::new(None));

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.check_latency = false;
    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(CapturingApp {
            logged_on: logged_on.clone(),
            last_new_order: last_new_order.clone(),
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

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut stream, &logon(1).encode())
        .await
        .unwrap();
    assert!(
        wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "session should log on"
    );

    tokio::io::AsyncWriteExt::write_all(&mut stream, &new_order_single_with_body_group(2).encode())
        .await
        .unwrap();

    assert!(
        wait_until(
            || last_new_order.lock().unwrap().is_some(),
            Duration::from_secs(5)
        )
        .await,
        "NewOrderSingle should be delivered to the application"
    );

    let captured = last_new_order
        .lock()
        .unwrap()
        .clone()
        .expect("captured NewOrderSingle");

    // The flat, group-skipping view is unchanged (FR-002/FR-004): every raw tag is still visible.
    let flat_tags: Vec<u32> = captured.body.fields().map(|f| f.tag()).collect();
    assert!(flat_tags.contains(&11));
    assert!(
        !flat_tags.contains(&453),
        "the group's own count tag is structured, not a flat field"
    );
    assert!(
        !flat_tags.contains(&448),
        "group member tags must not leak into the flat field view once structured"
    );

    // The structured group accessor sees every entry (SC-001) — no silent data loss.
    let party_ids = captured
        .body
        .group(453)
        .expect("NoPartyIDs(453) must be structured as a group, not left flat");
    assert_eq!(
        party_ids.len(),
        2,
        "both PartyID entries must be visible, not just the first"
    );
    assert_eq!(party_ids[0].get(448).unwrap().as_str().unwrap(), "PARTY1");
    assert_eq!(party_ids[1].get(448).unwrap().as_str().unwrap(), "PARTY2");

    // The nested group (a group of groups) is structured correctly at both levels.
    let nested = party_ids[1]
        .group(802)
        .expect("nested NoPartySubIDs(802) must be structured within the second entry");
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].get(523).unwrap().as_str().unwrap(), "SUB1");
}
