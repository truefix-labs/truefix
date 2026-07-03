//! T047 — the dual-track invariant: codegen and runtime derive from the same source.

use truefix_dict::{
    fix44_msgs, fix50_msgs, fixt11_msgs, load_fix44, load_fix50, load_fixt11, FIX44_DICT_HASH,
    FIX50_DICT_HASH, FIXT11_DICT_HASH,
};

#[test]
fn codegen_hash_matches_runtime_parsed_hash() {
    // build.rs hashed the dictionary source; the runtime parses the same source. Equality proves
    // the two tracks cannot diverge (Constitution Principle IV).
    assert_eq!(load_fix44().unwrap().hash(), FIX44_DICT_HASH);
    assert_eq!(load_fixt11().unwrap().hash(), FIXT11_DICT_HASH);
    assert_eq!(load_fix50().unwrap().hash(), FIX50_DICT_HASH);
}

#[test]
fn generated_msgtype_constants_present() {
    assert_eq!(fix44_msgs::LOGON, "A");
    assert_eq!(fix44_msgs::HEARTBEAT, "0");
    assert_eq!(fix44_msgs::NEWORDERSINGLE, "D");
    assert_eq!(fixt11_msgs::LOGON, "A");
    assert_eq!(fix50_msgs::NEWORDERSINGLE, "D");
}

// --- T067 (US9, feature 005): the dual-track hash-equality invariant, extended to the model
// fields FR-022–030 added. Most of the growth (open_enum, value_labels, field_order,
// version_meta, group child dictionaries) has no independent codegen-side representation to
// cross-check — `codegen_hash_matches_runtime_parsed_hash` above already proves the *source
// bytes* codegen hashes and the runtime parses are identical, which is the actual invariant
// Principle IV requires (both tracks consume the same source; neither can silently diverge in
// what it derives from it). `value_labels` is the one addition with a genuine second,
// independently-parsed representation — codegen's own per-field enum emission (`emit_field_enum`,
// pre-dating this feature) — so it gets a direct cross-track agreement check here. ---

#[test]
fn value_labels_agree_with_codegens_independently_parsed_field_enum() {
    use truefix_dict::fix44::Side;
    let d = load_fix44().unwrap();
    let side = d.field(54).unwrap(); // Side
    for (value, expected_label) in [("1", "BUY"), ("2", "SELL")] {
        assert_eq!(side.label(value), Some(expected_label));
        let parsed = Side::parse(value).unwrap_or_else(|| panic!("codegen Side::parse({value:?})"));
        assert_eq!(
            parsed.as_str(),
            value,
            "codegen track disagrees for {value:?}"
        );
    }
}
