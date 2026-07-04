//! T073/T074 (US2, feature 007): opening a `FileStore`/`CachedFileStore` against a long message
//! history does not read every stored message body into memory just to rebuild the offset index
//! (`FileStore`, BUG-45) or warm the cache (`CachedFileStore`, BUG-81) — QFJ's `FileStore` reads
//! only a compact offset index and never touches the body file at open time; `CachedFileStore`
//! never reads more bodies than its own `max_cached_msgs` bound.
//!
//! **Testing approach**: verifying an exact byte-for-byte memory bound isn't practical without
//! either OS-level introspection or a custom tracking global allocator -- the latter requires
//! `unsafe impl GlobalAlloc`, forbidden outright by this workspace's `unsafe_code = "forbid"` lint
//! (`Cargo.toml`'s `[workspace.lints.rust]`, inherited by every crate's `[lints] workspace = true`,
//! including test targets). Instead, this shells out to `ps -o rss=` (portable across the
//! macOS/Linux environments this project runs on) to observe this process's own resident set size
//! before and after opening a store backed by a genuinely large (tens of MB) body-log file
//! constructed directly on disk (bypassing the store's own slow per-message `append()` API purely
//! for test speed) — before the fix, opening reads the entire body file into memory at once
//! (`FileStore`) or reads every body before any cache eviction happens (`CachedFileStore`), so RSS
//! growth tracks the file size; after the fix, it stays orders of magnitude smaller. Thresholds are
//! deliberately generous (a large multiple of the expected bound) to avoid flakiness from
//! unrelated allocator/OS noise while still clearly separating "leaked the whole file" from "didn't".

use std::io::Write;
use std::path::Path;

use truefix_store::{CachedFileStore, FileStore};

fn resident_set_size_kb() -> u64 {
    let pid = std::process::id().to_string();
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .expect("ps must be available to measure RSS");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

/// Writes `count` synthetic records of `body_len` bytes each directly to `dir/body`, in the exact
/// on-disk format `BodyLog::append` itself produces (`seq: u64 LE`, `len: u32 LE`, then `len`
/// bytes) -- bypassing the store's own API entirely, purely so constructing a large fixture is
/// fast (a real `append()` call per record would dominate the test's runtime otherwise).
fn write_large_body_log(dir: &Path, count: u64, body_len: usize) {
    std::fs::create_dir_all(dir).unwrap();
    let mut f = std::fs::File::create(dir.join("body")).unwrap();
    let payload = vec![b'x'; body_len];
    for seq in 1..=count {
        f.write_all(&seq.to_le_bytes()).unwrap();
        f.write_all(&(body_len as u32).to_le_bytes()).unwrap();
        f.write_all(&payload).unwrap();
    }
}

fn unique_dir(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "truefix-store-membound-{name}-{}-{nanos}",
        std::process::id()
    ));
    dir
}

const RECORD_COUNT: u64 = 60_000;
const RECORD_BODY_LEN: usize = 800; // ~48 MB total body-log file
const GENEROUS_BOUND_KB: u64 = 8 * 1024; // 8 MiB -- well under the ~48 MiB file, well above noise

#[test]
fn opening_a_file_store_against_a_large_history_does_not_materialize_every_body() {
    let dir = unique_dir("filestore");
    write_large_body_log(&dir, RECORD_COUNT, RECORD_BODY_LEN);

    let before = resident_set_size_kb();
    let store = FileStore::open(&dir).expect("open must succeed against the large history");
    let after = resident_set_size_kb();

    let grew_by = after.saturating_sub(before);
    assert!(
        grew_by < GENEROUS_BOUND_KB,
        "opening FileStore against a ~{}MB body-log file grew RSS by {grew_by}KB, expected well \
         under {GENEROUS_BOUND_KB}KB -- the entire body file appears to have been read into \
         memory just to rebuild the offset index",
        RECORD_COUNT * RECORD_BODY_LEN as u64 / 1_000_000
    );

    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_cached_file_store_with_a_small_bound_does_not_read_every_body_to_warm_it() {
    let dir = unique_dir("cachedfilestore");
    write_large_body_log(&dir, RECORD_COUNT, RECORD_BODY_LEN);

    let before = resident_set_size_kb();
    let store = CachedFileStore::open_with_options(
        &dir,
        truefix_store::FileStoreOptions {
            sync: false,
            max_cached_msgs: 10, // tiny bound -- only ~10 bodies' worth should ever be read
        },
    )
    .expect("open must succeed against the large history");
    let after = resident_set_size_kb();

    let grew_by = after.saturating_sub(before);
    assert!(
        grew_by < GENEROUS_BOUND_KB,
        "opening CachedFileStore(max_cached_msgs=10) against a ~{}MB body-log file grew RSS by \
         {grew_by}KB, expected well under {GENEROUS_BOUND_KB}KB -- every body appears to have been \
         read before cache eviction ever kicked in, not just the most-recent {} allowed",
        RECORD_COUNT * RECORD_BODY_LEN as u64 / 1_000_000,
        10
    );
    assert_eq!(
        store.cached_len().unwrap(),
        10,
        "the cache should still end up holding exactly max_cached_msgs entries"
    );

    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}
