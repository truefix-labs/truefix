//! Authored acceptance-test scenarios (black-box behaviour contracts).
//!
//! These reproduce the *behaviour* of the classic QuickFIX server AT scenarios from the FIX
//! specification — independently scripted, with no source or test data copied (Constitution
//! Principle III). This is a representative core subset (logon, sequence handling, test request,
//! logout); porting the full 73-scenario corpus is incremental authoring on top of this runner.

use truefix_core::{Field, Message};

use crate::runner::{client_message, ExpectMsg, Scenario, Step};

/// The FIX versions the server suite is exercised against.
pub const SUITE_VERSIONS: &[&str] = &["FIX.4.2", "FIX.4.4"];

fn logon(version: &str, seq: i64, reset: bool) -> Message {
    let mut m = client_message(version, "A", seq);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    if reset {
        m.body.set(Field::string(141, "Y"));
    }
    m
}

fn scenario(name: &str, version: &str, steps: Vec<Step>) -> Scenario {
    Scenario {
        name: name.to_owned(),
        versions: vec![version.to_owned()],
        steps,
    }
}

/// 1a — a valid Logon with the correct MsgSeqNum is answered with a Logon.
fn valid_logon(v: &str) -> Scenario {
    scenario(
        "1a_ValidLogonWithCorrectMsgSeqNum",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
        ],
    )
}

/// 1a — a Logon whose MsgSeqNum is too high logs on, then a ResendRequest is issued.
fn logon_seq_too_high(v: &str) -> Scenario {
    scenario(
        "1a_ValidLogonMsgSeqNumTooHigh",
        v,
        vec![
            Step::Send(logon(v, 10, false)), // no reset; expected is 1
            Step::Expect(ExpectMsg::of("A")),
            Step::Expect(ExpectMsg::of("2").field(7, "1")), // ResendRequest BeginSeqNo=1
        ],
    )
}

/// 2b — an application/admin message with MsgSeqNum too high triggers a ResendRequest.
fn msgseqnum_too_high(v: &str) -> Scenario {
    scenario(
        "2b_MsgSeqNumTooHigh",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(client_message(v, "0", 10)), // Heartbeat, seq too high
            Step::Expect(ExpectMsg::of("2").field(7, "2")), // ResendRequest BeginSeqNo=2
        ],
    )
}

/// 2c — a message with MsgSeqNum too low (no PossDup) draws a Logout and disconnect.
fn msgseqnum_too_low(v: &str) -> Scenario {
    scenario(
        "2c_MsgSeqNumTooLow",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(client_message(v, "0", 1)), // Heartbeat, seq too low
            Step::Expect(ExpectMsg::of("5")),      // Logout
            Step::ExpectDisconnect,
        ],
    )
}

/// 4b — a received TestRequest is answered with a Heartbeat echoing the TestReqID.
fn received_test_request(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 2);
    tr.body.set(Field::string(112, "HELLO"));
    scenario(
        "4b_ReceivedTestRequest",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "HELLO")),
        ],
    )
}

/// 13b — an unsolicited Logout is answered with a Logout and disconnect.
fn unsolicited_logout(v: &str) -> Scenario {
    scenario(
        "13b_UnsolicitedLogoutMessage",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(client_message(v, "5", 2)), // Logout
            Step::Expect(ExpectMsg::of("5")),
            Step::ExpectDisconnect,
        ],
    )
}

/// A valid FIX.4.4 NewOrderSingle (all required fields present).
fn new_order_single(seq: i64) -> Message {
    let mut m = client_message("FIX.4.4", "D", seq);
    m.body.set(Field::string(11, "ORDER-1")); // ClOrdID
    m.body.set(Field::string(21, "1")); // HandlInst
    m.body.set(Field::string(55, "AAPL")); // Symbol
    m.body.set(Field::string(54, "1")); // Side
    m.body.set(Field::string(60, "20240101-00:00:00")); // TransactTime
    m.body.set(Field::string(40, "2")); // OrdType
    m
}

/// 14b — a NewOrderSingle missing a required field draws a session-level Reject.
fn required_field_missing(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body = {
        // rebuild body without HandlInst(21)
        let mut b = truefix_core::FieldMap::new();
        for f in new_order_single(2).body.fields() {
            if f.tag() != 21 {
                b.set(Field::new(f.tag(), f.value_bytes().to_vec()));
            }
        }
        b
    };
    scenario(
        "14b_RequiredFieldMissing",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "1")), // Reject: RequiredTagMissing
        ],
    )
}

/// 14e — a field with an out-of-range enumerated value draws a session-level Reject.
fn incorrect_enum_value(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body.set(Field::string(54, "9")); // Side not in {1,2,5,6}
    scenario(
        "14e_IncorrectEnumValue",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "5")), // Reject: ValueIsIncorrect
        ],
    )
}

/// 2r — an unregistered (unknown) MsgType draws a Business Message Reject.
fn unregistered_msg_type(v: &str) -> Scenario {
    scenario(
        "2r_UnregisteredMsgType",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(client_message(v, "UU", 2)), // unknown application MsgType
            Step::Expect(ExpectMsg::of("j")),       // BusinessMessageReject
        ],
    )
}

/// The (representative) server acceptance-test suite across [`SUITE_VERSIONS`].
pub fn server_suite() -> Vec<Scenario> {
    let mut out = Vec::new();
    for &v in SUITE_VERSIONS {
        out.push(valid_logon(v));
        out.push(logon_seq_too_high(v));
        out.push(msgseqnum_too_high(v));
        out.push(msgseqnum_too_low(v));
        out.push(received_test_request(v));
        out.push(unsolicited_logout(v));
    }
    // Field-validation scenarios require the dictionary; authored for FIX.4.4.
    out.push(required_field_missing("FIX.4.4"));
    out.push(incorrect_enum_value("FIX.4.4"));
    out.push(unregistered_msg_type("FIX.4.4"));
    out
}
