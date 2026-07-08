#![cfg(feature = "dict-tooling")]

use truefix_dict::fast_template::{
    FastDataType, FastOperator, FastTemplateError, parse_fast_templates,
};

#[test]
fn parses_valid_multi_template_file() {
    let set = parse_fast_templates(
        r#"
        <templates>
          <template id="1" name="Basic">
            <field id="34" name="MsgSeqNum" type="uInt32" operator="increment"/>
            <field id="55" name="Symbol" type="ascii" operator="copy"/>
            <field id="44" name="Price" type="decimal" operator="delta"/>
            <field id="59" name="TimeInForce" type="ascii" operator="default" value="0"/>
            <group id="268" name="NoMDEntries">
              <field id="269" name="MDEntryType" type="ascii"/>
            </group>
          </template>
          <template id="2" name="Secondary">
            <uInt32 id="100" name="TestField"/>
          </template>
        </templates>
        "#,
    )
    .expect("valid FAST template XML should parse");

    let basic = set.get(1).expect("template 1");
    assert_eq!(basic.name, "Basic");
    assert_eq!(basic.fields.len(), 5);
    assert_eq!(basic.fields[0].operator, FastOperator::Increment);
    assert_eq!(basic.fields[1].operator, FastOperator::Copy);
    assert_eq!(basic.fields[2].operator, FastOperator::Delta);
    assert_eq!(
        basic.fields[3].operator,
        FastOperator::Default {
            value: Some("0".to_owned())
        }
    );
    assert!(basic.fields[4].group.is_some());

    let secondary = set.get(2).expect("template 2");
    assert_eq!(secondary.fields[0].data_type, FastDataType::UInt32);
}

#[test]
fn rejects_duplicate_template_id() {
    let err = parse_fast_templates(
        r#"
        <templates>
          <template id="1" name="A"/>
          <template id="1" name="B"/>
        </templates>
        "#,
    )
    .expect_err("duplicate ids should fail");

    assert_eq!(err, FastTemplateError::DuplicateTemplateId { id: 1 });
}

#[test]
fn rejects_unknown_operator() {
    let err = parse_fast_templates(
        r#"
        <templates>
          <template id="1" name="A">
            <field id="55" name="Symbol" type="ascii" operator="sticky"/>
          </template>
        </templates>
        "#,
    )
    .expect_err("unknown operator should fail");

    assert!(matches!(err, FastTemplateError::UnknownOperator { .. }));
}

#[test]
fn rejects_unknown_data_type() {
    let err = parse_fast_templates(
        r#"
        <templates>
          <template id="1" name="A">
            <field id="55" name="Symbol" type="blob"/>
          </template>
        </templates>
        "#,
    )
    .expect_err("unknown type should fail");

    assert!(matches!(err, FastTemplateError::UnknownDataType { .. }));
}
