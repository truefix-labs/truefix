use truefix_binary::BinaryCodec;
use truefix_binary::sbe::{SbeCodec, SbeCodecError};
use truefix_core::Message;
use truefix_core::field::Field;
use truefix_core::field_map::FieldMap;
use truefix_core::group::Group;
use truefix_dict::sbe_schema::parse_sbe_schemas;

fn hex_fixture(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|part| u8::from_str_radix(part, 16).expect("fixture hex byte"))
        .collect()
}

fn codec() -> SbeCodec {
    let schemas =
        parse_sbe_schemas(include_str!("fixtures/sbe/schema_basic.xml")).expect("schema parses");
    SbeCodec::with_session(schemas, "test")
}

#[test]
fn decodes_and_reencodes_reference_message() {
    let codec = codec();
    let reference = hex_fixture(include_str!("fixtures/sbe/message_basic.bin"));

    let (message, template_id) = codec.decode(&reference).expect("decode");
    assert_eq!(template_id, 1);
    assert_eq!(
        message.body.get(34).and_then(|f| f.as_str().ok()),
        Some("1")
    );
    assert_eq!(
        message.body.get(44).and_then(|f| f.as_str().ok()),
        Some("1250")
    );
    assert_eq!(
        message.body.get(58).map(|f| f.value_bytes()),
        Some(&b"hi"[..])
    );
    let entries = message.body.group(268).expect("group");
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get(269).map(|f| f.value_bytes()),
        Some(&b"A"[..])
    );
    assert_eq!(
        entries[1].get(269).map(|f| f.value_bytes()),
        Some(&b"B"[..])
    );

    assert_eq!(
        codec.encode(&message, template_id).expect("encode"),
        reference
    );
}

#[test]
fn encodes_message_fields_in_schema_order() {
    let codec = codec();
    let mut message = Message::new();
    message.body.set(Field::int(34, 1));
    message.body.set(Field::int(44, 1250));
    let mut group = Group::new(268);
    let mut first = FieldMap::new();
    first.set(Field::bytes(269, b"A"));
    group.add_entry(first);
    let mut second = FieldMap::new();
    second.set(Field::bytes(269, b"B"));
    group.add_entry(second);
    message.body.add_group(group);
    message.body.set(Field::bytes(58, b"hi"));

    assert_eq!(
        codec.encode(&message, 1).expect("encode"),
        hex_fixture(include_str!("fixtures/sbe/message_basic.bin"))
    );
}

#[test]
fn malformed_inputs_return_typed_errors() {
    let codec = codec();

    assert!(matches!(
        codec.decode(&hex_fixture(include_str!(
            "fixtures/sbe/malformed/truncated.bin"
        ))),
        Err(SbeCodecError::Truncated { .. })
    ));
    assert!(matches!(
        codec.decode(&hex_fixture(include_str!(
            "fixtures/sbe/malformed/block_length.bin"
        ))),
        Err(SbeCodecError::BlockLengthMismatch { .. })
    ));
    assert!(matches!(
        codec.decode(&hex_fixture(include_str!(
            "fixtures/sbe/malformed/vardata_length.bin"
        ))),
        Err(SbeCodecError::VarDataLengthMismatch { .. })
    ));
}

#[test]
fn field_reads_after_decode_borrow_existing_values() {
    let codec = codec();
    let reference = hex_fixture(include_str!("fixtures/sbe/message_basic.bin"));
    let (message, _) = codec.decode(&reference).expect("decode");

    let _ = message.body.get(34).and_then(|f| f.as_str().ok());
    let _ = message.body.get(44).and_then(|f| f.as_str().ok());
    let _ = message.body.get(58).map(|f| f.value_bytes());
}
