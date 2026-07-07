//! T024/T025 (US3, feature 011) — vendor dictionary XML → normalized `.fixdict` conversion.
#![cfg(feature = "dict-tooling")]

use std::fs;
use std::path::Path;

use truefix_dict::vendor_xml::{VendorXmlError, convert};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

fn malformed_fixture(name: &str) -> String {
    fixture(&format!("vendor_dict_malformed/{name}"))
}

#[test]
fn converts_the_sample_fixture_and_resolves_fields_components_and_nested_groups() {
    let xml = fixture("vendor_dict_sample.xml");
    let text = convert(&xml).expect("convert");
    let dict = truefix_dict::parse(&text).expect("parse converted .fixdict text");

    assert_eq!(dict.version(), "FIX.4.4");
    assert!(dict.is_header(8)); // BeginString
    assert!(dict.is_trailer(10)); // CheckSum

    // Multi-character custom MsgType (FR-012).
    let confirmation = dict
        .message("XCN")
        .expect("XCN (TestCustomConfirmation) is defined");
    assert_eq!(confirmation.name, "TestCustomConfirmation");
    assert!(confirmation.allows_tag(17)); // ExecID

    // Component reference resolves to the same fields as if inlined (FR-013).
    let new_order = dict.message("D").expect("NewOrderSingle is defined");
    assert!(new_order.allows_tag(55)); // Symbol, via the Instrument component
    assert!(new_order.allows_tag(48)); // SecurityID, via the Instrument component

    // Body-level group (NoPartyIDs, 453) resolves with the correct delimiter/members/required.
    let party_ids = dict.group(453).expect("NoPartyIDs(453) is defined");
    assert_eq!(party_ids.delimiter, 448);
    assert_eq!(party_ids.members, vec![448, 447, 452]);
    assert_eq!(party_ids.required, vec![448]);

    // Nested group (NoMiscFees containing NoNestedFeeDetails) resolves at both levels (FR-013).
    let misc_fees = dict.group(136).expect("NoMiscFees(136) is defined");
    assert_eq!(misc_fees.members, vec![137, 138, 9001]);
    let nested = dict
        .group(9001)
        .expect("NoNestedFeeDetails(9001) is defined");
    assert_eq!(nested.members, vec![9002]);
}

#[test]
fn missing_version_attributes_produce_an_error_not_a_guess() {
    let xml = malformed_fixture("missing_version.xml");
    let err = convert(&xml).unwrap_err();
    assert_eq!(err, VendorXmlError::MissingVersion);
}

#[test]
fn undefined_component_reference_produces_an_error() {
    let xml = malformed_fixture("undefined_component_reference.xml");
    let err = convert(&xml).unwrap_err();
    assert!(matches!(
        err,
        VendorXmlError::UnknownReference {
            kind: "component",
            ..
        }
    ));
}

#[test]
fn broken_xml_syntax_produces_an_error_never_a_panic() {
    let xml = malformed_fixture("broken_syntax.xml");
    let err = convert(&xml).unwrap_err();
    assert!(matches!(err, VendorXmlError::Xml { .. }));
}

#[test]
fn unknown_field_type_errors_never_panic() {
    let xml = r#"<fix type="FIX" major="4" minor="4">
        <header><field name="BeginString" required="Y"/></header>
        <trailer><field name="CheckSum" required="Y"/></trailer>
        <messages></messages>
        <components/>
        <fields>
            <field number="8" name="BeginString" type="STRING"/>
            <field number="10" name="CheckSum" type="STRING"/>
            <field number="1" name="Bogus" type="NotARealType"/>
        </fields>
    </fix>"#;
    let err = convert(xml).unwrap_err();
    assert!(matches!(err, VendorXmlError::UnknownType { .. }));
}
