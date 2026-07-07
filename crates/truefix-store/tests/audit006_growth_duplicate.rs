//! NEW-108: `FileStoreOptions::max_body_records` bounds a file-backed store's body-log *index*
//! to the most-recently-appended records, evicting the oldest once the bound is exceeded.
//! NEW-137: `MessageStore::contains` exposes duplicate-sequence detection without changing the
//! existing `save()` overwrite-on-conflict contract, for every backend.

use truefix_store::{CachedFileStore, FileStore, FileStoreOptions, MemoryStore, MessageStore};

fn temp_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "truefix-audit006-growthdup-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[tokio::test]
async fn audit006_memory_store_contains_reports_saved_sequences_only() {
    let store = MemoryStore::new();
    assert!(!store.contains(1).await.unwrap());
    store.save(1, b"msg-1").await.unwrap();
    assert!(store.contains(1).await.unwrap());
    assert!(!store.contains(2).await.unwrap());
}

#[tokio::test]
async fn audit006_file_store_contains_reports_saved_sequences_only() {
    let dir = temp_dir("filestore-contains");
    let store = FileStore::open(&dir).unwrap();
    assert!(!store.contains(1).await.unwrap());
    store.save(1, b"msg-1").await.unwrap();
    assert!(store.contains(1).await.unwrap());
    assert!(!store.contains(2).await.unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn audit006_file_store_bounds_body_index_to_max_records() {
    let dir = temp_dir("filestore-bounded");
    let opts = FileStoreOptions {
        max_body_records: 3,
        ..FileStoreOptions::default()
    };
    let store = FileStore::open_with_options(&dir, opts).unwrap();
    for seq in 1..=5u64 {
        store.save(seq, format!("msg-{seq}").as_bytes()).await.unwrap();
    }

    // Only the 3 most-recently-appended records (3, 4, 5) remain indexed.
    assert!(!store.contains(1).await.unwrap());
    assert!(!store.contains(2).await.unwrap());
    assert!(store.contains(3).await.unwrap());
    assert!(store.contains(4).await.unwrap());
    assert!(store.contains(5).await.unwrap());

    let got = store.get(1, 5).await.unwrap();
    let seqs: Vec<u64> = got.iter().map(|(s, _)| *s).collect();
    assert_eq!(seqs, vec![3, 4, 5]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn audit006_file_store_unbounded_by_default() {
    let dir = temp_dir("filestore-unbounded");
    let store = FileStore::open(&dir).unwrap();
    for seq in 1..=5u64 {
        store.save(seq, format!("msg-{seq}").as_bytes()).await.unwrap();
    }
    for seq in 1..=5u64 {
        assert!(store.contains(seq).await.unwrap());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn audit006_cached_file_store_contains_and_bounded_index_match_file_store() {
    let dir = temp_dir("cachedstore-bounded");
    let opts = FileStoreOptions {
        max_body_records: 2,
        ..FileStoreOptions::default()
    };
    let store = CachedFileStore::open_with_options(&dir, opts).unwrap();
    for seq in 1..=4u64 {
        store.save(seq, format!("msg-{seq}").as_bytes()).await.unwrap();
    }
    assert!(!store.contains(1).await.unwrap());
    assert!(!store.contains(2).await.unwrap());
    assert!(store.contains(3).await.unwrap());
    assert!(store.contains(4).await.unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}
