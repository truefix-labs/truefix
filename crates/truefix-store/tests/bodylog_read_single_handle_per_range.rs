//! T174/T176 (feature 009, NEW-41): `BodyLog::read`'s `range()` reuses a single file handle
//! across a multi-message resend instead of opening (and seeking) a fresh handle per message.

use truefix_store::{FileStore, MessageStore};

#[tokio::test]
async fn range_returns_every_message_correctly_across_multiple_entries() {
    let dir = std::env::temp_dir().join(format!(
        "truefix-bodylog-range-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let store = FileStore::open(&dir).unwrap();
    for seq in 1..=5u64 {
        store
            .save(seq, format!("body-{seq}").as_bytes())
            .await
            .unwrap();
    }
    let range = store.get(2, 4).await.unwrap();
    let bodies: Vec<(u64, String)> = range
        .into_iter()
        .map(|(seq, bytes)| (seq, String::from_utf8(bytes).unwrap()))
        .collect();
    assert_eq!(
        bodies,
        vec![
            (2, "body-2".to_owned()),
            (3, "body-3".to_owned()),
            (4, "body-4".to_owned()),
        ]
    );
}

/// `range()`'s per-message read must not call `File::open` in its own loop body — verified at the
/// source level (mirroring this codebase's established pattern, e.g.
/// `frame_length_uses_checked_add_not_bare_addition`, for internal-implementation-shape checks a
/// black-box test can't directly observe: there's no portable, non-flaky way to count OS-level
/// file-handle opens from a `#[tokio::test]`).
#[test]
fn range_opens_the_body_file_exactly_once_not_once_per_message() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/file.rs"))
        .expect("read src/file.rs");
    let fn_start = src.find("fn range(").expect("range() present");
    let fn_src = &src[fn_start..];
    let fn_end = fn_src.find("\n    fn reset(").unwrap_or(fn_src.len());
    let fn_src = &fn_src[..fn_end];

    assert_eq!(
        fn_src.matches("File::open").count(),
        1,
        "range() must open the body file exactly once for the whole range, not once per \
         message:\n{fn_src}"
    );
}
