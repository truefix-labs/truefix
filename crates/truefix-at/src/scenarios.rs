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
    out
}
