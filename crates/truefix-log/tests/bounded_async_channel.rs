//! T067 — asynchronous log writers cap their pending work when a backend is slower than producers.

#![cfg(feature = "redb")]

use std::path::Path;
use std::time::{Duration, Instant};

use redb::{Database, ReadableDatabase, ReadableTableMetadata, TableDefinition};
use truefix_log::{Log, RedbLog};

const EVENT: TableDefinition<u64, (i64, &str, &str)> = TableDefinition::new("log_event");
const ATTEMPTED_ENTRIES: u64 = 4096;

async fn open_after_writer_stops(path: &Path) -> Database {
    let start = Instant::now();
    loop {
        match Database::open(path) {
            Ok(db) => return db,
            Err(_) if start.elapsed() < Duration::from_secs(15) => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(error) => panic!("background log writer did not stop: {error}"),
        }
    }
}

#[tokio::test]
async fn slow_async_consumer_has_a_bounded_pending_queue() {
    let path = std::env::temp_dir().join(format!(
        "truefix-bounded-async-log-{}-{}.redb",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    let _ = std::fs::remove_file(&path);

    let log = RedbLog::connect(&path).await.unwrap();

    // Every event requires its own blocking redb transaction and commit. Producing this burst is
    // therefore substantially faster than consuming it, deterministically saturating the queue.
    for index in 0..ATTEMPTED_ENTRIES {
        log.on_event(&format!("event-{index}"));
    }
    drop(log);

    let db = open_after_writer_stops(&path).await;
    let transaction = db.begin_read().unwrap();
    let table = transaction.open_table(EVENT).unwrap();
    let persisted = table.len().unwrap();

    assert!(
        persisted < ATTEMPTED_ENTRIES,
        "a slow consumer accepted all {ATTEMPTED_ENTRIES} entries; its pending queue is unbounded"
    );
    assert!(
        persisted > 0,
        "the writer should still persist queued entries"
    );

    drop(table);
    drop(transaction);
    drop(db);
    let _ = std::fs::remove_file(&path);
}
