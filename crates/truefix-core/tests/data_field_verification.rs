//! T053/T054 (US2, feature 007): a length-prefixed data field (e.g. RawDataLength(95)/RawData(96))
//! is verified properly during decode:
//! - `BUG-38`: a length field followed by a *non-matching* tag (not its documented data-tag
//!   partner) must fail decoding cleanly, not silently consume `len` bytes from the wrong field's
//!   value (corrupting all subsequent parsing).
//! - `BUG-49`: a length field carrying a non-numeric value must fail decoding cleanly, not silently
//!   skip setting `pending_data_len` and let the following data field be misparsed as plain text.

use truefix_core::{decode, Field, Message};

fn base_message() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "n")); // XMLnonDictionary msgtype, body free-form for this test
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m
}

#[test]
fn a_length_field_followed_by_a_non_matching_tag_fails_to_decode() {
    let mut m = base_message();
    // RawDataLength(95) declares 5 bytes, but the field that actually follows is tag 99
    // (an ordinary string field), not RawData(96).
    m.body.set(Field::int(95, 5));
    m.body.set(Field::string(99, "hello-does-not-matter"));
    let bytes = m.encode();

    let result = decode(&bytes);
    assert!(
        result.is_err(),
        "a length field not immediately followed by its documented data-tag partner must fail \
         decoding, not silently consume bytes from the wrong field -- got {result:?}"
    );
}

/// A more adversarial construction proving genuine silent corruption (not just a decode error from
/// an accidental byte-count mismatch): a phantom length that exactly spans an embedded SOH plus an
/// entire following field's `tag=value`, swallowing it whole into tag 99's "value" with a raw SOH
/// byte inside it -- and, worse, skipping the swallowed tag (100) out of the decoded message
/// entirely, with no decode error raised at all.
#[test]
fn a_length_that_spans_an_embedded_soh_and_a_whole_following_field_does_not_silently_swallow_it() {
    let mut m = base_message();
    let swallowed = b"ab\x01100=cd".to_vec(); // 9 raw bytes, including one embedded SOH
    assert_eq!(swallowed.len(), 9);
    m.body.set(Field::int(95, 9)); // RawDataLength claims 9 bytes
    m.body.set(Field::new(99, swallowed)); // tag 99's on-the-wire value is exactly those 9 bytes
    let bytes = m.encode();

    let result = decode(&bytes);
    assert!(
        result.is_err(),
        "a length field must only apply to its documented data-tag partner (96 for 95), never to \
         an unrelated tag (99 here) -- letting it apply regardless silently swallows an embedded \
         SOH and an entire following field (tag 100) into tag 99's value with no error at all, \
         got {result:?}"
    );
}

#[test]
fn a_length_field_followed_by_its_matching_data_tag_decodes_normally() {
    let mut m = base_message();
    m.body.set(Field::int(95, 5));
    m.body.set(Field::string(96, "hello")); // exactly 5 bytes, correct partner tag
    let bytes = m.encode();

    let decoded = decode(&bytes).expect("a correctly-paired length/data field must decode");
    assert_eq!(
        decoded.body.get(96).and_then(|f| f.as_str().ok()),
        Some("hello")
    );
}

#[test]
fn a_non_numeric_length_field_value_fails_to_decode() {
    let mut m = base_message();
    m.body.set(Field::string(95, "abc")); // non-numeric RawDataLength
    m.body.set(Field::string(96, "hello"));
    let bytes = m.encode();

    let result = decode(&bytes);
    assert!(
        result.is_err(),
        "a non-numeric data-length field value must fail decoding, not silently skip setting the \
         pending length (letting the following data field be misparsed as plain text) -- got \
         {result:?}"
    );
}
