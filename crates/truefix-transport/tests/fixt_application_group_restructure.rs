//! T017-T021 (US2, feature 011, FR-008/FR-009/FR-010): under a FIXT 1.1 dual-dictionary
//! configuration, an application-layer message's body-level repeating group is structured using
//! the application dictionary resolved for that message's `ApplVerID` — not left transport-scoped
//! — for both acceptor and initiator roles (Constitution Principle VI). A session-layer message
//! stays on the transport dictionary, and a message whose `ApplVerID` can't be resolved falls back
//! to transport-scoped structuring rather than being rejected or misattributed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use truefix_core::{BusinessReject, Field, FieldMap, Group, Message, Reject};
use truefix_dict::{FixtDictionaries, load_fix50sp2, load_fixt11};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{Acceptor, Services, connect_initiator_with};

struct CapturingApp {
    logged_on: Arc<AtomicBool>,
    last_admin: Arc<Mutex<Option<Message>>>,
    last_app: Arc<Mutex<Option<Message>>>,
}

/// `(app, logged_on flag, last captured admin message, last captured app message)`, returned by
/// [`CapturingApp::new`] so each test can hold onto the shared handles it needs to assert on.
type CapturingAppHandles = (
    Arc<CapturingApp>,
    Arc<AtomicBool>,
    Arc<Mutex<Option<Message>>>,
    Arc<Mutex<Option<Message>>>,
);

impl CapturingApp {
    fn new() -> CapturingAppHandles {
        let logged_on = Arc::new(AtomicBool::new(false));
        let last_admin: Arc<Mutex<Option<Message>>> = Arc::new(Mutex::new(None));
        let last_app: Arc<Mutex<Option<Message>>> = Arc::new(Mutex::new(None));
        (
            Arc::new(Self {
                logged_on: logged_on.clone(),
                last_admin: last_admin.clone(),
                last_app: last_app.clone(),
            }),
            logged_on,
            last_admin,
            last_app,
        )
    }
}

#[async_trait::async_trait]
impl Application for CapturingApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
    async fn from_admin(&self, msg: &Message, _s: &SessionId) -> Result<(), Reject> {
        *self.last_admin.lock().unwrap() = Some(msg.clone());
        Ok(())
    }
    async fn from_app(&self, msg: &Message, _s: &SessionId) -> Result<(), BusinessReject> {
        *self.last_app.lock().unwrap() = Some(msg.clone());
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

fn fixt_dicts() -> FixtDictionaries {
    FixtDictionaries::new(load_fixt11().expect("FIXT11 transport dictionary")).with_application(
        "9",
        load_fix50sp2().expect("FIX50SP2 application dictionary"),
    )
}

fn logon(sender: &str, target: &str, seq: i64, default_appl_ver_id: Option<&str>) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIXT.1.1"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, sender));
    m.header.set(Field::string(56, target));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    if let Some(v) = default_appl_ver_id {
        m.body.set(Field::string(1137, v));
    }
    m
}

/// A `NewOrderSingle` with a two-entry `NoPartyIDs`(453) body group, optionally carrying an
/// explicit per-message `ApplVerID`(1128) header tag.
fn new_order_with_body_group(
    sender: &str,
    target: &str,
    seq: i64,
    appl_ver_id: Option<&str>,
) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIXT.1.1"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, sender));
    m.header.set(Field::string(56, target));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    if let Some(v) = appl_ver_id {
        m.header.set(Field::string(1128, v));
    }
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
    let mut party_ids = Group::new(453);
    party_ids.add_entry(entry1);
    party_ids.add_entry(entry2);
    m.body.add_group(party_ids);
    m
}

fn assert_party_ids_structured(msg: &Message) {
    let entries = msg
        .body
        .group(453)
        .expect("NoPartyIDs(453) must be structured against the resolved application dictionary");
    assert_eq!(
        entries.len(),
        2,
        "both entries must be visible, not just the first"
    );
    assert_eq!(entries[0].get(448).unwrap().as_str().unwrap(), "PARTY1");
    assert_eq!(entries[1].get(448).unwrap().as_str().unwrap(), "PARTY2");
}

/// T017 [US2]: an acceptor session structures an application-layer body group using the
/// application dictionary resolved from the negotiated `DefaultApplVerID`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acceptor_structures_application_body_group_via_negotiated_appl_ver_id() {
    let (app, logged_on, _last_admin, last_app) = CapturingApp::new();
    let mut acc_cfg = SessionConfig::new("FIXT.1.1", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.check_latency = false;
    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        app,
        Services {
            fixt_dictionaries: Some((fixt_dicts(), truefix_dict::ValidationOptions::default())),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        &logon("CLIENT", "SERVER", 1, Some("9")).encode(),
    )
    .await
    .unwrap();
    assert!(wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(5)).await);

    // No explicit ApplVerID(1128) on the message itself — resolution must fall back to the
    // negotiated DefaultApplVerID from Logon (1137=9), exactly like `validate_app` already does.
    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        &new_order_with_body_group("CLIENT", "SERVER", 2, None).encode(),
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || last_app.lock().unwrap().is_some(),
            Duration::from_secs(5)
        )
        .await
    );
    assert_party_ids_structured(&last_app.lock().unwrap().clone().unwrap());
}

/// T018 [US2]: same as above, but the session under test is an **initiator** (Constitution
/// Principle VI parity) — a manually-driven raw `TcpListener` stands in for the counterparty so
/// the initiator's own receive path (not the acceptor's) is what's exercised.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_structures_application_body_group_via_negotiated_appl_ver_id() {
    let (app, logged_on, _last_admin, last_app) = CapturingApp::new();
    let mut init_cfg = SessionConfig::new("FIXT.1.1", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.check_latency = false;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let _handle = connect_initiator_with(
        addr,
        init_cfg,
        app,
        Services {
            fixt_dictionaries: Some((fixt_dicts(), truefix_dict::ValidationOptions::default())),
            ..Services::default()
        },
    )
    .await
    .unwrap();

    let (mut stream, _) = listener.accept().await.unwrap();
    // Drain the initiator's own outbound Logon; its exact content doesn't matter here.
    let mut buf = [0u8; 4096];
    let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
        .await
        .unwrap();

    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        &logon("SERVER", "CLIENT", 1, Some("9")).encode(),
    )
    .await
    .unwrap();
    assert!(wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(5)).await);

    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        &new_order_with_body_group("SERVER", "CLIENT", 2, None).encode(),
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || last_app.lock().unwrap().is_some(),
            Duration::from_secs(5)
        )
        .await
    );
    assert_party_ids_structured(&last_app.lock().unwrap().clone().unwrap());
}

/// T019 [US2]: a session-layer message continues to be structured against the transport
/// dictionary — application-dictionary resolution never applies to it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_layer_message_is_unaffected_by_application_dictionary_resolution() {
    let (app, logged_on, last_admin, _last_app) = CapturingApp::new();
    let mut acc_cfg = SessionConfig::new("FIXT.1.1", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.check_latency = false;
    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        app,
        Services {
            fixt_dictionaries: Some((fixt_dicts(), truefix_dict::ValidationOptions::default())),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut logon_msg = logon("CLIENT", "SERVER", 1, Some("9"));
    let mut hops = Group::new(627);
    let mut hop1 = FieldMap::new();
    hop1.add_field(Field::string(628, "HOP1"));
    hops.add_entry(hop1);
    logon_msg.header.add_group(hops);
    tokio::io::AsyncWriteExt::write_all(&mut stream, &logon_msg.encode())
        .await
        .unwrap();

    assert!(wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(5)).await);
    let captured = last_admin.lock().unwrap().clone().expect("captured Logon");
    let group = captured
        .header
        .group(627)
        .expect("NoHops must still be structured against the transport dictionary");
    assert_eq!(group.len(), 1);
    assert_eq!(group[0].get(628).unwrap().as_str().unwrap(), "HOP1");
}

/// T020 [US2]: when a message's `ApplVerID` can't be resolved to any registered application
/// dictionary (here: the message carries its own explicit `ApplVerID`(1128), which takes
/// precedence over the negotiated `DefaultApplVerID`, but that explicit value has no registered
/// application dictionary), the engine falls back to leaving the body transport-scoped rather than
/// rejecting the connection or misattributing the message to the negotiated dictionary.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unresolvable_appl_ver_id_falls_back_to_transport_scoped_structuring() {
    let (app, logged_on, _last_admin, last_app) = CapturingApp::new();
    let mut acc_cfg = SessionConfig::new("FIXT.1.1", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.check_latency = false;
    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        app,
        Services {
            fixt_dictionaries: Some((fixt_dicts(), truefix_dict::ValidationOptions::default())),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    // DefaultApplVerID=9 negotiates successfully at Logon.
    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        &logon("CLIENT", "SERVER", 1, Some("9")).encode(),
    )
    .await
    .unwrap();
    assert!(wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(5)).await);

    // This message's own explicit ApplVerID=7 (unregistered) takes precedence over the negotiated
    // "9" and has no registered application dictionary.
    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        &new_order_with_body_group("CLIENT", "SERVER", 2, Some("7")).encode(),
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || last_app.lock().unwrap().is_some(),
            Duration::from_secs(5)
        )
        .await
    );
    let captured = last_app.lock().unwrap().clone().unwrap();
    assert!(
        captured.body.group(453).is_none(),
        "with no application dictionary resolvable, the body must stay transport-scoped (flat \
         for this group), not rejected or partially/incorrectly structured"
    );
    // The connection must not have been torn down or the message rejected outright -- the
    // application still received it (just flat).
    assert_eq!(captured.msg_type(), Some("D"));
}

/// T021 [US2]: two application-layer messages under different registered `ApplVerID`s in the same
/// connection each resolve their own application dictionary independently -- no single cached
/// choice for the connection.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_message_resolves_its_own_appl_ver_id_independently() {
    let (app, logged_on, _last_admin, last_app) = CapturingApp::new();
    let mut acc_cfg = SessionConfig::new("FIXT.1.1", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.check_latency = false;
    let dicts = FixtDictionaries::new(load_fixt11().expect("FIXT11 transport dictionary"))
        .with_application(
            "9",
            load_fix50sp2().expect("FIX50SP2 application dictionary"),
        );
    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        app,
        Services {
            fixt_dictionaries: Some((dicts, truefix_dict::ValidationOptions::default())),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    // DefaultApplVerID=9 is negotiated at Logon (required by the FIXT11 dictionary), but every
    // message below carries its own explicit ApplVerID(1128), which takes precedence -- proving
    // resolution is per-message, not a single cached connection-level choice.
    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        &logon("CLIENT", "SERVER", 1, Some("9")).encode(),
    )
    .await
    .unwrap();
    assert!(wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(5)).await);

    // First message: explicit ApplVerID=9 (registered) -- must structure.
    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        &new_order_with_body_group("CLIENT", "SERVER", 2, Some("9")).encode(),
    )
    .await
    .unwrap();
    assert!(
        wait_until(
            || last_app.lock().unwrap().is_some(),
            Duration::from_secs(5)
        )
        .await
    );
    assert_party_ids_structured(&last_app.lock().unwrap().clone().unwrap());
    *last_app.lock().unwrap() = None;

    // Second message: explicit ApplVerID="7" (unregistered) -- must fall back to flat, not reuse
    // the first message's resolved dictionary.
    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        &new_order_with_body_group("CLIENT", "SERVER", 3, Some("7")).encode(),
    )
    .await
    .unwrap();
    assert!(
        wait_until(
            || last_app.lock().unwrap().is_some(),
            Duration::from_secs(5)
        )
        .await
    );
    let second = last_app.lock().unwrap().clone().unwrap();
    assert!(
        second.body.group(453).is_none(),
        "an unregistered ApplVerID must not resolve to the previous message's dictionary"
    );
}
