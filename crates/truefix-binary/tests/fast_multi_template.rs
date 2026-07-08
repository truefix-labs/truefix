use truefix_binary::BinaryCodec;
use truefix_binary::fast::{FastCodec, FastCodecError};
use truefix_dict::fast_template::parse_fast_templates;

fn hex_fixture(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|part| u8::from_str_radix(part, 16).expect("fixture hex byte"))
        .collect()
}

#[test]
fn selects_template_by_inline_id() {
    let templates = parse_fast_templates(&format!(
        "{}{}",
        include_str!("fixtures/fast/template_basic.xml"),
        include_str!("fixtures/fast/template_secondary.xml")
    ))
    .expect("templates parse");
    let codec = FastCodec::new(templates);
    let bytes = hex_fixture("82 c0 aa");

    let (message, template_id) = codec.decode(&bytes).expect("decode");
    assert_eq!(template_id, 2);
    assert_eq!(
        message.body.get(100).and_then(|f| f.as_str().ok()),
        Some("42")
    );
}

#[test]
fn unknown_template_id_is_typed_error() {
    let templates = parse_fast_templates(include_str!("fixtures/fast/template_basic.xml"))
        .expect("template parses");
    let codec = FastCodec::with_session(templates, "test");

    assert!(matches!(
        codec.decode(&hex_fixture("e3 80")),
        Err(FastCodecError::UnknownTemplateId { id: 99 })
    ));
}
