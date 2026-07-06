//! T143 — a STRING-typed enum value literal containing `#` is not truncated by `strip_comment`.

use truefix_dict::parse;

#[test]
fn hash_inside_a_value_token_is_not_treated_as_a_comment_start() {
    let dict = parse(
        "version FIX.TEST\n\
         field 100 TestField STRING SYM#BOL OTHER\n",
    )
    .unwrap();

    let field = dict.field(100).expect("field 100 must be defined");
    assert_eq!(field.values, vec!["SYM#BOL".to_owned(), "OTHER".to_owned()]);
}

#[test]
fn trailing_whitespace_preceded_hash_is_still_a_comment() {
    let dict = parse(
        "version FIX.TEST\n\
         field 100 TestField STRING VAL1 VAL2 # a real trailing comment\n",
    )
    .unwrap();

    let field = dict.field(100).expect("field 100 must be defined");
    assert_eq!(field.values, vec!["VAL1".to_owned(), "VAL2".to_owned()]);
}

#[test]
fn leading_hash_comment_line_is_still_ignored() {
    let dict = parse(
        "version FIX.TEST\n\
         # a full-line comment\n\
         field 100 TestField STRING VAL1\n",
    )
    .unwrap();

    let field = dict.field(100).expect("field 100 must be defined");
    assert_eq!(field.values, vec!["VAL1".to_owned()]);
}
