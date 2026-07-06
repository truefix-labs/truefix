//! FR-048 (US3, feature 009, `NEW-81`): `FieldMap::set` only replaces the *first* matching field
//! it finds and returns, leaving any further stale duplicate copies of that tag in place (e.g. if
//! `add_field` was used to push duplicates directly, bypassing `set`'s usual dedup). `set` must
//! remove ALL existing copies of the tag, leaving exactly one -- the new value.

use truefix_core::{Field, FieldMap};

#[test]
fn set_removes_all_stale_duplicate_copies_of_the_tag() {
    let mut fm = FieldMap::new();
    // `add_field` bypasses `set`'s dedup, so this leaves three copies of tag 58 present.
    fm.add_field(Field::string(58, "first"));
    fm.add_field(Field::string(58, "second"));
    fm.add_field(Field::string(58, "third"));

    fm.set(Field::string(58, "final"));

    let copies: Vec<&str> = fm
        .fields()
        .filter(|f| f.tag() == 58)
        .map(|f| f.as_str().expect("string field"))
        .collect();
    assert_eq!(
        copies,
        vec!["final"],
        "set() must leave exactly one copy of tag 58 (the new value), not the new value plus \
         stale duplicates"
    );
}
