//! T141 — runtime dictionary loading rejects definitions that would overwrite an earlier entry.

use truefix_dict::{ParseError, parse};

#[test]
fn duplicate_field_tag_is_an_error() {
    let error = parse(
        "version FIX.TEST\n\
         field 100 First STRING\n\
         field 100 Second STRING\n",
    )
    .unwrap_err();

    assert_eq!(
        error,
        ParseError::DuplicateDefinition {
            line: 3,
            kind: "field tag",
            key: "100".to_owned(),
        }
    );
}

#[test]
fn duplicate_field_name_is_an_error() {
    let error = parse(
        "version FIX.TEST\n\
         field 100 SameName STRING\n\
         field 101 SameName STRING\n",
    )
    .unwrap_err();

    assert_eq!(
        error,
        ParseError::DuplicateDefinition {
            line: 3,
            kind: "field name",
            key: "SameName".to_owned(),
        }
    );
}

#[test]
fn duplicate_message_type_is_an_error() {
    let error = parse(
        "version FIX.TEST\n\
         field 100 Value STRING\n\
         message Z First opt:100\n\
         message Z Second opt:100\n",
    )
    .unwrap_err();

    assert_eq!(
        error,
        ParseError::DuplicateDefinition {
            line: 4,
            kind: "message type",
            key: "Z".to_owned(),
        }
    );
}
