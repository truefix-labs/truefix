//! T113/T114 (US3, feature 007; `--features dict-tooling`): the dictionary codegen path handles
//! three gaps versus the runtime parser's already-correct handling (BUG-72/73/74, FR-050):
//! - `TIME`/`PRICEOFFSET` field types get their real typed accessor, not the generic `&str`
//!   fallback (BUG-72).
//! - An enum-value label colliding with a Rust keyword (e.g. `Type`, `Self`) is sanitized, not
//!   emitted verbatim as an uncompilable identifier (BUG-73).
//! - A component/message member list using the `open` enum modifier has it stripped, not ingested
//!   as a literal (unlabeled) enum value — a dual-track divergence versus `parser.rs`, which
//!   already strips it (BUG-74).

const DICT: &str = "\
version FIX.4.4
field 8 BeginString STRING
field 9 BodyLength LENGTH
field 35 MsgType STRING
field 34 MsgSeqNum SEQNUM
field 49 SenderCompID STRING
field 56 TargetCompID STRING
field 52 SendingTime UTCTIMESTAMP
field 10 CheckSum STRING
field 60 TransactTime TIME
field 211 PegOffsetValue PRICEOFFSET
field 40 OrdType CHAR 1=Type 2=Self
field 167 SecurityType STRING open FUT OPT
message D NewOrderSingle req: opt:60,211,40,167
";

#[test]
fn time_and_priceoffset_get_real_typed_accessors_not_the_str_fallback() {
    let generated = truefix_dict::codegen::generate("TESTDICT_TIME", DICT.as_bytes())
        .expect("codegen should succeed for TIME/PRICEOFFSET fields");
    assert!(
        generated.contains("as_utc_timestamp") && generated.contains("time :: OffsetDateTime")
            || generated.contains("time::OffsetDateTime"),
        "TransactTime (TIME) must get the same typed accessor as UTCTIMESTAMP -- generated \
         code:\n{generated}"
    );
    assert!(
        generated.contains("rust_decimal :: Decimal")
            || generated.contains("rust_decimal::Decimal"),
        "PegOffsetValue (PRICEOFFSET) must get a Decimal accessor, not the &str fallback -- \
         generated code:\n{generated}"
    );
}

#[test]
fn a_keyword_colliding_enum_label_is_sanitized() {
    let generated = truefix_dict::codegen::generate("TESTDICT_KW", DICT.as_bytes())
        .expect("codegen should succeed for a keyword-colliding enum label");
    assert!(
        !generated.contains("    Type,") && !generated.contains("    Self,"),
        "a bare `Type`/`Self` enum variant would be uncompilable (Self is reserved) -- generated \
         code:\n{generated}"
    );
    assert!(
        generated.contains("VType") && generated.contains("VSelf"),
        "expected the keyword-colliding labels to be sanitized with the same V-prefix strategy \
         as the leading-digit case -- generated code:\n{generated}"
    );
}

/// BUG-74/FR-050: unlike the other two items in this file, this one has no observable effect on
/// `generate()`'s emitted text today, for any input -- `emit_field_enum` (the only consumer of
/// `FieldDef::values` anywhere in `codegen.rs`, confirmed by inspection) already filters to
/// *labeled* entries only (`filter_map(|(v, l)| l.as_deref()...)`), so a spurious unlabeled
/// `("open", None)` entry is silently dropped regardless of whether it's stripped beforehand. The
/// fix is still correct and worth keeping (a stray `"open"` polluting the parsed model is real
/// dual-track divergence versus `parser.rs`, and a future codegen feature could start consulting
/// unlabeled values too) — but proving it via `generate()`'s output would be testing a
/// non-difference. This is therefore a source-inspection test instead, mirroring this session's
/// established pattern for hardening whose trigger condition isn't observable via the available
/// test surface (e.g. `crates/truefix-core/tests/frame_checksum_overflow_hardening.rs`'s
/// `frame_length_uses_checked_add_not_bare_addition`).
#[test]
fn the_open_modifier_is_stripped_in_codegens_field_parsing() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/codegen.rs"))
        .expect("read codegen.rs");
    let field_arm_start = src
        .find("Some(\"field\") =>")
        .expect("the \"field\" match arm is present in parse_dict");
    let field_arm = &src[field_arm_start..];
    let field_arm_end = field_arm.find("Some(\"group\")").unwrap_or(field_arm.len());
    let field_arm = &field_arm[..field_arm_end];
    assert!(
        field_arm.contains("== Some(&\"open\")"),
        "codegen's \"field\" parsing must strip a leading `open` token the same way parser.rs's \
         runtime track does, got:\n{field_arm}"
    );
}
