//! T070 (US12) — `CachedFileStore` maintains a real, bounded in-memory cache and honors the
//! `FileStoreSync` fsync toggle, while still surviving a restart (FR-025).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{CachedFileStore, FileStore, FileStoreOptions, MessageStore};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("truefix-cached-{}-{nanos}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn unbounded_cache_holds_every_saved_message() {
    let dir = unique_dir();
    let s = CachedFileStore::open(&dir).unwrap(); // default: max_cached_msgs = 0 (unbounded)
    for i in 1..=10u64 {
        s.save(i, format!("msg{i}").as_bytes()).await.unwrap();
    }
    assert_eq!(s.cached_len().unwrap(), 10);
    cleanup(&dir);
}

#[tokio::test]
async fn bounded_cache_evicts_oldest_but_get_still_returns_everything() {
    let dir = unique_dir();
    let options = FileStoreOptions {
        sync: true,
        max_cached_msgs: 3,
        max_body_records: 0,
    };
    let s = CachedFileStore::open_with_options(&dir, options).unwrap();
    for i in 1..=10u64 {
        s.save(i, format!("msg{i}").as_bytes()).await.unwrap();
    }
    // The cache only holds the most recent 3 messages...
    assert_eq!(s.cached_len().unwrap(), 3);
    // ...but `get` still returns every message, falling back to disk for evicted ones.
    let all = s.get(1, 10).await.unwrap();
    assert_eq!(all.len(), 10);
    assert_eq!(all[0], (1, b"msg1".to_vec()));
    assert_eq!(all[9], (10, b"msg10".to_vec()));
    cleanup(&dir);
}

#[tokio::test]
async fn cache_is_rewarmed_from_disk_on_restart() {
    let dir = unique_dir();
    {
        let s = CachedFileStore::open(&dir).unwrap();
        for i in 1..=5u64 {
            s.save(i, format!("m{i}").as_bytes()).await.unwrap();
        }
    }
    let s2 = CachedFileStore::open(&dir).unwrap();
    assert_eq!(s2.cached_len().unwrap(), 5);
    assert_eq!(s2.get(1, 5).await.unwrap().len(), 5);
    cleanup(&dir);
}

#[tokio::test]
async fn fsync_disabled_still_persists_and_survives_restart() {
    let dir = unique_dir();
    let options = FileStoreOptions {
        sync: false,
        max_cached_msgs: 0,
        max_body_records: 0,
    };
    {
        let s = FileStore::open_with_options(&dir, options).unwrap();
        s.set_next_sender_seq(9).await.unwrap();
        s.save(1, b"no-fsync").await.unwrap();
    }
    let s2 = FileStore::open_with_options(&dir, options).unwrap();
    assert_eq!(s2.next_sender_seq().await.unwrap(), 9);
    assert_eq!(s2.get(1, 1).await.unwrap(), vec![(1, b"no-fsync".to_vec())]);
    cleanup(&dir);
}

#[tokio::test]
async fn plain_file_store_reads_bodies_from_disk_not_memory() {
    // FileStore keeps only the offset index in memory, not message bytes; verify a message saved
    // in one handle is visible via a second, independently-opened handle over the same directory
    // (i.e. correctness doesn't depend on any process-local cache).
    let dir = unique_dir();
    let s1 = FileStore::open(&dir).unwrap();
    s1.save(1, b"from-disk").await.unwrap();
    let s2 = FileStore::open(&dir).unwrap();
    assert_eq!(
        s2.get(1, 1).await.unwrap(),
        vec![(1, b"from-disk".to_vec())]
    );
    cleanup(&dir);
}
