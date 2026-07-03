//! T032 (US3, feature 006): `BodyLog::reset()` syncs when `FileStoreSync=Y` (BUG-15/FR-016), and
//! `FileStore`/`CachedFileStore::reset()` clear `seq` before `body` (B17/FR-016) so a crash
//! mid-reset cannot leave `seqnums` reporting advanced sequence numbers with no corresponding
//! `body` data (unrecoverable) -- only the reverse (harmless unindexed leftover `body` data with
//! `seqnums` already at the post-reset state) is possible.
//!
//! A true crash-mid-write can't be simulated from a black-box unit test without OS-level fault
//! injection; these tests verify the observable end-state (both files land correctly reset, with
//! sync enabled) as a baseline regression guard, and the ordering itself is verified by direct
//! source inspection (research.md §R3.5) plus the code's own comments at each call site.

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
    let dir = std::env::temp_dir().join(format!(
        "truefix-reset-ordering-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn file_store_reset_with_sync_enabled_lands_both_files_correctly_reset() {
    let dir = unique_dir();
    let opts = FileStoreOptions {
        sync: true,
        ..FileStoreOptions::default()
    };
    let s = FileStore::open_with_options(&dir, opts).unwrap();
    s.set_next_sender_seq(10).await.unwrap();
    s.set_next_target_seq(8).await.unwrap();
    s.save(9, b"stale").await.unwrap();
    s.reset().await.unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    assert_eq!(s.next_target_seq().await.unwrap(), 1);
    assert!(
        s.get(1, 100).await.unwrap().is_empty(),
        "reset must clear previously-stored messages"
    );
    cleanup(&dir);
}

#[tokio::test]
async fn cached_file_store_reset_with_sync_enabled_lands_both_files_correctly_reset() {
    let dir = unique_dir();
    let opts = FileStoreOptions {
        sync: true,
        ..FileStoreOptions::default()
    };
    let s = CachedFileStore::open_with_options(&dir, opts).unwrap();
    s.set_next_sender_seq(10).await.unwrap();
    s.set_next_target_seq(8).await.unwrap();
    s.save(9, b"stale").await.unwrap();
    s.reset().await.unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    assert_eq!(s.next_target_seq().await.unwrap(), 1);
    assert!(
        s.get(1, 100).await.unwrap().is_empty(),
        "reset must clear previously-stored messages"
    );
    cleanup(&dir);
}
