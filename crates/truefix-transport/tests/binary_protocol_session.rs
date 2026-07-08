use truefix_binary::BinaryCodec;
use truefix_binary::fast::FastCodec;
use truefix_binary::sbe::SbeCodec;
use truefix_dict::fast_template::parse_fast_templates;
use truefix_dict::sbe_schema::parse_sbe_schemas;

fn hex_fixture(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|part| u8::from_str_radix(part, 16).expect("fixture hex byte"))
        .collect()
}

#[test]
fn fast_codec_round_trip_is_available_to_transport() {
    let templates = parse_fast_templates(include_str!(
        "../../truefix-binary/tests/fixtures/fast/template_basic.xml"
    ))
    .expect("template parses");
    let codec = FastCodec::with_session(templates, "transport-test");
    let bytes = hex_fixture(include_str!(
        "../../truefix-binary/tests/fixtures/fast/message_basic.bin"
    ));

    let (message, template_id) = codec.decode(&bytes).expect("decode");

    assert_eq!(template_id, 1);
    assert_eq!(
        message.body.get(55).and_then(|f| f.as_str().ok()),
        Some("ABC")
    );
}

#[test]
fn sbe_codec_round_trip_is_available_to_transport() {
    let schemas = parse_sbe_schemas(include_str!(
        "../../truefix-binary/tests/fixtures/sbe/schema_basic.xml"
    ))
    .expect("schema parses");
    let codec = SbeCodec::with_session(schemas, "transport-test");
    let bytes = hex_fixture(include_str!(
        "../../truefix-binary/tests/fixtures/sbe/message_basic.bin"
    ));

    let (message, template_id) = codec.decode(&bytes).expect("decode");

    assert_eq!(template_id, 1);
    assert_eq!(
        message.body.get(34).and_then(|f| f.as_str().ok()),
        Some("1")
    );
}
