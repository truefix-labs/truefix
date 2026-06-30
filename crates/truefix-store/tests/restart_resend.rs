//! T053 / T060 — restart-survivable resend and corrupted-store recovery.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{
    build_store, CachedFileStore, FileStore, MemoryStore, MessageStore, NoopStore, StoreConfig,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("truefix-store-{}-{nanos}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn memory_store_round_trips_within_session() {
    let s = MemoryStore::new();
    s.set_next_sender_seq(5).await.unwrap();
    s.save(2, b"msg2").await.unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 5);
    assert_eq!(s.get(1, 10).await.unwrap(), vec![(2, b"msg2".to_vec())]);
}

#[tokio::test]
async fn file_store_survives_restart() {
    let dir = unique_dir();
    {
        let s = FileStore::open(&dir).unwrap();
        s.set_next_sender_seq(7).await.unwrap();
        s.set_next_target_seq(3).await.unwrap();
        s.save(5, b"hello").await.unwrap();
        s.save(6, b"world").await.unwrap();
    }
    // reopen (simulated restart)
    let s2 = FileStore::open(&dir).unwrap();
    assert_eq!(s2.next_sender_seq().await.unwrap(), 7);
    assert_eq!(s2.next_target_seq().await.unwrap(), 3);
    assert_eq!(
        s2.get(5, 6).await.unwrap(),
        vec![(5, b"hello".to_vec()), (6, b"world".to_vec())]
    );
    cleanup(&dir);
}

#[tokio::test]
async fn cached_file_store_survives_restart() {
    let dir = unique_dir();
    {
        let s = CachedFileStore::open(&dir).unwrap();
        s.set_next_sender_seq(4).await.unwrap();
        s.save(1, b"a").await.unwrap();
        s.save(2, b"bb").await.unwrap();
    }
    let s2 = CachedFileStore::open(&dir).unwrap();
    assert_eq!(s2.next_sender_seq().await.unwrap(), 4);
    assert_eq!(s2.get(1, 2).await.unwrap().len(), 2);
    cleanup(&dir);
}

#[tokio::test]
async fn noop_store_is_inert() {
    let s = NoopStore;
    s.save(1, b"x").await.unwrap();
    s.set_next_sender_seq(9).await.unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    assert!(s.get(1, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn corrupted_trailing_record_is_recovered() {
    let dir = unique_dir();
    {
        let s = FileStore::open(&dir).unwrap();
        s.save(1, b"good").await.unwrap();
    }
    // Append an incomplete record header to simulate a torn write.
    let body = dir.join("body");
    let mut bytes = std::fs::read(&body).unwrap();
    bytes.extend_from_slice(&[9, 9, 9]);
    std::fs::write(&body, bytes).unwrap();

    let s2 = FileStore::open(&dir).unwrap();
    assert!(s2.was_corrupted(), "corruption should be detected");
    // the good prefix is still recovered (supports ForceResendWhenCorruptedStore)
    assert_eq!(s2.get(1, 1).await.unwrap(), vec![(1, b"good".to_vec())]);
    cleanup(&dir);
}

#[tokio::test]
async fn reset_clears_messages_and_seqs() {
    let s = MemoryStore::new();
    s.set_next_sender_seq(10).await.unwrap();
    s.save(3, b"x").await.unwrap();
    s.reset().await.unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    assert!(s.get(1, 100).await.unwrap().is_empty());
}

#[tokio::test]
async fn factory_builds_each_backend() {
    let dir = unique_dir();
    let configs = [
        StoreConfig::Memory,
        StoreConfig::Noop,
        StoreConfig::File { dir: dir.clone() },
        StoreConfig::CachedFile { dir: dir.clone() },
    ];
    for cfg in configs {
        let s = build_store(&cfg).await.unwrap();
        // every backend implements the trait
        let _ = s.next_sender_seq().await.unwrap();
    }
    cleanup(&dir);
}

#[cfg(feature = "sql")]
#[tokio::test]
async fn sql_store_survives_restart() {
    let dir = unique_dir();
    let url = format!("sqlite:{}/store.db", dir.display());
    {
        let s = truefix_store::SqlStore::connect(&url).await.unwrap();
        s.set_next_sender_seq(8).await.unwrap();
        s.save(3, b"sql-msg").await.unwrap();
    }
    let s2 = truefix_store::SqlStore::connect(&url).await.unwrap();
    assert_eq!(s2.next_sender_seq().await.unwrap(), 8);
    assert_eq!(s2.get(3, 3).await.unwrap(), vec![(3, b"sql-msg".to_vec())]);
    cleanup(&dir);
}
