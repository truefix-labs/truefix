//! T026 (US5, feature 004) — embedded transactional log (behind the `redb` feature): entries are
//! persisted via the background writer, mirroring `sql_log.rs`'s pattern.

#![cfg(feature = "redb")]

use std::time::Duration;

use redb::{Database, ReadableDatabase, ReadableTableMetadata, TableDefinition};
use truefix_log::{Log, RedbLog};

const INCOMING: TableDefinition<u64, &str> = TableDefinition::new("log_incoming");
const OUTGOING: TableDefinition<u64, &str> = TableDefinition::new("log_outgoing");
const EVENT: TableDefinition<u64, &str> = TableDefinition::new("log_event");

fn count(db: &Database, table: TableDefinition<u64, &str>) -> u64 {
    let txn = db.begin_read().unwrap();
    let t = txn.open_table(table).unwrap();
    t.len().unwrap()
}

/// `RedbLog`'s background writer task holds the `redb::Database`'s exclusive file lock until it
/// notices its channel closed and exits — an async race relative to `drop(log)` returning
/// synchronously. Retries `Database::open` for a short window rather than a single fixed sleep
/// guess, so this isn't flaky under system load.
async fn open_with_retry(path: &std::path::Path) -> Database {
    let start = std::time::Instant::now();
    loop {
        match Database::open(path) {
            Ok(db) => return db,
            Err(_) if start.elapsed() < Duration::from_secs(3) => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(e) => panic!("failed to reopen the redb log file: {e}"),
        }
    }
}

#[tokio::test]
async fn redb_log_persists_messages_and_events() {
    let path = std::env::temp_dir().join(format!("truefix-redblog-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let log = RedbLog::connect(&path).await.unwrap();
    log.on_incoming("8=FIX.4.4|35=A");
    log.on_outgoing("8=FIX.4.4|35=0");
    log.on_event("logged on");

    // Allow the background writer to flush.
    tokio::time::sleep(Duration::from_millis(300)).await;
    drop(log);

    let db = open_with_retry(&path).await;
    assert_eq!(count(&db, INCOMING), 1, "one incoming message logged");
    assert_eq!(count(&db, OUTGOING), 1, "one outgoing message logged");
    assert_eq!(count(&db, EVENT), 1, "one event logged");

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn redb_log_heartbeats_n_filters_heartbeats() {
    let path = std::env::temp_dir().join(format!("truefix-redblog-hb-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let log = RedbLog::connect_with_config(truefix_log::RedbLogConfig {
        include_heartbeats: false,
        ..truefix_log::RedbLogConfig::new(&path)
    })
    .await
    .unwrap();
    log.on_incoming("8=FIX.4.4\x0135=0\x01"); // heartbeat, should be filtered
    log.on_incoming("8=FIX.4.4\x0135=A\x01"); // logon, should be kept

    tokio::time::sleep(Duration::from_millis(300)).await;
    drop(log);

    let db = open_with_retry(&path).await;
    assert_eq!(
        count(&db, INCOMING),
        1,
        "only the non-heartbeat should be logged"
    );

    let _ = std::fs::remove_file(&path);
}
