//! T047/T048 (US1, feature 009, `NEW-07`): `FieldDef::allows()` did not check `MultipleCharValue`
//! per space-delimited token like `MultipleValueString`/`MultipleStringValue` do -- it fell into
//! the `_` whole-string-match arm, so a valid multi-token wire value (e.g. `"A B"`, two single
//! chars each individually allowed) was incorrectly rejected. `value_ok` already splits
//! `MultipleCharValue` per-token, an inconsistency between format-checking and enum-checking this
//! fix closes.

use std::collections::BTreeMap;

use truefix_dict::{FieldDef, FieldType};

fn multi_char_field(values: &[&str]) -> FieldDef {
    FieldDef {
        tag: 277, // an arbitrary MultipleCharValue-typed tag for this test
        name: "TestMultiChar".to_owned(),
        field_type: FieldType::MultipleCharValue,
        values: values.iter().map(|s| s.to_string()).collect(),
        open_enum: false,
        value_labels: BTreeMap::new(),
    }
}

#[test]
fn a_valid_multi_token_wire_value_is_allowed() {
    let field = multi_char_field(&["A", "B", "C"]);
    assert!(
        field.allows("A B"),
        "a wire value of two individually-allowed single characters must be allowed (NEW-07)"
    );
    assert!(field.allows("A"));
    assert!(field.allows("A B C"));
}

#[test]
fn a_token_not_in_the_allowed_set_is_rejected() {
    let field = multi_char_field(&["A", "B", "C"]);
    assert!(
        !field.allows("A Z"),
        "a token not in the allowed set must still be rejected"
    );
}
