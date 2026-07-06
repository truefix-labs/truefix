//! T175/T176 (feature 009, NEW-42): `RedbStore::reset()` must clear every stored message for the
//! session without collecting all its keys into an intermediate `Vec` first.

#![cfg(feature = "redb")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use truefix_store::{MessageStore, RedbStore, RedbStoreConfig};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "truefix-redb-reset-{}-{nanos}-{n}.redb",
        std::process::id()
    ))
}

/// Functional correctness: `reset()` still clears every message and resets both sequence
/// counters, regardless of how many messages were stored.
#[tokio::test]
async fn reset_clears_every_stored_message_and_resets_sequences() {
    let path = unique_path();
    let config = RedbStoreConfig {
        session_id: "s1".to_owned(),
        ..RedbStoreConfig::new(&path)
    };
    let store = RedbStore::connect_with_config(config).await.unwrap();

    for seq in 1..=20u64 {
        store
            .save(seq, format!("body-{seq}").as_bytes())
            .await
            .unwrap();
    }
    store.set_next_sender_seq(21).await.unwrap();
    store.set_next_target_seq(21).await.unwrap();

    store.reset().await.unwrap();

    assert_eq!(store.next_sender_seq().await.unwrap(), 1);
    assert_eq!(store.next_target_seq().await.unwrap(), 1);
    let remaining = store.get(1, 20).await.unwrap();
    assert!(
        remaining.is_empty(),
        "every message must be cleared by reset(), got {remaining:?}"
    );
}

/// `reset()` must not collect the session's message keys into an intermediate `Vec` before
/// removing them — verified at the source level (mirroring this codebase's established pattern
/// for internal-implementation-shape checks, e.g. `frame_length_uses_checked_add_not_bare_addition`),
/// since there's no portable, non-flaky way to observe peak allocation from a `#[tokio::test]`.
#[test]
fn reset_does_not_collect_keys_into_an_intermediate_vec() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/redb.rs"))
        .expect("read src/redb.rs");
    let fn_start = src.find("async fn reset(").expect("reset() present");
    let fn_src = &src[fn_start..];
    let fn_end = fn_src
        .find("\n    async fn creation_time(")
        .unwrap_or(fn_src.len());
    let fn_src = &fn_src[..fn_end];

    assert!(
        !fn_src.contains("Vec<"),
        "reset() must not collect keys into an intermediate Vec:\n{fn_src}"
    );
    assert!(
        fn_src.contains("retain_in"),
        "reset() should remove the session's message range directly via retain_in:\n{fn_src}"
    );
}
