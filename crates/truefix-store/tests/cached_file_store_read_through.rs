use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{CachedFileStore, FileStoreOptions, MessageStore};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "truefix-cached-read-through-{}-{nanos}-{count}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn cache_miss_is_cached_for_the_next_resend() {
    let dir = unique_dir();
    let store = CachedFileStore::open_with_options(
        &dir,
        FileStoreOptions {
            sync: false,
            max_cached_msgs: 1,
        },
    )
    .unwrap();

    store.save(1, b"first").await.unwrap();
    store.save(2, b"second").await.unwrap();

    // Saving sequence 2 evicts sequence 1, so this first resend must fall back to disk.
    assert_eq!(store.get(1, 1).await.unwrap(), vec![(1, b"first".to_vec())]);

    // Removing the body file makes any subsequent disk read fail. A read-through cache hit must
    // still return the first message without touching the file.
    std::fs::remove_file(dir.join("body")).unwrap();
    assert_eq!(store.get(1, 1).await.unwrap(), vec![(1, b"first".to_vec())]);

    cleanup(&dir);
}
