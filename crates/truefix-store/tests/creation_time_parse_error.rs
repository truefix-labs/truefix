use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{FileStore, StoreError};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "truefix-creation-time-parse-error-{}-{nanos}-{count}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn present_but_unparseable_creation_time_is_a_typed_error() {
    let dir = unique_dir();
    std::fs::write(dir.join("session"), "not-a-unix-timestamp").unwrap();

    let error = FileStore::open(&dir)
        .err()
        .expect("a present but unparseable creation-time file must not silently become epoch");
    assert!(
        matches!(&error, StoreError::Backend(message) if message.contains("creation-time")),
        "expected a typed corrupted creation-time error, got {error}"
    );

    cleanup(&dir);
}
