//! T162 (NEW-91) — `Log::shutdown()` flushes/awaits every entry already queued in an async log
//! backend's background writer before returning, rather than letting them race a process exit.
//! Uses `RedbLog` as the representative async backend (the same channel+background-writer shape
//! `Sql`/`Mssql`/`Mongo`Log all share).

#![cfg(feature = "redb")]

use redb::{Database, ReadableDatabase, ReadableTableMetadata, TableDefinition};
use truefix_log::{Log, RedbLog};

const INCOMING: TableDefinition<u64, (i64, &str, &str)> = TableDefinition::new("log_incoming");

fn count(db: &Database, table: TableDefinition<u64, (i64, &str, &str)>) -> u64 {
    let txn = db.begin_read().unwrap();
    let t = txn.open_table(table).unwrap();
    t.len().unwrap()
}

#[tokio::test]
async fn shutdown_flushes_every_queued_entry_before_returning() {
    let path = std::env::temp_dir().join(format!(
        "truefix-redblog-shutdown-flush-{}.redb",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let log = RedbLog::connect(&path).await.unwrap();
    const N: u64 = 200;
    for i in 0..N {
        log.on_incoming(&format!("8=FIX.4.4|35=A|entry={i}"));
    }

    // No sleep, no "allow the background writer to flush" delay -- `shutdown()` itself must be
    // the synchronization point that guarantees every already-queued entry has been persisted.
    log.shutdown().await;

    let db = Database::open(&path).expect("the writer task must have released the file by now");
    assert_eq!(
        count(&db, INCOMING),
        N,
        "every entry queued before shutdown() must be persisted once it returns"
    );

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn shutdown_is_idempotent() {
    let path = std::env::temp_dir().join(format!(
        "truefix-redblog-shutdown-idempotent-{}.redb",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let log = RedbLog::connect(&path).await.unwrap();
    log.on_event("logged on");
    log.shutdown().await;
    // A second call must not panic or hang.
    log.shutdown().await;

    let _ = std::fs::remove_file(&path);
}
