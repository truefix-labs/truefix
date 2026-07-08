use truefix_binary::fast::FastCodec;
use truefix_dict::fast_template::parse_fast_templates;

#[test]
fn encoding_context_reset_emits_without_panicking() {
    let templates = parse_fast_templates(include_str!("fixtures/fast/template_basic.xml"))
        .expect("template parses");
    let codec = FastCodec::with_session(templates, "SESSION");

    codec.reset_context(1);
}
