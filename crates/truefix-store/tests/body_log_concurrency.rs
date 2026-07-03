//! T088 (US10, feature 006): `BodyLog::append`'s offset-determining `fs::metadata` read must
//! happen inside the same critical section as the write and index-insert, not before it
//! (BUG-20/FR-046) — otherwise concurrent `save()` callers can race to record the same
//! (stale-for-one-of-them) offset in the index, corrupting whichever message's `get()` reads back
//! from that wrong position.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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
        "truefix-body-log-concurrency-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_saves_all_round_trip_without_corrupting_each_others_offsets() {
    let dir = unique_dir();
    let store = Arc::new(FileStore::open(&dir).unwrap());

    const N: u64 = 64;
    let mut handles = Vec::new();
    for seq in 1..=N {
        let store = store.clone();
        // Distinct, easily-corruption-detectable per-message content and length (varying size
        // defeats a "got the right length, wrong bytes" false pass that a fixed-length payload
        // might hide).
        let msg = format!("MSG-{seq:03}-{}", "x".repeat(seq as usize % 17)).into_bytes();
        handles.push(tokio::spawn(async move {
            store.save(seq, &msg).await.unwrap();
            (seq, msg)
        }));
    }

    let mut expected = std::collections::BTreeMap::new();
    for h in handles {
        let (seq, msg) = h.await.unwrap();
        expected.insert(seq, msg);
    }

    let stored = store.get(1, N).await.unwrap();
    assert_eq!(
        stored.len(),
        N as usize,
        "every concurrently-saved sequence number must be retrievable"
    );
    for (seq, msg) in stored {
        assert_eq!(
            msg, expected[&seq],
            "sequence {seq}'s stored bytes must exactly match what was saved -- a mismatch here \
             means two concurrent appends raced onto the same recorded offset"
        );
    }

    cleanup(&dir);
}
