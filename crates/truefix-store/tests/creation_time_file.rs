//! T048 (US7, feature 005) — `FileStore`/`CachedFileStore` persist and expose a creation
//! timestamp, stable across restarts, updated on `reset()` (GAP-38/FR-017).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{CachedFileStore, FileStore, MessageStore};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "truefix-creation-time-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn file_store_reports_a_creation_time() {
    let dir = unique_dir();
    let s = FileStore::open(&dir).unwrap();
    let ct = s.creation_time().await.unwrap();
    assert!(ct.is_some(), "expected a creation time to be recorded");
    cleanup(&dir);
}

#[tokio::test]
async fn file_store_creation_time_is_stable_across_restarts() {
    // Persisted with whole-second precision, so compare `unix_timestamp()` rather than exact
    // `OffsetDateTime` equality (the first, in-memory-only reading carries sub-second precision;
    // every subsequent reopen round-trips through the on-disk whole-second value).
    let dir = unique_dir();
    let first = {
        let s = FileStore::open(&dir).unwrap();
        s.creation_time().await.unwrap().unwrap()
    };
    let second = {
        let s = FileStore::open(&dir).unwrap();
        s.creation_time().await.unwrap().unwrap()
    };
    assert_eq!(
        first.unix_timestamp(),
        second.unix_timestamp(),
        "reopening the same store must not change its recorded creation time"
    );
    cleanup(&dir);
}

#[tokio::test]
async fn file_store_reset_updates_the_creation_time() {
    let dir = unique_dir();
    let s = FileStore::open(&dir).unwrap();
    let before = s.creation_time().await.unwrap().unwrap();
    // Force a later wall-clock second so the update is observable even on a fast filesystem.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    s.reset().await.unwrap();
    let after = s.creation_time().await.unwrap().unwrap();
    assert!(
        after > before,
        "reset() should update the creation time to now, matching QFJ/QFGo semantics"
    );
    cleanup(&dir);
}

#[tokio::test]
async fn cached_file_store_reports_and_persists_a_creation_time() {
    let dir = unique_dir();
    let first = {
        let s = CachedFileStore::open(&dir).unwrap();
        s.creation_time().await.unwrap().unwrap()
    };
    let second = {
        let s = CachedFileStore::open(&dir).unwrap();
        s.creation_time().await.unwrap().unwrap()
    };
    assert_eq!(first.unix_timestamp(), second.unix_timestamp());
    cleanup(&dir);
}
