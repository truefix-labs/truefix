//! T059 — SQL log (behind the `sql` feature): entries are persisted via the background writer.

#![cfg(feature = "sql")]

use std::time::Duration;

use truefix_log::{Log, SqlLog};

#[tokio::test]
async fn sql_log_persists_messages_and_events() {
    let dir = std::env::temp_dir().join(format!("truefix-sqllog-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let url = format!("sqlite:{}/log.db", dir.display());

    let log = SqlLog::connect(&url).await.unwrap();
    log.on_incoming("8=FIX.4.4|35=A");
    log.on_outgoing("8=FIX.4.4|35=0");
    log.on_event("logged on");

    // Allow the background writer to flush.
    tokio::time::sleep(Duration::from_millis(300)).await;

    use sqlx::Row;
    let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
    let msgs: i64 = sqlx::query("SELECT COUNT(*) AS c FROM log_messages")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("c");
    let events: i64 = sqlx::query("SELECT COUNT(*) AS c FROM log_events")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("c");

    assert_eq!(msgs, 2, "two messages logged");
    assert_eq!(events, 1, "one event logged");

    let _ = std::fs::remove_dir_all(&dir);
}
