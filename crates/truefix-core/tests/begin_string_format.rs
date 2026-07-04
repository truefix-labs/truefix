//! T109/T110 (US3, feature 007): a buffer whose leading bytes don't form a recognizable
//! `BeginString` field (`FIX.\d\.\d`/`FIXT.\d\.\d`) is not treated as the start of a frame
//! (BUG-79, FR-048) — previously `frame_length` only checked the leading `8=` prefix, so any
//! bytes at all following it (including non-FIX data, or a garbled `BeginString` value) were
//! still accepted as a plausible frame start.

use truefix_core::framing::frame_length;

fn frame_with_begin_string(begin_string: &str) -> Vec<u8> {
    let body = b"35=0\x0134=1\x0149=A\x0156=B\x0152=20240101-00:00:00\x01";
    let mut buf = Vec::new();
    buf.extend_from_slice(format!("8={begin_string}\x01").as_bytes());
    buf.extend_from_slice(format!("9={}\x01", body.len()).as_bytes());
    buf.extend_from_slice(body);
    buf.extend_from_slice(b"10=000\x01");
    buf
}

#[test]
fn a_well_formed_fix_begin_string_frames_normally() {
    for bs in ["FIX.4.0", "FIX.4.2", "FIX.4.4", "FIX.5.0SP2", "FIXT.1.1"] {
        let buf = frame_with_begin_string(bs);
        assert!(
            matches!(frame_length(&buf), Ok(Some(_))),
            "{bs} should frame normally"
        );
    }
}

#[test]
fn non_fix_data_is_not_treated_as_a_frame_start() {
    // The BodyLength (5) and trailer position must be genuinely correct here -- otherwise a
    // mismatched BodyLength alone (BUG-46's own, separately-fixed checksum-position check) would
    // reject this frame regardless of whether the BeginString check under test fires at all,
    // making this test pass for the wrong reason.
    let mut buf = Vec::new();
    buf.extend_from_slice(b"8=NOTFIX\x01");
    buf.extend_from_slice(b"9=5\x01");
    buf.extend_from_slice(b"35=0\x01"); // exactly 5 bytes: "35=0" + SOH
    buf.extend_from_slice(b"10=000\x01");
    assert!(
        frame_length(&buf).is_err(),
        "a BeginString value that isn't FIX.\\d.\\d/FIXT.\\d.\\d must be rejected"
    );
}

#[test]
fn a_garbled_begin_string_value_is_rejected() {
    for garbled in ["FIX", "FIX.", "FIX.44", "FIXX.4.4", "4.4.4"] {
        let buf = frame_with_begin_string(garbled);
        assert!(
            frame_length(&buf).is_err(),
            "{garbled:?} must be rejected as a malformed BeginString"
        );
    }
}
