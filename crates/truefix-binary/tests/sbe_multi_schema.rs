use truefix_binary::BinaryCodec;
use truefix_binary::sbe::{SbeCodec, SbeCodecError};
use truefix_dict::sbe_schema::parse_sbe_schemas;

fn hex_fixture(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|part| u8::from_str_radix(part, 16).expect("fixture hex byte"))
        .collect()
}

#[test]
fn selects_schema_by_template_id() {
    let schemas = parse_sbe_schemas(&format!(
        "{}{}",
        include_str!("fixtures/sbe/schema_basic.xml"),
        include_str!("fixtures/sbe/schema_secondary.xml")
    ))
    .expect("schemas parse");
    let codec = SbeCodec::new(schemas);

    let (message, template_id) = codec
        .decode(&hex_fixture("02 00 07 00 04 00 2a 00 00 00"))
        .expect("decode");

    assert_eq!(template_id, 2);
    assert_eq!(
        message.body.get(100).and_then(|f| f.as_str().ok()),
        Some("42")
    );
}

#[test]
fn unknown_template_id_is_typed_error() {
    let schemas =
        parse_sbe_schemas(include_str!("fixtures/sbe/schema_basic.xml")).expect("schema parses");
    let codec = SbeCodec::with_session(schemas, "test");

    assert!(matches!(
        codec.decode(&hex_fixture("63 00 07 00 00 00")),
        Err(SbeCodecError::UnknownTemplateId { id: 99 })
    ));
}
