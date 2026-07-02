//! T035 (US6) — `DataDictionary::load_from_file` (FR-010).

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use truefix_dict::{load_from_file, DictLoadError};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_temp(contents: &str) -> std::path::PathBuf {
    // A per-process atomic counter, not just a nanosecond timestamp: tests in this file run
    // concurrently on multiple threads within the same test binary, and two threads can observe
    // the same `SystemTime::now()` nanosecond reading, which previously caused one test's file to
    // clobber another's mid-run (a real, intermittent test-isolation bug — this fixes it).
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "truefix-load-from-file-{}-{n}.fixdict",
        std::process::id()
    ));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(contents.as_bytes()).unwrap();
    path
}

#[test]
fn loads_a_well_formed_dictionary() {
    let path = write_temp(
        "version FIX.4.4\n\
         header 8 9 35\n\
         trailer 10\n\
         field 11 ClOrdID STRING\n\
         message D NewOrderSingle req:11\n",
    );
    let dict = load_from_file(&path).unwrap();
    assert_eq!(dict.version(), "FIX.4.4");
    assert!(dict.message("D").is_some());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn missing_file_is_a_typed_io_error() {
    let missing = std::env::temp_dir().join("truefix-does-not-exist-12345.fixdict");
    let err = load_from_file(&missing).unwrap_err();
    assert!(matches!(err, DictLoadError::Io { .. }));
}

#[test]
fn malformed_contents_is_a_typed_parse_error() {
    let path = write_temp("this is not a valid directive\n");
    let err = load_from_file(&path).unwrap_err();
    assert!(matches!(err, DictLoadError::Parse { .. }));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn error_messages_name_the_offending_path() {
    let missing = std::env::temp_dir().join("truefix-does-not-exist-67890.fixdict");
    let err = load_from_file(&missing).unwrap_err();
    assert!(err
        .to_string()
        .contains("truefix-does-not-exist-67890.fixdict"));
}
