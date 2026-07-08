use truefix_binary::BinaryCodec;
use truefix_binary::fast::FastCodec;
use truefix_dict::fast_template::parse_fast_templates;

fn hex_fixture(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|part| u8::from_str_radix(part, 16).expect("fixture hex byte"))
        .collect()
}

#[test]
fn copy_increment_delta_values_continue_from_context_when_omitted() {
    let templates = parse_fast_templates(include_str!("fixtures/fast/template_basic.xml"))
        .expect("template parses");
    let codec = FastCodec::new(templates);
    let first = hex_fixture(include_str!("fixtures/fast/message_basic.bin"));
    let second = hex_fixture("81 80");

    codec.decode(&first).expect("first decode");
    let (message, _) = codec.decode(&second).expect("second decode");

    assert_eq!(
        message.body.get(34).and_then(|f| f.as_str().ok()),
        Some("1")
    );
    assert_eq!(
        message.body.get(55).and_then(|f| f.as_str().ok()),
        Some("ABC")
    );
    assert_eq!(
        message.body.get(44).and_then(|f| f.as_str().ok()),
        Some("12.5")
    );
    assert_eq!(
        message.body.get(59).and_then(|f| f.as_str().ok()),
        Some("0")
    );
}
