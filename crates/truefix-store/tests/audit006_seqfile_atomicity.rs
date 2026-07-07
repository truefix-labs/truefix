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
        "truefix-audit006-seqfile-atomicity-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn failed_sequence_file_write_does_not_advance_in_memory_sender_seq() {
    let dir = unique_dir();
    let store = FileStore::open(&dir).unwrap();
    store.set_next_sender_seq(7).await.unwrap();
    std::fs::create_dir(dir.join("senderseqnums.tmp")).unwrap();

    let result = store.set_next_sender_seq(8).await;
    assert!(
        result.is_err(),
        "the blocked temp path should make persistence fail"
    );
    assert_eq!(
        store.next_sender_seq().await.unwrap(),
        7,
        "in-memory sender seq must change only after the durable write succeeds"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn failed_sequence_file_write_does_not_advance_in_memory_target_seq() {
    let dir = unique_dir();
    let store = FileStore::open(&dir).unwrap();
    store.set_next_target_seq(9).await.unwrap();
    std::fs::create_dir(dir.join("targetseqnums.tmp")).unwrap();

    let result = store.set_next_target_seq(10).await;
    assert!(
        result.is_err(),
        "the blocked temp path should make persistence fail"
    );
    assert_eq!(
        store.next_target_seq().await.unwrap(),
        9,
        "in-memory target seq must change only after the durable write succeeds"
    );

    let _ = std::fs::remove_dir_all(dir);
}
