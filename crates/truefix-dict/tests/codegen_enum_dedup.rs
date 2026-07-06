//! T122/T123 (US3, feature 009, `NEW-79`; `dict-tooling`): enum labels that sanitize to the same
//! Rust identifier must receive unique generated variant names.

const DICT: &str = "\
version FIX.4.4
field 54 Side CHAR 1=SAME 2=SAME
message D NewOrderSingle req:54 opt:
";

#[test]
fn colliding_sanitized_enum_variants_are_made_unique() {
    let generated = truefix_dict::codegen::generate("COLLISION", DICT.as_bytes())
        .expect("crafted dictionary should generate");

    assert_eq!(
        generated.matches("    SAME,").count(),
        1,
        "the original sanitized variant may be emitted only once:\n{generated}"
    );
    assert!(
        generated.contains("    SAME_2,"),
        "the colliding label must receive a stable unique suffix:\n{generated}"
    );
    assert!(
        generated.contains("Self::SAME => \"1\"")
            && generated.contains("Self::SAME_2 => \"2\"")
            && generated.contains("\"1\" => Some(Self::SAME)")
            && generated.contains("\"2\" => Some(Self::SAME_2)"),
        "both wire values must retain distinct round-trip mappings:\n{generated}"
    );
}
