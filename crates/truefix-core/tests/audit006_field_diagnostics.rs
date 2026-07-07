//! NEW-138: `Field::as_bool`'s error diagnostic preserves an invalid value losslessly -- a valid
//! UTF-8 (but non-Y/N) value is reported as-is, and genuinely invalid UTF-8 falls back to a hex
//! representation of the raw bytes, rather than `String::from_utf8_lossy`'s replacement
//! characters silently discarding the actual invalid bytes a caller might need to see.

use truefix_core::Field;

#[test]
fn audit006_as_bool_accepts_y_and_n() {
    assert!(Field::string(42, "Y").as_bool().unwrap());
    assert!(!Field::string(42, "N").as_bool().unwrap());
}

#[test]
fn audit006_as_bool_error_preserves_valid_utf8_value_verbatim() {
    let err = Field::string(42, "true").as_bool().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("true"),
        "diagnostic should include the actual (valid-UTF-8) offending value, got: {msg}"
    );
}

#[test]
fn audit006_as_bool_error_hex_encodes_invalid_utf8_instead_of_lossy_replacement() {
    // 0xFF is never valid as the start of a UTF-8 sequence.
    let field = Field::new(42, vec![0xFF, 0xFE]);
    let err = field.as_bool().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("fffe"),
        "diagnostic should hex-encode the raw invalid bytes, got: {msg}"
    );
    assert!(
        !msg.contains('\u{FFFD}'),
        "diagnostic must not use lossy UTF-8 replacement characters, got: {msg}"
    );
}
