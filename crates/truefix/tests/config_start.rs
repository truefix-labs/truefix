//! T007 (US1) — start a working acceptor + initiator entirely from a `.cfg`, with no code beyond
//! providing the Application (FR-013/014; SC-001).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use truefix::config::SessionSettings;
use truefix::{decode, Application, Engine, Field, Message, SessionId};
#[cfg(feature = "sql")]
use truefix_store::MessageStore;

struct App {
    logons: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }
}

/// Grab an ephemeral port, then release it for the acceptor to bind.
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_starts_acceptor_and_initiator_from_cfg() {
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
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n"
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
    .expect("engine starts from cfg");

    // Both the acceptor session and the initiator session should complete a logon.
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        logons.load(Ordering::SeqCst),
        2,
        "both sessions should log on when started from the .cfg"
    );

    engine.shutdown();
}

/// T072-adjacent (US12/FR-026): `FileLogPath` alone builds a working session log, with
/// `FileLogHeartbeats=N` filtering heartbeats and each line prefixed by the session ID.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_builds_file_log_from_cfg() {
    let port = free_port();
    let dir = std::env::temp_dir().join(format!("truefix-engine-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         FileLogPath={}\n\
         FileLogHeartbeats=N\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n",
        dir.display()
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
    .expect("engine starts from cfg");

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(logons.load(Ordering::SeqCst), 2);
    // Give the file log a moment to flush the logon event.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let events = std::fs::read_to_string(dir.join("event.log")).expect("event.log written");
    assert!(
        events.contains("FIX.4.4:SERVER->CLIENT") || events.contains("FIX.4.4:CLIENT->SERVER"),
        "event lines should be prefixed with the session ID: {events}"
    );
    let messages = std::fs::read_to_string(dir.join("messages.log")).expect("messages.log written");
    assert!(
        !messages.contains("35=0"),
        "FileLogHeartbeats=N should filter heartbeats from the message log"
    );

    engine.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

/// T019 (US3, feature 004) — a `.cfg`-only session with `JdbcURL=sqlite:...` persists sequence
/// numbers to the named SQL backend and survives a restart, with zero additional Rust code beyond
/// `Application` (FR-003, SC-003).
///
/// **Historical note**: until US8 (feature 005, FR-021), `StoreConfig::Sql`/`Mssql` carried only a
/// bare `url: String` — `JdbcSessionIdDefaultPropertyValue`/`JdbcStoreSessionsTableName` were
/// `Recognized`, not `Implemented`, in `crates/truefix-config/src/keys.rs`, and two sessions sharing
/// one `JdbcURL` collided on the same `"default"` store row. US8 closed this by threading
/// `sessions_table`/`messages_table`/`session_id`/`pool` through as optional fields (see
/// `store_and_log_mapping.rs`'s `jdbc_table_name_keys_apply_to_the_sql_store` for that coverage).
/// This test still gives the acceptor and initiator sessions their own SQLite file each — a
/// deliberately simpler setup than exercising the now-available `JdbcSessionIdDefaultPropertyValue`
/// override, which is covered separately in `truefix-config`.
#[cfg(feature = "sql")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cfg_only_session_with_jdbc_url_persists_sequence_numbers_across_a_restart() {
    let port = free_port();
    let dir = std::env::temp_dir().join(format!("truefix-jdbc-sql-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let acc_url = format!("sqlite:{}", dir.join("acceptor.db").display());
    let init_url = format!("sqlite:{}", dir.join("initiator.db").display());
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
         JdbcURL={acc_url}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n\
         JdbcURL={init_url}\n"
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
    .expect("engine starts from cfg");

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(logons.load(Ordering::SeqCst), 2);

    // Let a couple of heartbeats flow so sequence numbers advance and persist.
    tokio::time::sleep(Duration::from_millis(2500)).await;
    engine.shutdown();

    // Reopen the acceptor's SQLite file directly (bypassing `.cfg`) and confirm the advanced
    // sequence number survived the "restart" (a fresh `SqlStore` connection against the same file).
    let reopened = truefix_store::SqlStore::connect(&acc_url)
        .await
        .expect("reopen the sqlite store");
    let persisted = reopened.next_sender_seq().await.unwrap();
    assert!(
        persisted > 1,
        "sequence numbers should persist and advance (got {persisted})"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// T013 (US2, feature 004) — a `.cfg`-only acceptor with `UseDataDictionary=Y` rejects a
/// dictionary-invalid inbound message identically to a session whose `Services.validator` was
/// wired programmatically today (FR-002, SC-002). Same malformed input and expected outcome as
/// `crates/truefix-transport/tests/initiator_validation.rs`'s `bad_side_new_order`/
/// `crates/truefix-session/tests/validation_hook.rs`'s `invalid_app_message_is_rejected_and_seq_advances`
/// — proving the *`.cfg`-wiring* half of that same contract, with zero Rust code beyond
/// `Application`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cfg_only_acceptor_rejects_a_dictionary_invalid_message() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=30\n\
         CheckLatency=N\n\
         UseDataDictionary=Y\n\
         DataDictionary=FIX.4.4\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n"
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: Arc::new(AtomicU32::new(0)),
        }),
    )
    .await
    .expect("engine starts from cfg");

    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();

    let mut logon_msg = Message::new();
    logon_msg.header.set(Field::string(8, "FIX.4.4"));
    logon_msg.header.set(Field::string(35, "A"));
    logon_msg.header.set(Field::int(34, 1));
    logon_msg.header.set(Field::string(49, "CLIENT"));
    logon_msg.header.set(Field::string(56, "SERVER"));
    logon_msg.header.set(Field::string(52, "20240101-00:00:00"));
    logon_msg.body.set(Field::int(98, 0));
    logon_msg.body.set(Field::int(108, 30));
    stream.write_all(&logon_msg.encode()).await.unwrap();

    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("A"));

    // Structurally valid, but Side (54) carries an enum value the FIX.4.4 dictionary doesn't
    // recognize.
    let mut bad_order = Message::new();
    bad_order.header.set(Field::string(8, "FIX.4.4"));
    bad_order.header.set(Field::string(35, "D"));
    bad_order.header.set(Field::int(34, 2));
    bad_order.header.set(Field::string(49, "CLIENT"));
    bad_order.header.set(Field::string(56, "SERVER"));
    bad_order.header.set(Field::string(52, "20240101-00:00:00"));
    bad_order.body.set(Field::string(11, "ORD1"));
    bad_order.body.set(Field::string(21, "1"));
    bad_order.body.set(Field::string(55, "AAPL"));
    bad_order.body.set(Field::string(54, "Z")); // Side: not a valid enum value
    bad_order.body.set(Field::string(60, "20240101-00:00:00"));
    bad_order.body.set(Field::string(40, "2"));
    stream.write_all(&bad_order.encode()).await.unwrap();

    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    let reject = decode(&buf[..n]).unwrap();
    assert_eq!(reject.msg_type(), Some("3"), "expected a session Reject");
    assert_eq!(
        reject.body.get(373).and_then(|f| f.as_int().ok()),
        Some(5),
        "SessionRejectReason should be 5 (value is incorrect/out of range)"
    );
    assert_eq!(
        reject.body.get(45).and_then(|f| f.as_int().ok()),
        Some(2),
        "RefSeqNum should name the malformed message's own sequence number"
    );

    engine.shutdown();
}
