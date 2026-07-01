//! T069 (US12) — SQL store parity across PostgreSQL/MySQL/SQLite (FR-024). SQLite runs
//! unconditionally (file-backed, no external service); PostgreSQL/MySQL are gated on
//! `DATABASE_URL_PG`/`DATABASE_URL_MYSQL` being set to a reachable instance (CI provides both via
//! service containers — see `.github/workflows/ci.yml`'s `sql` job), since dev boxes without those
//! services shouldn't fail the suite.

#![cfg(feature = "sql")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{MessageStore, SqlPoolOptions, SqlStore, SqlStoreConfig};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("truefix-sql-{}-{nanos}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Exercises the full `MessageStore` contract against `url`, using a fresh, uniquely-named table
/// pair each call so repeated runs (and parallel test threads) never collide.
async fn exercise_full_contract(url: &str, unique_suffix: &str) {
    let config = SqlStoreConfig {
        sessions_table: format!("t_sessions_{unique_suffix}"),
        messages_table: format!("t_messages_{unique_suffix}"),
        session_id: "s1".to_owned(),
        pool: SqlPoolOptions {
            max_connections: 3,
            ..SqlPoolOptions::default()
        },
        ..SqlStoreConfig::new(url)
    };
    let store = SqlStore::connect_with_config(config).await.unwrap();

    assert_eq!(store.next_sender_seq().await.unwrap(), 1);
    assert_eq!(store.next_target_seq().await.unwrap(), 1);

    store.set_next_sender_seq(11).await.unwrap();
    store.set_next_target_seq(22).await.unwrap();
    assert_eq!(store.next_sender_seq().await.unwrap(), 11);
    assert_eq!(store.next_target_seq().await.unwrap(), 22);

    store.save(1, b"first").await.unwrap();
    store.save(2, b"second").await.unwrap();
    store.save(2, b"second-updated").await.unwrap(); // upsert overwrites, not duplicates
    let range = store.get(1, 2).await.unwrap();
    assert_eq!(
        range,
        vec![(1, b"first".to_vec()), (2, b"second-updated".to_vec())]
    );

    store.reset().await.unwrap();
    assert_eq!(store.next_sender_seq().await.unwrap(), 1);
    assert_eq!(store.next_target_seq().await.unwrap(), 1);
    assert!(store.get(1, 2).await.unwrap().is_empty());
}

/// Two distinct `session_id`s sharing the same table pair don't see each other's data.
async fn exercise_session_isolation(url: &str, unique_suffix: &str) {
    let cfg = |session_id: &str| SqlStoreConfig {
        sessions_table: format!("iso_sessions_{unique_suffix}"),
        messages_table: format!("iso_messages_{unique_suffix}"),
        session_id: session_id.to_owned(),
        ..SqlStoreConfig::new(url)
    };
    let a = SqlStore::connect_with_config(cfg("A")).await.unwrap();
    let b = SqlStore::connect_with_config(cfg("B")).await.unwrap();

    a.set_next_sender_seq(100).await.unwrap();
    a.save(1, b"a-only").await.unwrap();

    assert_eq!(b.next_sender_seq().await.unwrap(), 1); // unaffected by A
    assert!(b.get(1, 1).await.unwrap().is_empty());
    assert_eq!(a.get(1, 1).await.unwrap(), vec![(1, b"a-only".to_vec())]);
}

#[tokio::test]
async fn sqlite_full_contract() {
    let dir = unique_dir();
    let url = format!("sqlite:{}/store.db", dir.display());
    exercise_full_contract(&url, "sqlite1").await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sqlite_session_isolation() {
    let dir = unique_dir();
    let url = format!("sqlite:{}/store.db", dir.display());
    exercise_session_isolation(&url, "sqlite2").await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn postgres_full_contract_if_available() {
    let Ok(url) = std::env::var("DATABASE_URL_PG") else {
        eprintln!("skipping: DATABASE_URL_PG not set");
        return;
    };
    exercise_full_contract(&url, "pg1").await;
}

#[tokio::test]
async fn postgres_session_isolation_if_available() {
    let Ok(url) = std::env::var("DATABASE_URL_PG") else {
        eprintln!("skipping: DATABASE_URL_PG not set");
        return;
    };
    exercise_session_isolation(&url, "pg2").await;
}

#[tokio::test]
async fn mysql_full_contract_if_available() {
    let Ok(url) = std::env::var("DATABASE_URL_MYSQL") else {
        eprintln!("skipping: DATABASE_URL_MYSQL not set");
        return;
    };
    exercise_full_contract(&url, "mysql1").await;
}

#[tokio::test]
async fn mysql_session_isolation_if_available() {
    let Ok(url) = std::env::var("DATABASE_URL_MYSQL") else {
        eprintln!("skipping: DATABASE_URL_MYSQL not set");
        return;
    };
    exercise_session_isolation(&url, "mysql2").await;
}
