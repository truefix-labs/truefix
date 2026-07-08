#![cfg(feature = "dict-tooling")]

use truefix_dict::sbe_schema::{SbeDataType, SbeSchemaError, parse_sbe_schemas};

#[test]
fn parses_valid_multi_message_schema() {
    let set = parse_sbe_schemas(
        r#"
        <messageSchema id="7">
          <message id="1" name="Basic" blockLength="8">
            <field name="SeqNum" id="34" type="uint32" offset="0"/>
            <field name="Price" id="44" type="int32" offset="4"/>
            <group name="Entries" id="268" blockLength="1" dimensionOffset="8">
              <field name="EntryType" id="269" type="char" offset="0"/>
            </group>
            <data name="Text" id="58" lengthFieldSize="1"/>
          </message>
          <message id="2" name="Secondary" blockLength="4">
            <field name="TestField" id="100" type="uint32" offset="0"/>
          </message>
        </messageSchema>
        "#,
    )
    .expect("valid SBE schema parses");

    assert_eq!(set.schema_id, Some(7));
    let basic = set.get(1).expect("basic schema");
    assert_eq!(basic.fields[0].data_type, SbeDataType::UInt32);
    assert_eq!(basic.groups.len(), 1);
    assert_eq!(basic.var_data.len(), 1);
    assert!(set.get(2).is_some());
}

#[test]
fn rejects_duplicate_template_id() {
    let err = parse_sbe_schemas(
        r#"
        <messageSchema>
          <message id="1" name="A" blockLength="0"/>
          <message id="1" name="B" blockLength="0"/>
        </messageSchema>
        "#,
    )
    .expect_err("duplicate should fail");

    assert_eq!(err, SbeSchemaError::DuplicateTemplateId { id: 1 });
}

#[test]
fn rejects_overlapping_fixed_field_offsets() {
    let err = parse_sbe_schemas(
        r#"
        <messageSchema>
          <message id="1" name="A" blockLength="8">
            <field name="A" id="1" type="uint32" offset="0"/>
            <field name="B" id="2" type="uint32" offset="2"/>
          </message>
        </messageSchema>
        "#,
    )
    .expect_err("overlap should fail");

    assert!(matches!(err, SbeSchemaError::OverlappingOffset { .. }));
}

#[test]
fn rejects_unknown_data_type() {
    let err = parse_sbe_schemas(
        r#"
        <messageSchema>
          <message id="1" name="A" blockLength="4">
            <field name="A" id="1" type="float128" offset="0"/>
          </message>
        </messageSchema>
        "#,
    )
    .expect_err("unknown type should fail");

    assert!(matches!(err, SbeSchemaError::UnknownDataType { .. }));
}
