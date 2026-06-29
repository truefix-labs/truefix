//! T043 — dictionary parsing.

use truefix_dict::{load_fix44, parse, FieldType};

#[test]
fn parses_bundled_fix44() {
    let d = load_fix44().unwrap();
    assert_eq!(d.version(), "FIX.4.4");
    assert!(d.field(55).is_some()); // Symbol
    assert_eq!(d.field_by_name("Symbol"), Some(55));
    assert_eq!(d.field(54).unwrap().field_type, FieldType::Char); // Side
    assert_eq!(d.field(44).unwrap().field_type, FieldType::Price);
    assert!(d.message("D").is_some()); // NewOrderSingle
    assert!(d.message("A").is_some()); // Logon
    assert!(d.is_header(35));
    assert!(d.is_trailer(10));
    assert!(d.field_count() >= 20);
    assert!(d.message_count() >= 6);
}

#[test]
fn enum_values_parsed() {
    let d = load_fix44().unwrap();
    let side = d.field(54).unwrap();
    assert!(side.allows("1"));
    assert!(side.allows("2"));
    assert!(!side.allows("9"));
}

#[test]
fn message_required_and_optional_fields() {
    let d = load_fix44().unwrap();
    let nos = d.message("D").unwrap();
    assert!(nos.required.contains(&11)); // ClOrdID
    assert!(nos.required.contains(&55)); // Symbol
    assert!(nos.optional.contains(&44)); // Price
    assert!(nos.allows_tag(38)); // OrderQty (optional)
    assert!(!nos.allows_tag(7)); // BeginSeqNo not in NewOrderSingle
}

#[test]
fn missing_version_errors() {
    assert!(parse("field 55 Symbol STRING\n").is_err());
}

#[test]
fn unknown_type_errors() {
    assert!(parse("version FIX.4.4\nfield 55 Symbol WIDGET\n").is_err());
}
