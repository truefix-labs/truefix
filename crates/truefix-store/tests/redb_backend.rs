//! T024 (US5, feature 004) — `RedbStore` parity (FR-006/007), mirroring `sql_backends.rs`'s
//! pattern. Unconditional — `redb` is embedded (no external service needed), like SQLite today.

#![cfg(feature = "redb")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{MessageStore, RedbStore, RedbStoreConfig};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "truefix-redb-{}-{nanos}-{n}.redb",
        std::process::id()
    ))
}

/// Exercises the full `MessageStore` contract against a fresh, uniquely-named file each call.
async fn exercise_full_contract() {
    let path = unique_path();
    let config = RedbStoreConfig {
        session_id: "s1".to_owned(),
        ..RedbStoreConfig::new(&path)
    };
    let store = RedbStore::connect_with_config(config).await.unwrap();

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

    let _ = std::fs::remove_file(&path);
}

/// Sequence numbers and message bodies persist across a process restart against the same file (a
/// fresh `RedbStore` connection, simulating a restart).
async fn exercise_persistence_across_restart() {
    let path = unique_path();
    {
        let store = RedbStore::connect(&path).await.unwrap();
        store.set_next_sender_seq(5).await.unwrap();
        store.save(1, b"persisted").await.unwrap();
    }
    // Reopen — a fresh connection against the same file, simulating a restart.
    let reopened = RedbStore::connect(&path).await.unwrap();
    assert_eq!(reopened.next_sender_seq().await.unwrap(), 5);
    assert_eq!(
        reopened.get(1, 1).await.unwrap(),
        vec![(1, b"persisted".to_vec())]
    );
    let _ = std::fs::remove_file(&path);
}

/// Two distinct `session_id`s sharing the same file don't see each other's data.
///
/// Unlike `SqlStore`/`MssqlStore` (networked, independent connections), `redb` takes an exclusive
/// file lock per `Database::create`/`open` call — a second, independent `connect_with_config`
/// against the same path fails with lock contention (real, confirmed `redb` behavior, not a
/// TrueFix bug). Two sessions sharing one file must instead share the already-open handle via
/// `RedbStore::with_session_id`.
async fn exercise_session_isolation() {
    let path = unique_path();
    let a = RedbStore::connect_with_config(RedbStoreConfig {
        session_id: "A".to_owned(),
        ..RedbStoreConfig::new(&path)
    })
    .await
    .unwrap();
    let b = a.with_session_id("B");

    a.set_next_sender_seq(100).await.unwrap();
    a.save(1, b"a-only").await.unwrap();

    assert_eq!(b.next_sender_seq().await.unwrap(), 1); // unaffected by A
    assert!(b.get(1, 1).await.unwrap().is_empty());
    assert_eq!(a.get(1, 1).await.unwrap(), vec![(1, b"a-only".to_vec())]);

    let _ = std::fs::remove_file(&path);
}

/// T048/T049 (US7, feature 005): creation-time persistence (stable across restarts, updated on
/// reset) and atomic save+advance-sender (GAP-38/GAP-39; FR-017/FR-018).
async fn exercise_creation_time_and_atomic_save() {
    let path = unique_path();
    let first = {
        let store = RedbStore::connect(&path).await.unwrap();
        store.creation_time().await.unwrap().unwrap()
    };
    let second = {
        let store = RedbStore::connect(&path).await.unwrap();
        store.creation_time().await.unwrap().unwrap()
    };
    assert_eq!(
        first.unix_timestamp(),
        second.unix_timestamp(),
        "reopening the same file must not change the recorded creation time"
    );

    let store = RedbStore::connect(&path).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    store.reset().await.unwrap();
    let after_reset = store.creation_time().await.unwrap().unwrap();
    assert!(
        after_reset.unix_timestamp() > second.unix_timestamp(),
        "reset() should advance the recorded creation time"
    );

    store.set_next_sender_seq(5).await.unwrap();
    store.save_and_advance_sender(5, b"atomic").await.unwrap();
    assert_eq!(store.next_sender_seq().await.unwrap(), 6);
    assert_eq!(
        store.get(5, 5).await.unwrap(),
        vec![(5, b"atomic".to_vec())]
    );

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn redb_full_contract() {
    exercise_full_contract().await;
}

#[tokio::test]
async fn redb_creation_time_and_atomic_save() {
    exercise_creation_time_and_atomic_save().await;
}

#[tokio::test]
async fn redb_persists_across_restart() {
    exercise_persistence_across_restart().await;
}

#[tokio::test]
async fn redb_session_isolation() {
    exercise_session_isolation().await;
}
