//! T059 — SQL log (behind the `sql` feature): entries are persisted via the background writer.

#![cfg(feature = "sql")]

use std::time::Duration;

use truefix_log::{Log, SqlLog, SqlLogConfig};

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
    let incoming: i64 = sqlx::query("SELECT COUNT(*) AS c FROM log_incoming")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("c");
    let outgoing: i64 = sqlx::query("SELECT COUNT(*) AS c FROM log_outgoing")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("c");
    let events: i64 = sqlx::query("SELECT COUNT(*) AS c FROM log_event")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("c");

    assert_eq!(incoming, 1, "one incoming message logged");
    assert_eq!(outgoing, 1, "one outgoing message logged");
    assert_eq!(events, 1, "one event logged");

    let _ = std::fs::remove_dir_all(&dir);
}

/// T056 (US7, feature 005): every row carries a `logged_at` timestamp and `session_id`
/// (GAP-41/FR-019), so it can be audited/replayed on its own without an external join.
#[tokio::test]
async fn sql_log_rows_carry_logged_at_and_session_id() {
    let dir = std::env::temp_dir().join(format!("truefix-sqllog-widened-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let url = format!("sqlite:{}/log.db", dir.display());

    let config = SqlLogConfig {
        session_id: "SERVER->CLIENT".to_owned(),
        ..SqlLogConfig::new(&url)
    };
    let log = SqlLog::connect_with_config(config).await.unwrap();
    log.on_event("logged on");
    tokio::time::sleep(Duration::from_millis(300)).await;

    use sqlx::Row;
    let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
    let row = sqlx::query("SELECT logged_at, session_id FROM log_event LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let logged_at: i64 = row.get("logged_at");
    let session_id: String = row.get("session_id");
    assert!(logged_at > 0, "expected a nonzero logged_at timestamp");
    assert_eq!(session_id, "SERVER->CLIENT");

    let _ = std::fs::remove_dir_all(&dir);
}
