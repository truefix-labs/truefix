//! T016/T017 (US1, feature 009, `NEW-55`): `tags::is_header` was missing the FIXT 1.1
//! transport-layer header tags -- `ApplVerID(1128)`, `ApplReportID(1129)`, `LastApplVerID(1130)`,
//! `ApplExtID(1156)`, and the `NoApplIDs(1351)` group's members (1352-1355). A FIX 5.x message
//! carrying `ApplVerID` was routed to `message.body` instead of `message.header`, breaking
//! FIXT application-dictionary resolution downstream (`state.rs::validate_app` reads
//! `msg.header.get(APPL_VER_ID)`).

use truefix_core::{Field, Message, decode, tags};

fn msg_with_applverid() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIXT.1.1"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::string(1128, "9")); // ApplVerID = FIX50SP2
    m.body.set(Field::string(11, "ORD1"));
    m
}

#[test]
fn applverid_is_classified_as_a_header_tag() {
    assert!(
        tags::is_header(1128),
        "ApplVerID(1128) must be classified as a header tag"
    );
    assert!(tags::is_header(1129), "ApplReportID(1129)");
    assert!(tags::is_header(1130), "LastApplVerID(1130)");
    assert!(tags::is_header(1156), "ApplExtID(1156)");
    assert!(tags::is_header(1351), "NoApplIDs(1351)");
    for member in [1352, 1353, 1354, 1355] {
        assert!(tags::is_header(member), "NoApplIDs member {member}");
    }
}

#[test]
fn a_message_carrying_applverid_decodes_it_into_the_header_not_the_body() {
    let bytes = truefix_core::encode(&msg_with_applverid());
    let decoded = decode(&bytes).expect("decode");
    assert_eq!(
        decoded.header.get(1128).and_then(|f| f.as_str().ok()),
        Some("9"),
        "ApplVerID(1128) must land in message.header, not message.body"
    );
    assert!(
        decoded.body.get(1128).is_none(),
        "ApplVerID(1128) must not also appear in message.body"
    );
}
