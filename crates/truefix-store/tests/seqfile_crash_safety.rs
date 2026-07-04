//! T002/T003 (US1, feature 007): the sequence-number store's atomic-write path (BUG-30) and its
//! corrupted-vs-missing-file distinction (BUG-31).

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
        "truefix-seqfile-crash-safety-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn sequence_numbers_are_stored_in_two_separate_files() {
    let dir = unique_dir();
    let s = FileStore::open(&dir).unwrap();
    s.set_next_sender_seq(5).await.unwrap();
    s.set_next_target_seq(7).await.unwrap();
    assert!(
        dir.join("senderseqnums").is_file(),
        "sender sequence must be its own file"
    );
    assert!(
        dir.join("targetseqnums").is_file(),
        "target sequence must be its own file"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("senderseqnums"))
            .unwrap()
            .trim(),
        "5"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("targetseqnums"))
            .unwrap()
            .trim(),
        "7"
    );
    cleanup(&dir);
}

#[tokio::test]
async fn no_temp_file_survives_a_completed_write() {
    // BUG-30: the atomic write path uses a sibling `.tmp` file, renamed over the real path. After
    // a normal (non-crashed) write completes, no `.tmp` file should be left behind.
    let dir = unique_dir();
    let s = FileStore::open(&dir).unwrap();
    s.set_next_sender_seq(42).await.unwrap();
    assert!(!dir.join("senderseqnums.tmp").exists());
    cleanup(&dir);
}

#[tokio::test]
async fn a_present_but_corrupted_sequence_file_is_a_typed_error_not_a_silent_default() {
    // BUG-31: previously `unwrap_or(1)` made a corrupted file indistinguishable from a missing
    // one. Write garbage directly to senderseqnums, then confirm open() fails loudly.
    let dir = unique_dir();
    // Establish the store once so targetseqnums/body exist, then corrupt senderseqnums.
    {
        let s = FileStore::open(&dir).unwrap();
        s.set_next_target_seq(3).await.unwrap();
    }
    std::fs::write(dir.join("senderseqnums"), b"not-a-number\n").unwrap();

    let result = FileStore::open(&dir);
    match result {
        Ok(_) => panic!("a corrupted senderseqnums file must not open successfully"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("corrupted") || msg.contains("backend"),
                "expected a typed corruption error, got: {msg}"
            );
        }
    }
    cleanup(&dir);
}

#[tokio::test]
async fn a_missing_sequence_file_is_not_an_error_and_defaults_to_one() {
    let dir = unique_dir();
    let s = FileStore::open(&dir).unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    assert_eq!(s.next_target_seq().await.unwrap(), 1);
    cleanup(&dir);
}

#[tokio::test]
async fn reset_deletes_and_recreates_both_files() {
    let dir = unique_dir();
    let s = FileStore::open(&dir).unwrap();
    s.set_next_sender_seq(99).await.unwrap();
    s.set_next_target_seq(88).await.unwrap();
    s.reset().await.unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    assert_eq!(s.next_target_seq().await.unwrap(), 1);
    assert_eq!(
        std::fs::read_to_string(dir.join("senderseqnums"))
            .unwrap()
            .trim(),
        "1"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("targetseqnums"))
            .unwrap()
            .trim(),
        "1"
    );
    cleanup(&dir);
}
