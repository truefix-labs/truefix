use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{FileStore, MessageStore};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "truefix-bodylog-reset-lock-order-{}-{nanos}-{count}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn reset_acquires_the_index_lock_before_truncating_the_body_file() {
    let dir = unique_dir();
    let store = Arc::new(FileStore::open(&dir).unwrap());
    store.save(1, b"must-survive-a-failed-reset").await.unwrap();
    let body_path = dir.join("body");
    let original_len = std::fs::metadata(&body_path).unwrap().len();

    // A reversed BTreeMap range panics while BodyLog holds its index mutex. This deliberately
    // poisons that mutex so reset's ordering is observable: reset must fail on the lock before it
    // performs the irreversible file truncation.
    let poisoner = {
        let store = store.clone();
        tokio::spawn(async move {
            let _ = store.get(2, 1).await;
        })
    };
    assert!(poisoner.await.is_err(), "the index mutex must be poisoned");

    assert!(store.reset().await.is_err());
    assert_eq!(
        std::fs::metadata(&body_path).unwrap().len(),
        original_len,
        "a reset that cannot acquire the index lock must not truncate the body file"
    );

    cleanup(&dir);
}
