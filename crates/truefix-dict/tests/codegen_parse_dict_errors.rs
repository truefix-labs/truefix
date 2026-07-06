//! T120/T121 (US3, feature 009, `NEW-78`; `dict-tooling`): codegen's independent parser must
//! reject malformed and unknown directives just as the runtime parser does.

#[test]
fn codegen_rejects_the_same_malformed_field_as_the_runtime_parser() {
    let input = "version FIX.4.4\nfield not-a-tag Broken STRING\n";
    assert!(truefix_dict::parse(input).is_err(), "runtime control");
    assert!(
        truefix_dict::codegen::generate("BROKEN", input.as_bytes()).is_err(),
        "codegen must not silently skip a malformed field"
    );
}

#[test]
fn codegen_rejects_the_same_unknown_directive_as_the_runtime_parser() {
    let input = "version FIX.4.4\nfiled 54 Side CHAR 1=BUY\n";
    assert!(truefix_dict::parse(input).is_err(), "runtime control");
    assert!(
        truefix_dict::codegen::generate("BROKEN", input.as_bytes()).is_err(),
        "codegen must not silently ignore an unknown directive"
    );
}
