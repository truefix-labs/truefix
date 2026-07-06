use truefix_core::Field;
use truefix_dict::FieldType;

fn field(value: &str) -> Field {
    Field::string(1, value)
}

#[test]
fn day_of_month_is_limited_to_one_through_thirty_one() {
    for valid in ["1", "15", "31"] {
        assert!(FieldType::DayOfMonth.value_ok(&field(valid), false));
    }
    for invalid in ["0", "32", "-1"] {
        assert!(!FieldType::DayOfMonth.value_ok(&field(invalid), false));
    }
}

#[test]
fn nonnegative_integer_types_reject_negative_values() {
    for field_type in [FieldType::Length, FieldType::SeqNum, FieldType::NumInGroup] {
        assert!(field_type.value_ok(&field("0"), false));
        assert!(field_type.value_ok(&field("1"), false));
        assert!(
            !field_type.value_ok(&field("-1"), false),
            "{field_type:?} accepted a negative value"
        );
    }
}
