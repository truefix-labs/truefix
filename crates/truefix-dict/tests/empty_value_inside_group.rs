use truefix_core::{Field, FieldMap, Group, Message};
use truefix_dict::{RejectReason, ValidationOptions, load_fix44};

fn order_with_empty_party_id() -> Message {
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

    let mut entry = FieldMap::new();
    entry.add_field(Field::string(448, ""));
    entry.add_field(Field::string(447, "D"));
    entry.add_field(Field::string(452, "1"));
    let mut parties = Group::new(453);
    parties.add_entry(entry);
    message.body.add_group(parties);
    message
}

#[test]
fn empty_value_in_a_structured_group_entry_is_rejected() {
    let error = load_fix44()
        .unwrap()
        .validate(&order_with_empty_party_id(), &ValidationOptions::default())
        .unwrap_err();

    assert_eq!(error.reason, RejectReason::TagSpecifiedWithoutValue);
    assert_eq!(error.ref_tag, Some(448));
}

#[test]
fn group_empty_value_check_respects_the_validation_toggle() {
    let options = ValidationOptions {
        validate_fields_have_values: false,
        ..ValidationOptions::default()
    };

    assert!(
        load_fix44()
            .unwrap()
            .validate(&order_with_empty_party_id(), &options)
            .is_ok()
    );
}
