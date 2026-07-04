//! T004 (US1, feature 007): legacy-single-file-to-two-file sequence-number store migration
//! (BUG-99, FR-001a).

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
        "truefix-seqfile-migration-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_legacy_combined_file_migrates_transparently_on_open() {
    let dir = unique_dir();
    // Construct a store directory containing only the legacy pre-007 combined file.
    std::fs::write(dir.join("seqnums"), "42\n17\n").unwrap();

    let s = FileStore::open(&dir).unwrap();
    assert_eq!(
        s.next_sender_seq().await.unwrap(),
        42,
        "sender value must be read from the legacy file"
    );
    assert_eq!(
        s.next_target_seq().await.unwrap(),
        17,
        "target value must be read from the legacy file"
    );

    assert!(
        dir.join("senderseqnums").is_file(),
        "migration must produce senderseqnums"
    );
    assert!(
        dir.join("targetseqnums").is_file(),
        "migration must produce targetseqnums"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("senderseqnums"))
            .unwrap()
            .trim(),
        "42"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("targetseqnums"))
            .unwrap()
            .trim(),
        "17"
    );
    assert!(
        dir.join("seqnums").is_file(),
        "the legacy file must be left in place, not deleted, so a downgrade still finds it"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("seqnums")).unwrap(),
        "42\n17\n",
        "the legacy file's own content must be untouched"
    );

    cleanup(&dir);
}

#[tokio::test]
async fn migration_never_appears_as_a_sequence_reset() {
    // The upgrade itself must never look like a fresh store to a caller who only observes
    // next_sender_seq()/next_target_seq() — this is the exact SC-002 concern this feature exists
    // to close.
    let dir = unique_dir();
    std::fs::write(dir.join("seqnums"), "100\n200\n").unwrap();
    let s = FileStore::open(&dir).unwrap();
    assert_ne!(s.next_sender_seq().await.unwrap(), 1);
    assert_eq!(s.next_sender_seq().await.unwrap(), 100);
    assert_eq!(s.next_target_seq().await.unwrap(), 200);
    cleanup(&dir);
}

#[tokio::test]
async fn a_partially_migrated_directory_completes_migration_for_the_missing_file_only() {
    // Simulates a crash mid-migration: senderseqnums already migrated, targetseqnums not yet.
    let dir = unique_dir();
    std::fs::write(dir.join("seqnums"), "5\n6\n").unwrap();
    std::fs::write(dir.join("senderseqnums"), "999\n").unwrap(); // already-migrated, distinct value

    let s = FileStore::open(&dir).unwrap();
    assert_eq!(
        s.next_sender_seq().await.unwrap(),
        999,
        "an already-migrated senderseqnums must not be overwritten from the legacy file"
    );
    assert_eq!(
        s.next_target_seq().await.unwrap(),
        6,
        "targetseqnums must still be migrated from the legacy file"
    );
    cleanup(&dir);
}

#[tokio::test]
async fn no_legacy_file_present_means_a_fresh_store() {
    let dir = unique_dir();
    let s = FileStore::open(&dir).unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    assert_eq!(s.next_target_seq().await.unwrap(), 1);
    assert!(!dir.join("seqnums").exists());
    cleanup(&dir);
}
