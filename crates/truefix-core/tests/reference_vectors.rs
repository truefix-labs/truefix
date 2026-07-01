//! T099 — reference wire vectors captured from QuickFIX/J test resources (data).
//!
//! These are reference *messages* (wire bytes) used to cross-validate that TrueFix's framing,
//! BodyLength, and CheckSum agree with QuickFIX/J on real data. `decode` verifies BodyLength and
//! CheckSum, so a successful decode of each vector is the cross-check, and re-encoding proves
//! byte-exact fidelity (FR-B9, SC-002).
//!
//! Coverage note: this captures the cleanly-verifiable vectors available in the QuickFIX/J test
//! resources. Expanding to the full canonical set across every FIX version is a follow-up that
//! builds QuickFIX/J to emit additional output vectors.

use truefix_core::{decode, encode};

const FIX44_LOGON_RAWDATA: &[u8] = include_bytes!("fixtures/reference/fix44_logon_rawdata.fix");
const FIX44_NOS_GROUP: &[u8] = include_bytes!("fixtures/reference/fix44_newordersingle_group.fix");

const ALL: &[&[u8]] = &[FIX44_LOGON_RAWDATA, FIX44_NOS_GROUP];

#[test]
fn reference_vectors_decode_and_validate() {
    // decode() validates BodyLength + CheckSum against the reference bytes.
    for v in ALL {
        assert!(
            decode(v).is_ok(),
            "reference vector failed to validate: {:?}",
            String::from_utf8_lossy(v)
        );
    }
}

#[test]
fn reference_vectors_roundtrip_byte_identical() {
    for v in ALL {
        let m = decode(v).unwrap();
        assert_eq!(encode(&m), *v, "{:?}", String::from_utf8_lossy(v));
    }
}

#[test]
fn rawdata_field_preserved_exactly() {
    let m = decode(FIX44_LOGON_RAWDATA).unwrap();
    assert_eq!(m.body.get(96).unwrap().value_bytes(), b"rawdata");
}
