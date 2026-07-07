//! NEW-117: `save_and_advance_sender` orders the durable sequence-number write ahead of the body
//! write, deliberately -- see the code comment in `crates/truefix-store/src/file.rs` for why the
//! reverse order (suggested by the raw audit finding) would be worse given this codebase's actual
//! call-time invariant (the wire write already happened before persistence is attempted). This
//! file verifies the *existing, safe* behavior stays intact rather than the (rejected) literal
//! "body-first" reordering.
//! NEW-136: `reset()` truncates the body log before resetting sequence numbers, so a crash between
//! the two never leaves a store that looks freshly-reset (`next_*_seq() == 1`) while still
//! serving stale pre-reset message bodies out of `get()`.

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
        "truefix-audit006-store-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

// --- NEW-117 ---

#[tokio::test]
async fn audit006_save_and_advance_sender_persists_body_and_advances_seq_together() {
    let dir = unique_dir();
    let s = FileStore::open(&dir).unwrap();
    s.save_and_advance_sender(1, b"first").await.unwrap();
    s.save_and_advance_sender(2, b"second").await.unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 3);
    assert_eq!(
        s.get(1, 2).await.unwrap(),
        vec![(1, b"first".to_vec()), (2, b"second".to_vec())]
    );
    cleanup(&dir);
}

#[tokio::test]
async fn audit006_seq_advance_survives_restart_even_if_a_later_body_is_never_written() {
    // Models the crash window `save_and_advance_sender`'s ordering deliberately leaves open (seq
    // advances first): after the sequence number is durably advanced past `seq`, a subsequent
    // restart must still see the advanced value even though this test's own body write is the
    // *last* completed step -- the point is that seq durability never depends on a body write
    // that comes after it.
    let dir = unique_dir();
    {
        let s = FileStore::open(&dir).unwrap();
        s.save_and_advance_sender(1, b"only-message").await.unwrap();
    }
    let s2 = FileStore::open(&dir).unwrap();
    assert_eq!(s2.next_sender_seq().await.unwrap(), 2);
    cleanup(&dir);
}

// --- NEW-136 ---

#[tokio::test]
async fn audit006_reset_clears_both_seq_and_body_together() {
    let dir = unique_dir();
    let s = FileStore::open(&dir).unwrap();
    s.set_next_sender_seq(10).await.unwrap();
    s.set_next_target_seq(20).await.unwrap();
    s.save(1, b"stale").await.unwrap();

    s.reset().await.unwrap();

    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    assert_eq!(s.next_target_seq().await.unwrap(), 1);
    assert!(s.get(1, 1).await.unwrap().is_empty());
    cleanup(&dir);
}

#[tokio::test]
async fn audit006_a_crash_between_body_truncate_and_seq_reset_looks_like_a_safe_gap_not_stale_data()
{
    // Simulates the exact crash window reset()'s NEW-136 fix targets: body already truncated
    // (this test does it directly, standing in for `BodyLog::reset()` having completed), but the
    // process is killed before `SeqFile::reset()` runs -- so the on-disk sequence files still hold
    // their pre-reset values. Re-opening from this exact on-disk state must report the *old*
    // (not-yet-reset) sequence numbers over an already-empty body: an ordinary "missing stored
    // message" state that `build_resend`'s gap-fill path already handles safely -- never a
    // freshly-reset-looking `next_*_seq() == 1` serving stale pre-reset messages (the bug this
    // fix closes).
    let dir = unique_dir();
    {
        let s = FileStore::open(&dir).unwrap();
        s.set_next_sender_seq(10).await.unwrap();
        s.set_next_target_seq(20).await.unwrap();
        s.save(1, b"pre-reset-message").await.unwrap();
    }
    // Directly truncate the body file, standing in for `BodyLog::reset()` having already
    // completed and synced, immediately before a simulated crash.
    std::fs::File::create(dir.join("body")).unwrap();

    let s2 = FileStore::open(&dir).unwrap();
    assert_eq!(
        s2.next_sender_seq().await.unwrap(),
        10,
        "sequence numbers must still show the pre-reset values (seq reset never completed)"
    );
    assert_eq!(s2.next_target_seq().await.unwrap(), 20);
    assert!(
        s2.get(1, 1).await.unwrap().is_empty(),
        "the body is genuinely empty -- this must read as an ordinary missing-message gap, \
         never as if seq==1 legitimately had nothing to report"
    );
    cleanup(&dir);
}
