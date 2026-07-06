use truefix_core::{Field, Message};
use truefix_dict::{RejectReason, ValidationOptions, load_fix44};

#[test]
fn nonnumeric_group_count_is_an_incorrect_data_format() {
    let mut message = Message::new();
    message.header.set(Field::string(8, "FIX.4.4"));
    message.header.set(Field::string(35, "D"));
    message.header.set(Field::int(34, 1));
    message.header.set(Field::string(49, "CLIENT"));
    message.header.set(Field::string(56, "SERVER"));
    message.header.set(Field::string(52, "20240101-00:00:00"));
    message.body.set(Field::string(11, "ORDER-1"));
    message.body.set(Field::string(21, "1"));
    message.body.set(Field::string(55, "AAPL"));
    message.body.set(Field::string(54, "1"));
    message.body.set(Field::string(60, "20240101-00:00:00"));
    message.body.set(Field::string(40, "2"));
    message.body.add_field(Field::string(453, "not-a-number"));

    // Group structure still requires a numeric count even when general field-type validation is
    // disabled. Previously validate_group mapped this parse failure to -1 and returned the less
    // precise IncorrectNumInGroupCount.
    let options = ValidationOptions {
        check_field_types: false,
        ..ValidationOptions::default()
    };
    let error = load_fix44()
        .unwrap()
        .validate(&message, &options)
        .unwrap_err();

    assert_eq!(error.reason, RejectReason::IncorrectDataFormat);
    assert_eq!(error.ref_tag, Some(453));
}
