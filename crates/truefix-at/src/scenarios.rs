//! Authored acceptance-test scenarios (black-box behaviour contracts).
//!
//! These reproduce the *behaviour* of the classic QuickFIX server AT scenarios from the FIX
//! specification — independently scripted, with no source or test data copied (Constitution
//! Principle III). Coverage spans logon/sequencing, admin handling, resend/gap-fill, out-of-order
//! queueing, the field-validation reject family (across FIX.4.2 and FIX.4.4), and the special
//! suites (789/369, CheckLatency, garbled/checksum, resend chunking). The corpus is grown by
//! adding permutations on top of this runner.

use truefix_core::{Field, Message};

use crate::runner::{client_message, ExpectMsg, Scenario, SessionTweaks, Step};

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
    scenario_with(name, version, steps, SessionTweaks::default())
}

fn scenario_with(name: &str, version: &str, steps: Vec<Step>, tweaks: SessionTweaks) -> Scenario {
    Scenario {
        name: name.to_owned(),
        versions: vec![version.to_owned()],
        steps,
        tweaks,
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

/// 2f — a ResendRequest for already-sent admin traffic is answered with a SequenceReset-GapFill.
fn resend_request_gap_fill(v: &str) -> Scenario {
    let mut rr = client_message(v, "2", 2);
    rr.body.set(Field::int(7, 1)); // BeginSeqNo
    rr.body.set(Field::int(16, 0)); // EndSeqNo=0 → "until end"
    scenario(
        "2f_ResendRequestGapFill",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(rr),
            // Only the admin Logon was sent, so the resend collapses to a GapFill.
            Step::Expect(ExpectMsg::of("4").field(123, "Y").field(36, "2")),
        ],
    )
}

/// 2g — a SequenceReset-Reset advances the expected inbound sequence number.
fn sequence_reset_reset(v: &str) -> Scenario {
    let mut sr = client_message(v, "4", 99); // MsgSeqNum ignored for SequenceReset-Reset
    sr.body.set(Field::int(36, 5)); // NewSeqNo=5 (no GapFillFlag → Reset)
    let mut tr = client_message(v, "1", 5); // now-expected sequence
    tr.body.set(Field::string(112, "AFTER-RESET"));
    scenario(
        "2g_SequenceResetReset",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(sr),
            Step::Send(tr),
            // Heartbeat (not ResendRequest) proves the server now expects seq 5.
            Step::Expect(ExpectMsg::of("0").field(112, "AFTER-RESET")),
        ],
    )
}

/// 2m — a message with MsgSeqNum too low but PossDupFlag=Y is ignored, not a Logout.
fn poss_dup_too_low(v: &str) -> Scenario {
    let mut dup = client_message(v, "0", 1); // Heartbeat, seq too low
    dup.header.set(Field::string(43, "Y")); // PossDupFlag
    let mut tr = client_message(v, "1", 2);
    tr.body.set(Field::string(112, "STILL-UP"));
    scenario(
        "2m_PossDupMsgSeqNumTooLow",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(dup),
            Step::Send(tr),
            // A Heartbeat (not Logout/disconnect) proves the dup was silently ignored.
            Step::Expect(ExpectMsg::of("0").field(112, "STILL-UP")),
        ],
    )
}

/// 2_noseq — a post-logon message with no MsgSeqNum(34) draws a session-level Reject.
fn missing_msg_seq_num(v: &str) -> Scenario {
    let mut m = Message::new();
    m.header.set(Field::string(8, v));
    m.header.set(Field::string(35, "1")); // TestRequest, but without MsgSeqNum
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(112, "NOSEQ"));
    scenario(
        "2_MissingMsgSeqNum",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(m),
            Step::Expect(ExpectMsg::of("3")), // Reject: missing/invalid MsgSeqNum
        ],
    )
}

/// 1c — the acceptor adopts the HeartBtInt from the inbound Logon and echoes it in its response.
fn logon_adopts_heartbeat_interval(v: &str) -> Scenario {
    let mut lo = logon(v, 1, true);
    lo.body.set(Field::int(108, 45)); // override the helper's default of 30
    scenario(
        "1c_LogonAdoptsHeartBtInt",
        v,
        vec![
            Step::Send(lo),
            Step::Expect(ExpectMsg::of("A").field(108, "45")),
        ],
    )
}

/// 2x — out-of-order delivery: a too-high message is queued and a ResendRequest is issued; once the
/// gap is filled the queued message is processed in order.
fn out_of_order_queued_then_drained(v: &str) -> Scenario {
    let mut tr3 = client_message(v, "1", 3);
    tr3.body.set(Field::string(112, "THREE"));
    let mut tr2 = client_message(v, "1", 2);
    tr2.body.set(Field::string(112, "TWO"));
    scenario(
        "2x_OutOfOrderQueuedThenDrained",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr3), // seq 3 arrives before 2 → queued
            Step::Expect(ExpectMsg::of("2").field(7, "2")), // ResendRequest BeginSeqNo=2
            Step::Send(tr2), // fills the gap
            Step::Expect(ExpectMsg::of("0").field(112, "TWO")), // seq 2 processed
            Step::Expect(ExpectMsg::of("0").field(112, "THREE")), // queued seq 3 drained in order
        ],
    )
}

/// 2f3 — a ResendRequest with an explicit (bounded) EndSeqNo still collapses admin traffic to a
/// GapFill.
fn resend_request_bounded_end(v: &str) -> Scenario {
    let mut rr = client_message(v, "2", 2);
    rr.body.set(Field::int(7, 1)); // BeginSeqNo
    rr.body.set(Field::int(16, 1)); // EndSeqNo explicit, not 0
    scenario(
        "2f3_ResendRequestBoundedEnd",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(rr),
            Step::Expect(ExpectMsg::of("4").field(123, "Y").field(36, "2")),
        ],
    )
}

/// 2d — a SequenceReset-GapFill advances the expected inbound sequence number.
fn sequence_reset_gap_fill_advances(v: &str) -> Scenario {
    let mut sr = client_message(v, "4", 2);
    sr.body.set(Field::string(123, "Y")); // GapFillFlag
    sr.body.set(Field::int(36, 5)); // NewSeqNo
    let mut tr = client_message(v, "1", 5);
    tr.body.set(Field::string(112, "GAP-FILLED"));
    scenario(
        "2d_SequenceResetGapFillAdvances",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(sr),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "GAP-FILLED")),
        ],
    )
}

/// 2e — a SequenceReset-GapFill whose NewSeqNo is behind the expected number is ignored.
fn sequence_reset_gap_fill_backward_ignored(v: &str) -> Scenario {
    // After logon the server expects seq 2; a GapFill to NewSeqNo=1 must not rewind it.
    let mut sr = client_message(v, "4", 2);
    sr.body.set(Field::string(123, "Y"));
    sr.body.set(Field::int(36, 1)); // backward NewSeqNo
    let mut tr = client_message(v, "1", 2); // still the expected sequence
    tr.body.set(Field::string(112, "NO-REWIND"));
    scenario(
        "2e_SequenceResetGapFillBackwardIgnored",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(sr),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "NO-REWIND")),
        ],
    )
}

/// 2h — a ResendRequest for messages that were never sent yields no response (session continues).
fn resend_request_nothing_to_resend(v: &str) -> Scenario {
    let mut rr = client_message(v, "2", 2);
    rr.body.set(Field::int(7, 5)); // BeginSeqNo beyond anything sent
    rr.body.set(Field::int(16, 0));
    let mut tr = client_message(v, "1", 3); // ResendRequest consumed seq 2
    tr.body.set(Field::string(112, "ALIVE"));
    scenario(
        "2h_ResendRequestNothingToResend",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(rr),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "ALIVE")),
        ],
    )
}

/// 3a — a received Reject is consumed (no reply) and the sequence number advances.
fn reject_message_consumed(v: &str) -> Scenario {
    let mut rej = client_message(v, "3", 2);
    rej.body.set(Field::int(45, 1)); // RefSeqNum
    let mut tr = client_message(v, "1", 3); // Reject consumed seq 2
    tr.body.set(Field::string(112, "AFTER-REJECT"));
    scenario(
        "3a_ReceivedRejectConsumed",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(rej),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "AFTER-REJECT")),
        ],
    )
}

/// 0a — a received Heartbeat is consumed (no reply) and the sequence number advances.
fn heartbeat_consumed(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 3); // Heartbeat consumed seq 2
    tr.body.set(Field::string(112, "AFTER-HB"));
    scenario(
        "0a_ReceivedHeartbeatConsumed",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(client_message(v, "0", 2)), // Heartbeat at the expected sequence
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "AFTER-HB")),
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

/// A valid FIX.4.2 NewOrderSingle (req:11,55,54,38,40 — note no HandlInst/TransactTime, Side {1,2}).
fn new_order_single_42(seq: i64) -> Message {
    let mut m = client_message("FIX.4.2", "D", seq);
    m.body.set(Field::string(11, "ORDER-42")); // ClOrdID
    m.body.set(Field::string(55, "AAPL")); // Symbol
    m.body.set(Field::string(54, "1")); // Side
    m.body.set(Field::int(38, 100)); // OrderQty
    m.body.set(Field::string(40, "2")); // OrdType
    m
}

/// 14_valid (FIX.4.2) — a fully-valid NewOrderSingle passes the 4.2 dictionary: no Reject.
fn valid_new_order_accepted_42() -> Scenario {
    let mut tr = client_message("FIX.4.2", "1", 3);
    tr.body.set(Field::string(112, "ORDER42-OK"));
    scenario(
        "14_valid_NewOrderSingleAccepted_42",
        "FIX.4.2",
        vec![
            Step::Send(logon("FIX.4.2", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(new_order_single_42(2)),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "ORDER42-OK")),
        ],
    )
}

/// 14b (FIX.4.2) — a NewOrderSingle missing required OrderQty(38) draws a session-level Reject.
fn required_field_missing_42() -> Scenario {
    let mut order = new_order_single_42(2);
    order.body = {
        let mut b = truefix_core::FieldMap::new();
        for f in new_order_single_42(2).body.fields() {
            if f.tag() != 38 {
                b.set(Field::new(f.tag(), f.value_bytes().to_vec()));
            }
        }
        b
    };
    scenario(
        "14b_RequiredFieldMissing_42",
        "FIX.4.2",
        vec![
            Step::Send(logon("FIX.4.2", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "1")), // RequiredTagMissing
        ],
    )
}

/// 14e (FIX.4.2) — Side=9 is outside the 4.2 enumeration {1,2} and draws a session-level Reject.
fn incorrect_enum_value_42() -> Scenario {
    let mut order = new_order_single_42(2);
    order.body.set(Field::string(54, "9")); // Side not in {1,2} for FIX.4.2
    scenario(
        "14e_IncorrectEnumValue_42",
        "FIX.4.2",
        vec![
            Step::Send(logon("FIX.4.2", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "5")), // ValueIsIncorrect
        ],
    )
}

/// 14_valid — a fully-valid NewOrderSingle passes validation: no Reject, sequence advances.
fn valid_new_order_accepted(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 3); // order consumed seq 2
    tr.body.set(Field::string(112, "ORDER-OK"));
    scenario(
        "14_valid_NewOrderSingleAccepted",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(new_order_single(2)),
            Step::Send(tr),
            // A Heartbeat (not a Reject) proves the order was accepted and seq advanced.
            Step::Expect(ExpectMsg::of("0").field(112, "ORDER-OK")),
        ],
    )
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

/// 14a — a field whose tag is not defined in the dictionary draws a session-level Reject.
fn invalid_tag_number(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body.set(Field::string(999, "X")); // 999 undefined, below the UDF range
    scenario(
        "14a_InvalidTagNumber",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "0")), // Reject: InvalidTagNumber
        ],
    )
}

/// 14c — a defined field that is not valid for this message type draws a session-level Reject.
fn tag_not_defined_for_msg_type(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body.set(Field::string(112, "X")); // TestReqID is defined but not part of NewOrderSingle
    scenario(
        "14c_TagNotDefinedForMsgType",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "2")), // Reject: TagNotDefinedForMessageType
        ],
    )
}

/// 14d — a field present with no value draws a session-level Reject.
fn tag_specified_without_value(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body.set(Field::new(11, Vec::new())); // ClOrdID present but empty
    scenario(
        "14d_TagSpecifiedWithoutValue",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "4")), // Reject: TagSpecifiedWithoutValue
        ],
    )
}

/// 14f — a field whose value has the wrong data format draws a session-level Reject.
fn incorrect_data_format(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body.set(Field::string(38, "abc")); // OrderQty is a QTY; "abc" is not numeric
    scenario(
        "14f_IncorrectDataFormat",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "6")), // Reject: IncorrectDataFormat
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

/// Special suite — NextExpectedMsgSeqNum (789): a peer reporting it is behind our outbound
/// counter triggers an immediate resend (which, for admin-only traffic, is a GapFill).
fn next_expected_msg_seq_num(v: &str) -> Scenario {
    let mut lo = logon(v, 1, true);
    lo.body.set(Field::int(789, 1)); // peer next-expects our seq 1, but we will have sent the Logon
    scenario_with(
        "special_NextExpectedMsgSeqNum",
        v,
        vec![
            Step::Send(lo),
            // Having consumed inbound Logon seq 1, the server reports it next expects seq 2.
            Step::Expect(ExpectMsg::of("A").field(789, "2")),
            // The server's Logon already consumed outbound seq 1, so it fills the gap to 2.
            Step::Expect(ExpectMsg::of("4").field(123, "Y").field(36, "2")),
        ],
        SessionTweaks {
            enable_next_expected: true,
            ..SessionTweaks::default()
        },
    )
}

/// Special suite — LastMsgSeqNumProcessed (369): the server stamps the sequence number of the
/// last inbound message it processed, reflecting the just-consumed Logon.
fn last_msg_seq_num_processed(v: &str) -> Scenario {
    scenario_with(
        "special_LastMsgSeqNumProcessed",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            // After processing inbound Logon seq 1, LastMsgSeqNumProcessed is 1.
            Step::Expect(ExpectMsg::of("A").field(369, "1")),
        ],
        SessionTweaks {
            enable_last_processed: true,
            ..SessionTweaks::default()
        },
    )
}

/// Special suite — CheckLatency/timestamps: a Logon with a stale SendingTime is rejected with a
/// Logout citing a SendingTime accuracy problem, then the session is disconnected.
fn check_latency_timestamps(v: &str) -> Scenario {
    // client_message stamps a fixed 2024 SendingTime, far outside MaxLatency relative to "now".
    scenario_with(
        "special_CheckLatencyStaleSendingTime",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("5")), // Logout: SendingTime accuracy problem
            Step::ExpectDisconnect,
        ],
        SessionTweaks {
            check_latency: true,
            ..SessionTweaks::default()
        },
    )
}

/// Encode `msg` and corrupt its CheckSum digits, keeping BodyLength (and thus framing) intact so
/// the frame is delivered whole but fails checksum validation on decode.
fn garble_checksum(msg: &Message) -> Vec<u8> {
    let mut raw = msg.encode();
    // The trailer is always `10=DDD<SOH>`: the three checksum digits sit at len-4..len-1, and the
    // literal `10=` occupies the three bytes before them. Overwrite the digits with (real+1) mod
    // 256 so the frame stays well-formed (BodyLength intact) but fails checksum validation.
    let digits_at = match raw.len().checked_sub(4) {
        Some(p) => p,
        None => return raw,
    };
    let Some(cs_field_start) = digits_at.checked_sub(3) else {
        return raw;
    };
    let computed: u32 = raw
        .get(..cs_field_start)
        .map(|pre| pre.iter().map(|&b| u32::from(b)).sum::<u32>() & 0xFF)
        .unwrap_or(0);
    let wrong = format!("{:03}", (computed + 1) & 0xFF);
    if let Some(slot) = raw.get_mut(digits_at..digits_at + 3) {
        slot.copy_from_slice(wrong.as_bytes());
    }
    raw
}

/// Special suite — validateChecksum / rejectGarbledMessages (default N): a frame with a bad
/// CheckSum is silently dropped (no Reject, no disconnect, sequence counter untouched), so the
/// next correctly-sequenced message is still processed.
fn garbled_message_dropped(v: &str) -> Scenario {
    let garbled = garble_checksum(&client_message(v, "0", 2)); // Heartbeat with a broken CheckSum
    let mut tr = client_message(v, "1", 2); // reuses seq 2: the garbled frame was never counted
    tr.body.set(Field::string(112, "AFTER-GARBLE"));
    scenario(
        "special_GarbledMessageDropped",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::SendRaw(garbled),
            Step::Send(tr),
            // A Heartbeat at seq 2 proves the garbled frame was dropped without advancing state.
            Step::Expect(ExpectMsg::of("0").field(112, "AFTER-GARBLE")),
        ],
    )
}

/// Special suite — resendRequestChunkSize: when a gap is detected, the ResendRequest is bounded to
/// one chunk (EndSeqNo = BeginSeqNo + chunk - 1) instead of an open-ended range.
fn resend_request_chunk_size(v: &str) -> Scenario {
    scenario_with(
        "special_ResendRequestChunkSize",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(client_message(v, "0", 10)), // gap: expected 2, received 10
            // chunk=5 → request only 2..=6 rather than 2..0 (open-ended).
            Step::Expect(ExpectMsg::of("2").field(7, "2").field(16, "6")),
        ],
        SessionTweaks {
            resend_chunk_size: 5,
            ..SessionTweaks::default()
        },
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
        out.push(logon_adopts_heartbeat_interval(v));
        out.push(resend_request_gap_fill(v));
        out.push(resend_request_bounded_end(v));
        out.push(out_of_order_queued_then_drained(v));
        out.push(sequence_reset_reset(v));
        out.push(sequence_reset_gap_fill_advances(v));
        out.push(sequence_reset_gap_fill_backward_ignored(v));
        out.push(resend_request_nothing_to_resend(v));
        out.push(reject_message_consumed(v));
        out.push(heartbeat_consumed(v));
        out.push(missing_msg_seq_num(v));
        out.push(poss_dup_too_low(v));
    }
    // Field-validation scenarios for FIX.4.2 (its NewOrderSingle subset differs from 4.4).
    out.push(valid_new_order_accepted_42());
    out.push(required_field_missing_42());
    out.push(incorrect_enum_value_42());
    // Field-validation scenarios require the dictionary; authored for FIX.4.4.
    out.push(valid_new_order_accepted("FIX.4.4"));
    out.push(required_field_missing("FIX.4.4"));
    out.push(incorrect_enum_value("FIX.4.4"));
    out.push(invalid_tag_number("FIX.4.4"));
    out.push(tag_not_defined_for_msg_type("FIX.4.4"));
    out.push(tag_specified_without_value("FIX.4.4"));
    out.push(incorrect_data_format("FIX.4.4"));
    out.push(unregistered_msg_type("FIX.4.4"));
    // Special-category suites (T086) requiring per-acceptor session-feature toggles.
    out.push(next_expected_msg_seq_num("FIX.4.4"));
    out.push(last_msg_seq_num_processed("FIX.4.4"));
    out.push(check_latency_timestamps("FIX.4.4"));
    out.push(garbled_message_dropped("FIX.4.4"));
    out.push(resend_request_chunk_size("FIX.4.4"));
    out
}
