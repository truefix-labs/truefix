//! T007 (US1, feature 007): `SqlStore::ensure_schema` must add the `creation_time` column before
//! any statement that references it, so opening against a database created before that column
//! existed doesn't fail (BUG-26).

#![cfg(feature = "sql")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{MessageStore, SqlStore, SqlStoreConfig};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "truefix-sql-migration-order-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn opening_against_a_pre_existing_schema_without_creation_time_succeeds() {
    let dir = unique_dir();
    let url = format!("sqlite:{}/store.db", dir.display());
    let sessions_table = "t_legacy_sessions";
    let messages_table = "t_legacy_messages";

    // Manually construct the sessions table exactly as it would have existed *before*
    // `creation_time` was introduced (005's GAP-38) -- no such column at all.
    {
        use std::str::FromStr;
        let opts = sqlx::sqlite::SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(opts).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE TABLE {sessions_table} (\
             session_id TEXT NOT NULL PRIMARY KEY, sender INTEGER NOT NULL, target INTEGER NOT NULL)"
        )))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO {sessions_table} (session_id, sender, target) VALUES ('default', 1, 1)"
        )))
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    }

    // Opening a SqlStore against this pre-existing, column-less table must not fail -- previously
    // the INSERT (run before the ALTER TABLE that adds creation_time) referenced a nonexistent
    // column whenever CREATE TABLE IF NOT EXISTS was a no-op against an already-existing table.
    let config = SqlStoreConfig {
        sessions_table: sessions_table.to_owned(),
        messages_table: messages_table.to_owned(),
        session_id: "default".to_owned(),
        ..SqlStoreConfig::new(&url)
    };
    let store = SqlStore::connect_with_config(config).await.expect(
        "opening against a pre-creation_time schema must succeed, not fail on migration order",
    );

    assert_eq!(store.next_sender_seq().await.unwrap(), 1);
    assert_eq!(store.next_target_seq().await.unwrap(), 1);
}
