use truefix_binary::fast::FastCodec;
use truefix_binary::fast::FastCodecError;
use truefix_binary::ir::{decode_into_message, encode_from_message};
use truefix_binary::sbe::SbeCodec;
use truefix_core::Message;
use truefix_core::field::Field;
use truefix_dict::fast_template::parse_fast_templates;
use truefix_dict::sbe_schema::parse_sbe_schemas;

fn hex_fixture(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|part| u8::from_str_radix(part, 16).expect("fixture hex byte"))
        .collect()
}

#[test]
fn fast_decode_encode_message_round_trip() {
    let templates = parse_fast_templates(include_str!("fixtures/fast/template_basic.xml"))
        .expect("template parses");
    let codec = FastCodec::new(templates);
    let reference = hex_fixture(include_str!("fixtures/fast/message_basic.bin"));

    let message = decode_into_message(&codec, &reference).expect("decode");
    assert_eq!(
        message.body.get(55).and_then(|f| f.as_str().ok()),
        Some("ABC")
    );
    assert_eq!(
        encode_from_message(&codec, &message, 1).expect("encode"),
        reference
    );
}

#[test]
fn sbe_decode_encode_message_round_trip() {
    let schemas =
        parse_sbe_schemas(include_str!("fixtures/sbe/schema_basic.xml")).expect("schema parses");
    let codec = SbeCodec::new(schemas);
    let reference = hex_fixture(include_str!("fixtures/sbe/message_basic.bin"));

    let message = decode_into_message(&codec, &reference).expect("decode");
    assert_eq!(
        message.body.get(34).and_then(|f| f.as_str().ok()),
        Some("1")
    );
    assert_eq!(
        encode_from_message(&codec, &message, 1).expect("encode"),
        reference
    );
}

#[test]
fn unsupported_construct_surfaces_codec_error() {
    let templates = parse_fast_templates(include_str!("fixtures/fast/template_basic.xml"))
        .expect("template parses");
    let codec = FastCodec::new(templates);
    let mut message = Message::new();
    message.body.set(Field::string(99999, "unsupported"));

    assert!(matches!(
        encode_from_message(&codec, &message, 1),
        Err(FastCodecError::UnsupportedValue { field: 99999, .. })
    ));
}
