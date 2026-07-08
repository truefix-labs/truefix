use truefix_binary::BinaryCodec;
use truefix_binary::fast::{FastCodec, FastCodecError};
use truefix_core::Message;
use truefix_core::field::Field;
use truefix_dict::fast_template::parse_fast_templates;

fn hex_fixture(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|part| u8::from_str_radix(part, 16).expect("fixture hex byte"))
        .collect()
}

fn codec() -> FastCodec {
    let templates = parse_fast_templates(include_str!("fixtures/fast/template_basic.xml"))
        .expect("template parses");
    FastCodec::with_session(templates, "test")
}

#[test]
fn decodes_and_reencodes_reference_message() {
    let codec = codec();
    let reference = hex_fixture(include_str!("fixtures/fast/message_basic.bin"));

    let (message, template_id) = codec.decode(&reference).expect("decode");
    assert_eq!(template_id, 1);
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

    let encoded = codec.encode(&message, template_id).expect("encode");
    assert_eq!(encoded, reference);
}

#[test]
fn nullable_field_presence_bit_zero_is_absent_not_panic() {
    let templates = parse_fast_templates(
        r#"
        <templates>
          <template id="7" name="Nullable">
            <field id="9000" name="OptionalText" type="ascii" nullable="true"/>
          </template>
        </templates>
        "#,
    )
    .expect("template parses");
    let codec = FastCodec::new(templates);
    let bytes = hex_fixture("87 80");

    let (message, template_id) = codec.decode(&bytes).expect("decode");
    assert_eq!(template_id, 7);
    assert!(!message.body.contains(9000));
}

#[test]
fn malformed_inputs_return_typed_errors() {
    let codec = codec();
    let truncated = hex_fixture(include_str!("fixtures/fast/malformed/truncated.bin"));
    let invalid_stop_bit =
        hex_fixture(include_str!("fixtures/fast/malformed/invalid_stop_bit.bin"));

    assert!(matches!(
        codec.decode(&truncated),
        Err(FastCodecError::Truncated { .. })
    ));
    assert!(matches!(
        codec.decode(&invalid_stop_bit),
        Err(FastCodecError::InvalidStopBit { .. })
    ));
}

#[test]
fn encodes_message_fields_in_template_order() {
    let codec = codec();
    let mut message = Message::new();
    message.body.set(Field::int(34, 1));
    message.body.set(Field::string(55, "ABC"));
    message.body.set(Field::string(44, "12.5"));
    message.body.set(Field::string(59, "0"));

    assert_eq!(
        codec.encode(&message, 1).expect("encode"),
        hex_fixture(include_str!("fixtures/fast/message_basic.bin"))
    );
}
