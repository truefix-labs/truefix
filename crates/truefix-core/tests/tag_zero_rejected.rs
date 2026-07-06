//! T060 (US2, feature 009, `NEW-21`): tag `0` is not a valid FIX tag number (tags are strictly
//! positive) -- previously `tokenize`'s tag parsing accepted it like any other unsigned integer.

use truefix_core::{DecodeError, decode};

#[test]
fn a_field_with_tag_zero_is_rejected() {
    assert!(matches!(
        decode(b"8=FIX.4.2\x010=1\x01"),
        Err(DecodeError::InvalidTag { .. })
    ));
}
