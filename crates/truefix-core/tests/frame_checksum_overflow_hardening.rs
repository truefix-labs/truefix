//! T083-T086 (US3, feature 007): defense-in-depth against integer overflow in two spots the
//! 2026-07-03 second-pass audit flagged (BUG-23/BUG-24, FR-032). Both are effectively unreachable
//! via the normal transport path today (`MAX_BODY_LEN`, feature 006's BUG-13 fix, already caps any
//! wire-decoded `BodyLength` well below where either overflow could occur) — but neither guarantee
//! held before that unrelated fix landed, and `encode()`/`Message::decode()` are also public APIs
//! callable directly with arbitrary/programmatically-built data that bypasses that cap entirely.
//! Fixing the arithmetic itself, rather than relying on the incidental upstream cap, is what "in
//! principle" hardening means here.

use truefix_core::{Field, Message};

/// BUG-23/FR-032: `frame_length`'s `body_start + body_len + 7` computation must use `checked_add`,
/// not a bare `+` that could silently wrap in a release build. Genuinely forcing this add to
/// overflow would require a buffer many exabytes long (`usize::MAX` bytes) -- not something any
/// test can actually allocate -- so this is a source-level check that the hardening is present,
/// mirroring this session's other tests for changes whose triggering condition is unreachable via
/// any real, allocatable input (e.g. `reconnecting_socket_options.rs`).
#[test]
fn frame_length_uses_checked_add_not_bare_addition() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/framing.rs"))
        .expect("read framing.rs");
    let fn_start = src
        .find("pub fn frame_length")
        .expect("frame_length function present");
    let fn_src = &src[fn_start..];
    let fn_end = fn_src.find("\nfn find(").unwrap_or(fn_src.len());
    let fn_src = &fn_src[..fn_end];

    assert!(
        fn_src.contains("checked_add"),
        "frame_length must use checked_add for its body_start/total arithmetic \
         (BUG-23/FR-032), got:\n{fn_src}"
    );
    assert!(
        !fn_src.contains("body_start + body_len + 7"),
        "the old unguarded bare-addition form must not remain"
    );
}

/// Independently recompute the FIX checksum (sum of bytes mod 256) over `bytes`, using plain `u8`
/// wrapping arithmetic -- a different code path than either `encode.rs`/`decode.rs`'s `u64`-sum
/// fix, so this is a real independent check, not a tautology against the implementation under
/// test.
fn reference_checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

/// BUG-24/FR-032: `encode()`'s checksum accumulator must not overflow `u32` (which panics in debug
/// builds, violating the crate's "no path panics" invariant) for a message whose encoded byte
/// length is large enough that the byte-value sum exceeds `u32::MAX` -- reachable because
/// `encode()` has no body-length cap of its own. A message with ~17,000,000 bytes of value 0xFE in
/// one big Data field comfortably exceeds `u32::MAX` (4,294,967,295) when summed (17,000,000 *
/// 254 ~= 4.318 billion).
#[test]
fn encode_checksum_does_not_overflow_or_panic_for_a_very_large_message() {
    let mut msg = Message::new();
    msg.header.set(Field::string(8, "FIX.4.4"));
    msg.header.set(Field::string(35, "0"));
    let huge = vec![0xFEu8; 17_000_000];
    msg.body.set(Field::bytes(96, &huge));

    // Must not panic (the pre-fix `u32` accumulator would, in a debug build, on this input).
    let encoded = msg.encode();

    // The trailer's CheckSum (last 7 bytes: "10=XXX\x01") must equal an independently-computed
    // reference, not a wrapped/incorrect value.
    let trailer_start = encoded.len() - 7;
    let checksum_field = std::str::from_utf8(&encoded[trailer_start..]).unwrap();
    assert!(
        checksum_field.starts_with("10="),
        "expected the last field to be CheckSum, got {checksum_field:?}"
    );
    let declared: u8 = checksum_field[3..6].parse().unwrap();

    let expected = reference_checksum(&encoded[..trailer_start]);
    assert_eq!(
        declared, expected,
        "encode()'s checksum must match an independently-computed mod-256 sum even for a message \
         whose byte-value sum exceeds u32::MAX"
    );
}

/// BUG-24/FR-032: `Message::decode`'s checksum verification (`decode.rs`) has the identical
/// overflow-prone sum, fixed the same way for parity. Previously verified by round-tripping a
/// ~17,000,000-byte message through `encode()` then `decode()` -- deliberately exceeding
/// `MAX_BODY_LEN` (16,777,216), since `decode()` had no cap of its own to bump into. T168/T169
/// (feature 009, NEW-04) closed exactly that gap: `decode()` now rejects any declared BodyLength
/// beyond `MAX_BODY_LEN` before ever reaching the checksum sum, so a message large enough to
/// overflow `u32` (needs ~16,909,321 bytes at value 0xFE) can no longer reach `decode()`'s
/// checksum computation at all -- the overflow-prone sum is now doubly unreachable via any real,
/// allocatable input. Converted to a source-level check (mirroring
/// `frame_length_uses_checked_add_not_bare_addition` above, established for this exact
/// situation) confirming the `u64` accumulator fix is still present as defense-in-depth for
/// direct internal callers of the checksum-computing code.
#[test]
fn decode_checksum_sum_still_uses_a_u64_accumulator() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/codec/decode.rs"))
        .expect("read codec/decode.rs");
    let fn_start = src
        .find("fn tokenize_validated")
        .expect("tokenize_validated function present");
    let fn_src = &src[fn_start..];

    assert!(
        fn_src.contains("u64::from(b)"),
        "tokenize_validated's checksum sum must accumulate in u64, not a narrower type prone to \
         overflow (BUG-24/FR-032), got:\n{fn_src}"
    );
}
