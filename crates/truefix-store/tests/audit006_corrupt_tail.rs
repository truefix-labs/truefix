use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{FileStore, MessageStore};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "truefix-audit006-corrupt-tail-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn corrupt_tail_is_truncated_so_later_valid_appends_survive_reopen() {
    let dir = unique_dir();
    {
        let store = FileStore::open(&dir).unwrap();
        store.save(1, b"one").await.unwrap();
    }

    let body = dir.join("body");
    let mut bytes = std::fs::read(&body).unwrap();
    bytes.extend_from_slice(b"bad");
    std::fs::write(&body, bytes).unwrap();

    {
        let store = FileStore::open(&dir).unwrap();
        assert!(store.was_corrupted());
        store.save(2, b"two").await.unwrap();
        assert_eq!(
            store.get(1, 2).await.unwrap(),
            vec![(1, b"one".to_vec()), (2, b"two".to_vec())]
        );
    }

    let reopened = FileStore::open(&dir).unwrap();
    assert_eq!(
        reopened.get(1, 2).await.unwrap(),
        vec![(1, b"one".to_vec()), (2, b"two".to_vec())],
        "valid appends after corrupt-tail recovery must be indexed after restart"
    );

    let _ = std::fs::remove_dir_all(dir);
}
