//! T059/T060 (US2, feature 007): an outbound session-level `Reject(35=3)` includes `RefMsgType`
//! only for FIX.4.2+ (`BUG-58`/FR-023), and `SessionRejectReason(373)` is version-filtered to the
//! codes each FIX version actually defines (`BUG-59`/FR-023):
//! - FIX.4.0/4.1: `SessionRejectReason` omitted entirely, `RefMsgType` omitted.
//! - FIX.4.2: `SessionRejectReason` only for codes <=11.
//! - FIX.4.3: `SessionRejectReason` only for codes <=15.
//! - FIX.4.4+: `SessionRejectReason` only for codes <=16 (or 99/OTHER).

use truefix_core::{Field, Message};
use truefix_dict::ValidationOptions;
use truefix_session::{Action, Event, Role, Session, SessionConfig};

fn cfg(version: &str) -> SessionConfig {
    let mut c = SessionConfig::new(version, "M", "Y", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn msg(version: &str, msg_type: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, version));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::string(49, "Y"));
    m.header.set(Field::string(56, "M"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

fn logon(version: &str) -> Message {
    let mut m = msg(version, "A", 1);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

/// A Heartbeat with tag 112 (TestReqID) repeated -- triggers `TagAppearsMoreThanOnce`
/// (SessionRejectReason code 13), stable and simple across every FIX.4.x dictionary.
fn heartbeat_with_repeated_tag(version: &str, seq: i64) -> Message {
    let mut m = msg(version, "0", seq);
    m.body.add_field(Field::string(112, "A"));
    m.body.add_field(Field::string(112, "B")); // repeated -- not via `.set()`, which would replace
    m
}

fn dict_for(version: &str) -> truefix_dict::DataDictionary {
    match version {
        "FIX.4.0" => truefix_dict::load_fix40(),
        "FIX.4.1" => truefix_dict::load_fix41(),
        "FIX.4.2" => truefix_dict::load_fix42(),
        "FIX.4.3" => truefix_dict::load_fix43(),
        "FIX.4.4" => truefix_dict::load_fix44(),
        _ => unreachable!(),
    }
    .unwrap()
}

fn reject_from(version: &str) -> Option<Message> {
    let mut s = Session::new(cfg(version));
    s.set_dictionary(dict_for(version), ValidationOptions::default());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(version)));
    let actions = s.handle(Event::Received(heartbeat_with_repeated_tag(version, 2)));
    actions.into_iter().find_map(|a| match a {
        Action::Send(m) if m.msg_type() == Some("3") => Some(m),
        _ => None,
    })
}

#[test]
fn fix44_reject_includes_ref_msg_type_and_session_reject_reason() {
    let rej = reject_from("FIX.4.4").expect("a Reject should be sent");
    assert_eq!(
        rej.body.get(372).and_then(|f| f.as_str().ok()),
        Some("0"),
        "RefMsgType(372) should reflect the Heartbeat's own MsgType"
    );
    assert_eq!(
        rej.body.get(373).and_then(|f| f.as_int().ok()),
        Some(13),
        "SessionRejectReason(373)=13 (TagAppearsMoreThanOnce) should be included for FIX.4.4 (<=16)"
    );
}

#[test]
fn fix40_reject_omits_ref_msg_type_and_session_reject_reason() {
    let rej = reject_from("FIX.4.0").expect("a Reject should be sent");
    assert!(
        rej.body.get(372).is_none(),
        "RefMsgType(372) must be omitted for FIX.4.0 (<4.2)"
    );
    assert!(
        rej.body.get(373).is_none(),
        "SessionRejectReason(373) must be omitted entirely for FIX.4.0 (<=4.1)"
    );
}

#[test]
fn fix41_reject_omits_ref_msg_type_and_session_reject_reason() {
    let rej = reject_from("FIX.4.1").expect("a Reject should be sent");
    assert!(rej.body.get(372).is_none());
    assert!(rej.body.get(373).is_none());
}

#[test]
fn fix42_reject_includes_ref_msg_type_but_omits_a_code_above_eleven() {
    let rej = reject_from("FIX.4.2").expect("a Reject should be sent");
    assert_eq!(
        rej.body.get(372).and_then(|f| f.as_str().ok()),
        Some("0"),
        "RefMsgType(372) should be present for FIX.4.2 (>=4.2)"
    );
    assert!(
        rej.body.get(373).is_none(),
        "SessionRejectReason(373)=13 exceeds FIX.4.2's defined range (<=11), must be omitted"
    );
}

#[test]
fn fix43_reject_includes_a_code_above_eleven_but_within_its_own_range() {
    let rej = reject_from("FIX.4.3").expect("a Reject should be sent");
    assert_eq!(rej.body.get(372).and_then(|f| f.as_str().ok()), Some("0"));
    assert_eq!(
        rej.body.get(373).and_then(|f| f.as_int().ok()),
        Some(13),
        "SessionRejectReason(373)=13 is within FIX.4.3's defined range (<=15), must be included"
    );
}
