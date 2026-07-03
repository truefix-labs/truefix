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

/// The FIX versions the server suite is exercised against (FR-002; US9 adds `FIX.Latest`, the
/// tenth dictionary — the whole version-agnostic core (logon/sequencing/resend/admin) runs
/// against it exactly as it does the other nine, since `start_acceptor`'s session-layer protocol
/// logic never depends on a per-version dictionary being loaded — dictionary-backed field
/// validation scenarios remain separately authored for FIX.4.2/FIX.4.4 only, unaffected).
pub const SUITE_VERSIONS: &[&str] = &[
    "FIX.4.0",
    "FIX.4.1",
    "FIX.4.2",
    "FIX.4.3",
    "FIX.4.4",
    "FIX.5.0",
    "FIX.5.0SP1",
    "FIX.5.0SP2",
    "FIX.Latest",
];

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

/// ReverseRoute — a reply to a message carrying OnBehalfOf* routing fields reverses them onto
/// DeliverTo* on the reply.
fn reverse_route(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 2);
    tr.body.set(Field::string(112, "ROUTE-ME"));
    tr.header.set(Field::string(115, "BROKER")); // OnBehalfOfCompID
    scenario(
        "ReverseRoute",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr),
            Step::Expect(
                ExpectMsg::of("0")
                    .field(112, "ROUTE-ME")
                    .field(128, "BROKER"),
            ), // DeliverToCompID
        ],
    )
}

/// ReverseRouteWithEmptyRoutingTags — a routing tag present but empty still reverses.
fn reverse_route_empty_tags(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 2);
    tr.body.set(Field::string(112, "ROUTE-EMPTY"));
    tr.header.set(Field::string(115, "")); // OnBehalfOfCompID, present but empty
    scenario(
        "ReverseRouteWithEmptyRoutingTags",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "ROUTE-EMPTY").field(128, "")),
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

/// GAP-08 (US3, feature 005) — a PossDup message whose OrigSendingTime is later than its
/// SendingTime (an anti-replay violation — the message claims to have originally been sent
/// *after* the resend that is carrying it) draws a Logout and disconnect, not the silent ignore
/// that `poss_dup_too_low` above exercises for a legitimate low-seq PossDup.
fn poss_dup_orig_sending_time_after_sending_time(v: &str) -> Scenario {
    let mut dup = client_message(v, "0", 1); // Heartbeat, seq too low
    dup.header.set(Field::string(43, "Y")); // PossDupFlag
    dup.header.set(Field::string(122, "20240101-00:00:05")); // OrigSendingTime > SendingTime(52)
    scenario(
        "GAP08_PossDupOrigSendingTimeAfterSendingTime",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(dup),
            Step::Expect(ExpectMsg::of("5")), // Logout: anti-replay violation
            Step::ExpectDisconnect,
        ],
    )
}

/// GAP-18a (US3, feature 005) — a second Logon received on an already-logged-on session is
/// rejected (Logout + disconnect), not silently reprocessed as if it were the first.
fn duplicate_logon_rejected(v: &str) -> Scenario {
    scenario(
        "GAP18a_DuplicateLogonRejected",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(logon(v, 2, false)),
            Step::Expect(ExpectMsg::of("5")), // Logout: duplicate Logon
            Step::ExpectDisconnect,
        ],
    )
}

/// 1d — with ResetOnLogon (default), the acceptor's Logon response carries ResetSeqNumFlag=Y.
fn logon_response_carries_reset_flag(v: &str) -> Scenario {
    scenario(
        "1d_LogonResponseResetFlag",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A").field(141, "Y")),
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

/// 0_idle — after the negotiated heartbeat interval of silence, the acceptor emits a Heartbeat.
fn idle_heartbeat_emitted(v: &str) -> Scenario {
    let mut lo = logon(v, 1, true);
    lo.body.set(Field::int(108, 1)); // 1-second heartbeat so the timer fires quickly
    scenario(
        "0_IdleHeartbeatEmitted",
        v,
        vec![
            Step::Send(lo),
            Step::Expect(ExpectMsg::of("A")),
            Step::Expect(ExpectMsg::of("0")), // unprompted Heartbeat from the idle timer
        ],
    )
}

/// 4_silence — after the counterparty is silent past the heartbeat interval, the acceptor issues a
/// TestRequest (TestReqID=TEST).
fn test_request_on_silence(v: &str) -> Scenario {
    let mut lo = logon(v, 1, true);
    lo.body.set(Field::int(108, 1));
    scenario(
        "4_TestRequestOnSilence",
        v,
        vec![
            Step::Send(lo),
            Step::Expect(ExpectMsg::of("A")),
            Step::Expect(ExpectMsg::of("0")), // first idle Heartbeat
            // Continued silence → the acceptor probes liveness with a TestRequest.
            Step::Expect(ExpectMsg::of("1").field(112, "TEST")),
        ],
    )
}

/// 5_initiated — an acceptor-initiated Logout (graceful): the server sends Logout, the client acks,
/// and the server disconnects.
fn acceptor_initiated_logout(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body.set(Field::string(11, "LOGOUT")); // sentinel: triggers monitor.force_logout
    scenario_with(
        "5_AcceptorInitiatedLogout",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("5")), // acceptor-initiated Logout
            Step::Send(client_message(v, "5", 3)), // client acknowledges
            Step::ExpectDisconnect,
        ],
        SessionTweaks {
            executor_app: true,
            ..SessionTweaks::default()
        },
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

/// 2_dedup — once a gap has triggered a ResendRequest, further too-high messages are queued
/// without emitting a second ResendRequest (the resend-pending guard).
fn resend_request_not_duplicated(v: &str) -> Scenario {
    let mut tr2 = client_message(v, "1", 2);
    tr2.body.set(Field::string(112, "TWO"));
    scenario(
        "2_ResendRequestNotDuplicated",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(client_message(v, "0", 4)), // seq 4: gap → one ResendRequest
            Step::Expect(ExpectMsg::of("2").field(7, "2")),
            Step::Send(client_message(v, "0", 5)), // seq 5: still too high → no second request
            Step::Send(tr2),                       // fills seq 2
            // The next message must be the Heartbeat for seq 2, not a duplicate ResendRequest.
            Step::Expect(ExpectMsg::of("0").field(112, "TWO")),
        ],
    )
}

/// 2_begin0 — a ResendRequest with BeginSeqNo=0 is ignored (no SequenceReset emitted).
fn resend_request_begin_zero_ignored(v: &str) -> Scenario {
    let mut rr = client_message(v, "2", 2);
    rr.body.set(Field::int(7, 0)); // BeginSeqNo=0
    rr.body.set(Field::int(16, 0));
    let mut tr = client_message(v, "1", 3); // ResendRequest consumed seq 2
    tr.body.set(Field::string(112, "ZERO-OK"));
    scenario(
        "2_ResendRequestBeginZeroIgnored",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(rr),
            Step::Send(tr),
            // A Heartbeat (not a SequenceReset) proves the begin=0 request was ignored.
            Step::Expect(ExpectMsg::of("0").field(112, "ZERO-OK")),
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
    m.body.set(Field::string(21, "1")); // HandlInst
    m.body.set(Field::string(55, "AAPL")); // Symbol
    m.body.set(Field::string(54, "1")); // Side
    m.body.set(Field::string(60, "20240101-00:00:00")); // TransactTime
    m.body.set(Field::int(38, 100)); // OrderQty
    m.body.set(Field::string(40, "2")); // OrdType
    m
}

/// 14a (FIX.4.2) — a tag not defined in the 4.2 dictionary draws a session-level Reject.
fn invalid_tag_number_42() -> Scenario {
    let mut order = new_order_single_42(2);
    order.body.set(Field::string(999, "X")); // 999 undefined, below the UDF range
    scenario(
        "14a_InvalidTagNumber_42",
        "FIX.4.2",
        vec![
            Step::Send(logon("FIX.4.2", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "0")), // InvalidTagNumber
        ],
    )
}

/// 14c (FIX.4.2) — a defined tag (TestReqID 112) not valid for NewOrderSingle draws a Reject.
fn tag_not_defined_for_msg_type_42() -> Scenario {
    let mut order = new_order_single_42(2);
    order.body.set(Field::string(112, "X")); // defined in 4.2, but not a NewOrderSingle field
    scenario(
        "14c_TagNotDefinedForMsgType_42",
        "FIX.4.2",
        vec![
            Step::Send(logon("FIX.4.2", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "2")), // TagNotDefinedForMessageType
        ],
    )
}

/// 14d (FIX.4.2) — a present-but-empty ClOrdID(11) draws a Reject (TagSpecifiedWithoutValue).
fn tag_specified_without_value_42() -> Scenario {
    let mut order = new_order_single_42(2);
    order.body.set(Field::new(11, Vec::new())); // ClOrdID present but empty
    scenario(
        "14d_TagSpecifiedWithoutValue_42",
        "FIX.4.2",
        vec![
            Step::Send(logon("FIX.4.2", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "4")), // TagSpecifiedWithoutValue
        ],
    )
}

/// 14f (FIX.4.2) — a non-numeric OrderQty(38) draws a session-level Reject (IncorrectDataFormat).
fn incorrect_data_format_42() -> Scenario {
    let mut order = new_order_single_42(2);
    order.body.set(Field::string(38, "abc")); // OrderQty is a QTY; "abc" is not numeric
    scenario(
        "14f_IncorrectDataFormat_42",
        "FIX.4.2",
        vec![
            Step::Send(logon("FIX.4.2", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "6")), // IncorrectDataFormat
        ],
    )
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

/// 14b (FIX.4.2) — a NewOrderSingle missing required HandlInst(21) draws a session-level Reject
/// (US9, feature 005, FR-031: OrderQty(38) is optional in the real bundled FIX.4.2 dictionary —
/// HandlInst/Side/TransactTime/OrdType are NewOrderSingle's actual directly-required fields).
fn required_field_missing_42() -> Scenario {
    let mut order = new_order_single_42(2);
    order.body = {
        let mut b = truefix_core::FieldMap::new();
        for f in new_order_single_42(2).body.fields() {
            if f.tag() != 21 {
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

/// 14e (FIX.4.2) — an out-of-enumeration Side value draws a session-level Reject.
fn incorrect_enum_value_42() -> Scenario {
    let mut order = new_order_single_42(2);
    order.body.set(Field::string(54, "Z")); // not a real Side value
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

/// A FIX.4.4 NewOrderSingle with the given repeating-group fields appended in wire order.
fn nos_with_group(seq: i64, group: &[(u32, &str)]) -> Message {
    let mut m = new_order_single(seq);
    for (t, v) in group {
        m.body.set(Field::string(*t, v));
    }
    m
}

/// 14i — a NoXxx count that does not match the number of group entries draws a session Reject.
fn group_count_mismatch(v: &str) -> Scenario {
    let order = nos_with_group(2, &[(453, "2"), (448, "A"), (447, "1"), (452, "1")]);
    scenario(
        "14i_RepeatingGroupCountNotEqual",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "16")), // IncorrectNumInGroupCount
        ],
    )
}

/// 14j — out-of-order repeating-group members draw a session Reject.
fn group_out_of_order(v: &str) -> Scenario {
    // Entry: delimiter 448, then 452 (PartyRole), then 447 (PartyIDSource) — out of order.
    let order = nos_with_group(2, &[(453, "1"), (448, "A"), (452, "1"), (447, "1")]);
    scenario(
        "14j_OutOfOrderRepeatingGroupMembers",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "15")), // RepeatingGroupFieldsOutOfOrder (15)
        ],
    )
}

/// QFJ934 — a nested group whose entry omits its delimiter draws a session Reject.
fn nested_group_missing_delimiter(v: &str) -> Scenario {
    // NoPartySubIDs entry starts with 803 instead of the delimiter 523.
    let order = nos_with_group(
        2,
        &[
            (453, "1"),
            (448, "A"),
            (447, "1"),
            (452, "1"),
            (802, "1"),
            (803, "1"),
            (523, "S"),
        ],
    );
    scenario(
        "QFJ934_MissingDelimiterNestedRepeatingGroup",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "15")),
        ],
    )
}

/// 21 — a repeating-group count of zero is accepted as an empty group.
fn group_zero_count(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 3);
    tr.body.set(Field::string(112, "ZERO-GRP-OK"));
    let order = nos_with_group(2, &[(453, "0")]);
    scenario(
        "21_RepeatingGroupSpecifierWithValueOfZero",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Send(tr),
            // A Heartbeat (not a Reject) proves the zero-count group was accepted.
            Step::Expect(ExpectMsg::of("0").field(112, "ZERO-GRP-OK")),
        ],
    )
}

/// app1 — an active acceptor fills each NewOrderSingle with an ExecutionReport (35=8).
fn app_order_executed(v: &str) -> Scenario {
    scenario_with(
        "app_NewOrderSingleExecuted",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(new_order_single(2)),
            // ExecutionReport echoing ClOrdID with ExecType=New.
            Step::Expect(ExpectMsg::of("8").field(11, "ORDER-1").field(150, "0")),
        ],
        SessionTweaks {
            executor_app: true,
            ..SessionTweaks::default()
        },
    )
}

/// app2 — successive application responses carry increasing outbound MsgSeqNum.
fn app_orders_sequenced(v: &str) -> Scenario {
    scenario_with(
        "app_OrdersOutboundSequenced",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(new_order_single(2)),
            Step::Expect(ExpectMsg::of("8").field(34, "2")), // first ExecutionReport: outbound seq 2
            Step::Send(new_order_single(3)),
            Step::Expect(ExpectMsg::of("8").field(34, "3")), // second ExecutionReport: outbound seq 3
        ],
        SessionTweaks {
            executor_app: true,
            ..SessionTweaks::default()
        },
    )
}

/// app3 — a ResendRequest for a stored application message replays it as a PossDup (43=Y), not a
/// GapFill (this is the application-resend path, distinct from admin gap-fill).
fn app_message_resent_as_poss_dup(v: &str) -> Scenario {
    let mut rr = client_message(v, "2", 3); // after logon(1) + order(2), this is seq 3
    rr.body.set(Field::int(7, 2)); // BeginSeqNo = the ExecutionReport's outbound seq
    rr.body.set(Field::int(16, 2)); // EndSeqNo = 2
    scenario_with(
        "app_MessageResentAsPossDup",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(new_order_single(2)),
            Step::Expect(ExpectMsg::of("8").field(34, "2")), // ExecutionReport at outbound seq 2
            Step::Send(rr),
            // Replayed application message: same MsgSeqNum, now flagged PossDup.
            Step::Expect(ExpectMsg::of("8").field(34, "2").field(43, "Y")),
        ],
        SessionTweaks {
            executor_app: true,
            ..SessionTweaks::default()
        },
    )
}

/// app4 — a ResendRequest spanning admin + application traffic yields a GapFill for the admin
/// Logon followed by a PossDup replay of the ExecutionReport.
fn app_mixed_resend_gapfill_then_possdup(v: &str) -> Scenario {
    let mut rr = client_message(v, "2", 3);
    rr.body.set(Field::int(7, 1)); // BeginSeqNo = 1 (the admin Logon)
    rr.body.set(Field::int(16, 2)); // EndSeqNo = 2 (the ExecutionReport)
    scenario_with(
        "app_MixedResendGapFillThenPossDup",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(new_order_single(2)),
            Step::Expect(ExpectMsg::of("8").field(34, "2")),
            Step::Send(rr),
            // Admin Logon(1) collapses to a GapFill up to seq 2 ...
            Step::Expect(ExpectMsg::of("4").field(123, "Y").field(36, "2")),
            // ... then the application ExecutionReport(2) is replayed as a PossDup.
            Step::Expect(ExpectMsg::of("8").field(34, "2").field(43, "Y")),
        ],
        SessionTweaks {
            executor_app: true,
            ..SessionTweaks::default()
        },
    )
}

/// GAP-07 (US3, feature 005) — an application-vetoed resend is replaced by a
/// SequenceReset-GapFill on the wire (FR-007), not replayed as a PossDup. Contrast with
/// `app_message_resent_as_poss_dup` above: identical shape, except the executor app's
/// "VETO-RESEND" sentinel ClOrdID (`runner::AtApp::to_app`) vetoes the resend specifically (the
/// original live send, seq 2, still goes out and is acknowledged normally).
fn app_resend_veto_produces_gap_fill(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body.set(Field::string(11, "VETO-RESEND"));
    let mut rr = client_message(v, "2", 3); // after logon(1) + order(2), this is seq 3
    rr.body.set(Field::int(7, 2)); // BeginSeqNo = the ExecutionReport's outbound seq
    rr.body.set(Field::int(16, 2)); // EndSeqNo = 2
    scenario_with(
        "GAP07_AppResendVetoProducesGapFill",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("8").field(34, "2")), // live send: not vetoed
            Step::Send(rr),
            // Vetoed resend: a GapFill covering exactly the vetoed sequence number, not a
            // PossDup replay of the ExecutionReport.
            Step::Expect(ExpectMsg::of("4").field(123, "Y").field(36, "3")),
        ],
        SessionTweaks {
            executor_app: true,
            ..SessionTweaks::default()
        },
    )
}

/// 14b — a NewOrderSingle missing required Side(54) draws a session-level Reject (US9, feature
/// 005, FR-031: HandlInst(21) is optional in the real bundled FIX.4.4 dictionary — Side, along
/// with ClOrdID/TransactTime/OrdType, is NewOrderSingle's actual directly-required field).
fn required_field_missing(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body = {
        let mut b = truefix_core::FieldMap::new();
        for f in new_order_single(2).body.fields() {
            if f.tag() != 54 {
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
    order.body.set(Field::string(54, "Z")); // not a real Side value
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

/// 14h — a tag appearing more than once outside a repeating group draws a session-level Reject.
fn repeated_tag(v: &str) -> Scenario {
    let mut order = new_order_single(2);
    order.body.add_field(Field::string(11, "ORDER-1-DUP")); // ClOrdID(11) repeated
    scenario(
        "14h_RepeatedTag",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "13")), // Reject: TagAppearsMoreThanOnce
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

/// Special suite — RejectGarbledMessage=Y: a garbled frame draws a session-level Reject (35=3,
/// SessionRejectReason=0) instead of a silent drop, and the sequence counter is untouched.
fn garbled_message_rejected(v: &str) -> Scenario {
    let garbled = garble_checksum(&client_message(v, "0", 2));
    let mut tr = client_message(v, "1", 2); // reuses seq 2: the garbled frame was never counted
    tr.body.set(Field::string(112, "AFTER-GARBLE-REJECT"));
    scenario_with(
        "special_RejectGarbledMessage",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::SendRaw(garbled),
            Step::Expect(ExpectMsg::of("3").field(373, "0")), // session Reject
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "AFTER-GARBLE-REJECT")),
        ],
        SessionTweaks {
            reject_garbled: true,
            ..SessionTweaks::default()
        },
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

/// GAP-09 (US4, feature 005) — a chunked inbound resend spanning more than one chunk
/// auto-continues without an external re-request: each GapFill that closes one chunk but not the
/// full known gap immediately draws the next chunk's `ResendRequest` from TrueFix itself, since a
/// well-behaved reference-engine counterparty never re-requests the remainder unprompted.
fn chunked_resend_auto_continues(v: &str) -> Scenario {
    let mut gf1 = client_message(v, "4", 2);
    gf1.body.set(Field::string(123, "Y")); // GapFillFlag
    gf1.body.set(Field::int(36, 5)); // NewSeqNo: closes chunk 1 (2..4)
    let mut gf2 = client_message(v, "4", 3);
    gf2.body.set(Field::string(123, "Y"));
    gf2.body.set(Field::int(36, 8)); // closes chunk 2 (5..7)
    let mut gf3 = client_message(v, "4", 4);
    gf3.body.set(Field::string(123, "Y"));
    gf3.body.set(Field::int(36, 11)); // closes chunk 3 (8..10); the full gap (target=10) is done
    let mut tr = client_message(v, "1", 11); // matches next_in_seq after gf3's NewSeqNo=11
    tr.body.set(Field::string(112, "SETTLED"));
    scenario_with(
        "GAP09_ChunkedResendAutoContinues",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(client_message(v, "0", 10)), // gap: expected 2, got 10
            Step::Expect(ExpectMsg::of("2").field(7, "2").field(16, "4")), // chunk 1: 2..4
            Step::Send(gf1),
            // No external nudge: the next chunk's ResendRequest is auto-issued.
            Step::Expect(ExpectMsg::of("2").field(7, "5").field(16, "7")), // chunk 2: 5..7
            Step::Send(gf2),
            Step::Expect(ExpectMsg::of("2").field(7, "8").field(16, "10")), // chunk 3: 8..10
            Step::Send(gf3),
            Step::Send(tr),
            // A Heartbeat (not a fourth ResendRequest) proves the gap is now fully closed.
            Step::Expect(ExpectMsg::of("0").field(112, "SETTLED")),
        ],
        SessionTweaks {
            resend_chunk_size: 3,
            ..SessionTweaks::default()
        },
    )
}

// --- 003 US1: identity/CompID/logon-integrity class (T012) ---
//
// NOTE: `1c_InvalidSenderCompID`/`1c_InvalidTargetCompID`/`1d_InvalidLogonWrongBeginString` (a
// mismatch on the *Logon itself*) cannot be represented against this harness's dynamic-template
// acceptor (`start_acceptor`): the template adopts whatever CompIDs/BeginString the first Logon
// claims, rather than checking them against a pre-existing fixed identity, so there is nothing to
// "mismatch" on a first connection. `identity_problem`'s CheckCompID/BeginString logic is instead
// proven mid-session, where a fixed identity already exists from the earlier Logon — see
// `begin_string_value_unexpected` / `comp_id_does_not_match_profile` below. Testing the Logon-time
// variant would need a non-dynamic (fixed-identity) acceptor mode in this test harness, which is a
// harness change, not scenario authoring — tracked as a follow-up, not attempted here.

/// 1d — a Logon carrying a stale SendingTime (outside MaxLatency) draws a Logout and disconnect.
/// Same underlying `CheckLatency` mechanism as the `special_CheckLatencyStaleSendingTime` suite,
/// exposed here under its Appendix B name and run across the full version matrix.
fn invalid_logon_bad_sending_time(v: &str) -> Scenario {
    scenario_with(
        "1d_InvalidLogonBadSendingTime",
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

/// QFJ648 — a Logon with a negative HeartBtInt does not crash or corrupt session state; the
/// invalid value is ignored and the acceptor's configured heartbeat interval is kept, and the
/// session logs on normally (defensive handling, not a protocol violation reply).
fn qfj648_negative_heart_bt_int(v: &str) -> Scenario {
    let mut lo = logon(v, 1, true);
    lo.body.set(Field::int(108, -1)); // HeartBtInt, invalid
    scenario(
        "QFJ648_NegativeHeartBtInt",
        v,
        vec![
            Step::Send(lo),
            // Logon still succeeds (echoing the *default* HeartBtInt=30 the harness template
            // configures, not the invalid -1) rather than a Reject/disconnect.
            Step::Expect(ExpectMsg::of("A").field(108, "30")),
        ],
    )
}

// --- 003 US1: sequence/PossDup class (T013) ---

/// 2a — a message whose MsgSeqNum exactly matches the expected inbound sequence number is
/// processed normally (the baseline case every other sequencing scenario builds on).
fn msgseqnum_correct(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 2); // TestRequest at exactly the expected seq
    tr.body.set(Field::string(112, "INSEQ"));
    scenario(
        "2a_MsgSeqNumCorrect",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "INSEQ")), // Heartbeat echo, not a reject
        ],
    )
}

/// 2e — a message carrying PossDupFlag=Y at exactly the expected sequence number is not actually
/// a duplicate (the flag doesn't match reality); it is processed normally rather than specially
/// rejected or ignored — only `Ordering::Less` PossDup traffic gets the "already received" pass.
fn poss_dup_not_received(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 2);
    tr.header.set(Field::string(43, "Y")); // PossDupFlag, but this seq was never actually sent before
    tr.body.set(Field::string(112, "NOTADUP"));
    scenario(
        "2e_PossDupNotReceived",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "NOTADUP")),
        ],
    )
}

/// 10 — a SequenceReset-Reset (not GapFill) with NewSeqNo equal to the current expected sequence
/// is accepted (a no-op re-affirmation), unlike GapFill's backward case.
fn seq_reset_new_seq_no_equal(v: &str) -> Scenario {
    let mut sr = client_message(v, "4", 99);
    sr.body.set(Field::int(36, 2)); // NewSeqNo == current expected (2), no GapFillFlag → Reset
    let mut tr = client_message(v, "1", 2);
    tr.body.set(Field::string(112, "STILL-2"));
    scenario(
        "10_MsgSeqNumEqual",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(sr),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "STILL-2")),
        ],
    )
}

/// 10 — a SequenceReset-Reset (not GapFill) with NewSeqNo *behind* the current expected sequence
/// is still honored unconditionally (Reset, unlike GapFill, has no backward guard).
fn seq_reset_new_seq_no_less(v: &str) -> Scenario {
    let mut sr = client_message(v, "4", 99);
    sr.body.set(Field::int(36, 1)); // NewSeqNo=1, behind the current expected (2)
    let mut tr = client_message(v, "1", 1); // now-expected sequence per the (backward) reset
    tr.body.set(Field::string(112, "REWOUND"));
    scenario(
        "10_MsgSeqNumLess",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(sr),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "REWOUND")),
        ],
    )
}

// --- 003 US1: resend/reset class (T014) ---

/// 11b — a SequenceReset-GapFill with NewSeqNo equal to the current expected sequence is accepted
/// (the `>=` guard passes; distinct from `11a`/`11c`, already covered by the existing
/// `sequence_reset_gap_fill_advances`/`sequence_reset_gap_fill_backward_ignored` scenarios).
fn seq_reset_gap_fill_new_seq_no_equal(v: &str) -> Scenario {
    let mut sr = client_message(v, "4", 2); // GapFill's own MsgSeqNum is consumed as usual
    sr.body.set(Field::string(123, "Y")); // GapFillFlag
    sr.body.set(Field::int(36, 3)); // NewSeqNo == the seq the GapFill message itself would advance to
    let mut tr = client_message(v, "1", 3);
    tr.body.set(Field::string(112, "GAPFILL-EQ"));
    scenario(
        "11b_NewSeqNoEqual",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(sr),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "GAPFILL-EQ")),
        ],
    )
}

/// RejectResentMessage — a replayed (PossDupFlag=Y) message that itself fails dictionary
/// validation still draws a session-level Reject; PossDup does not exempt a message from
/// validation (requires the FIX.4.4 dictionary; scoped to that version like the other
/// dictionary-validation scenarios).
fn reject_resent_message() -> Scenario {
    let mut order = new_order_single(2);
    order.body = {
        // rebuild body without Side(54), the same required field `required_field_missing`
        // demonstrates, so this exercises validation rather than a fresh failure mode.
        let mut b = truefix_core::FieldMap::new();
        for f in new_order_single(2).body.fields() {
            if f.tag() != 54 {
                b.set(Field::new(f.tag(), f.value_bytes().to_vec()));
            }
        }
        b
    };
    order.header.set(Field::string(43, "Y")); // PossDupFlag
    order.header.set(Field::string(122, "20240101-00:00:00")); // OrigSendingTime, required with PossDup
    scenario(
        "RejectResentMessage",
        "FIX.4.4",
        vec![
            Step::Send(logon("FIX.4.4", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(order),
            Step::Expect(ExpectMsg::of("3").field(373, "1")), // Reject: RequiredTagMissing
        ],
    )
}

// --- 003 US1: message-type/admin-app class (T015) ---

/// 2i — a BeginString mismatch on a *post-logon* message (not the Logon itself) draws a Logout
/// and disconnect, proving `CheckCompID`'s BeginString guard applies to every inbound message.
fn begin_string_value_unexpected(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 2);
    tr.header.set(Field::string(8, "FIX.9.9"));
    scenario(
        "2i_BeginStringValueUnexpected",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("5")), // Logout: Incorrect BeginString
            Step::ExpectDisconnect,
        ],
    )
}

/// 2k — a post-logon message whose CompIDs don't match the session profile draws a Logout and
/// disconnect (the same `CheckCompID` guard as `1c_Invalid*CompID`, exercised mid-session).
fn comp_id_does_not_match_profile(v: &str) -> Scenario {
    let mut tr = client_message(v, "1", 2);
    tr.header.set(Field::string(49, "IMPOSTER"));
    scenario(
        "2k_CompIDDoesNotMatchProfile",
        v,
        vec![
            Step::Send(logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("5")), // Logout: CompID problem
            Step::ExpectDisconnect,
        ],
    )
}

/// A Logon with SendingTime stamped to the real current time (rather than `client_message`'s fixed
/// 2024 timestamp), so a `CheckLatency`-enabled session accepts it.
fn fresh_logon(version: &str, seq: i64, reset: bool) -> Message {
    let mut m = logon(version, seq, reset);
    m.header
        .set(Field::utc_timestamp(52, time::OffsetDateTime::now_utc()));
    m
}

/// 2o — a post-logon message with a stale SendingTime (outside MaxLatency) draws a Logout and
/// disconnect, the same `CheckLatency` guard as `1d_InvalidLogonBadSendingTime` exercised
/// mid-session (after a Logon whose own SendingTime is fresh) rather than at logon.
fn sending_time_value_out_of_range(v: &str) -> Scenario {
    let tr = client_message(v, "1", 2); // fixed 2024 SendingTime, stale relative to "now"
    scenario_with(
        "2o_SendingTimeValueOutOfRange",
        v,
        vec![
            Step::Send(fresh_logon(v, 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("5")), // Logout: SendingTime accuracy problem
            Step::ExpectDisconnect,
        ],
        SessionTweaks {
            check_latency: true,
            ..SessionTweaks::default()
        },
    )
}

/// 8 — a stream of only admin (session-level) messages is processed in sequence with no
/// application-layer involvement.
fn only_admin_messages() -> Scenario {
    let mut tr1 = client_message("FIX.4.4", "1", 2);
    tr1.body.set(Field::string(112, "A1"));
    let mut tr2 = client_message("FIX.4.4", "1", 3);
    tr2.body.set(Field::string(112, "A2"));
    scenario(
        "8_OnlyAdminMessages",
        "FIX.4.4",
        vec![
            Step::Send(logon("FIX.4.4", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(tr1),
            Step::Expect(ExpectMsg::of("0").field(112, "A1")),
            Step::Send(tr2),
            Step::Expect(ExpectMsg::of("0").field(112, "A2")),
        ],
    )
}

/// 8 — a stream of only application messages (no admin traffic beyond the Logon) is processed in
/// sequence, each acknowledged by the executor app.
fn only_application_messages() -> Scenario {
    scenario_with(
        "8_OnlyApplicationMessages",
        "FIX.4.4",
        vec![
            Step::Send(logon("FIX.4.4", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(new_order_single(2)),
            Step::Expect(ExpectMsg::of("8")),
            Step::Send(new_order_single(3)),
            Step::Expect(ExpectMsg::of("8")),
        ],
        SessionTweaks {
            executor_app: true,
            ..SessionTweaks::default()
        },
    )
}

/// 8 — admin and application messages interleaved in the same stream are each dispatched to the
/// correct layer, in order, without cross-contamination.
fn admin_and_application_messages() -> Scenario {
    let mut tr = client_message("FIX.4.4", "1", 3);
    tr.body.set(Field::string(112, "MIXED"));
    scenario_with(
        "8_AdminAndApplicationMessages",
        "FIX.4.4",
        vec![
            Step::Send(logon("FIX.4.4", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::Send(new_order_single(2)),
            Step::Expect(ExpectMsg::of("8")),
            Step::Send(tr),
            Step::Expect(ExpectMsg::of("0").field(112, "MIXED")),
            Step::Send(new_order_single(4)),
            Step::Expect(ExpectMsg::of("8")),
        ],
        SessionTweaks {
            executor_app: true,
            ..SessionTweaks::default()
        },
    )
}

// --- 003 US3: field-order validation (T022) ---
//
// These require `ValidateFieldsOutOfOrder` enabled on the acceptor's validator (the
// `validate_fields_out_of_order` `SessionTweaks` field), and a non-admin application message —
// `Session::validate_app` skips admin-type messages (including Logon) entirely, so the Logon
// itself is never a vehicle for this check. Raw bytes are sent directly (`Step::SendRaw`) since
// `Message::encode()` always re-emits the canonical header/body/trailer order, which would erase
// the very out-of-order-ness under test.

/// Build a well-formed FIX.4.4 NewOrderSingle frame from `rest` (every field after BeginString/
/// BodyLength), computing BodyLength/CheckSum correctly regardless of `rest`'s order.
fn raw_new_order_single(rest: &[(u32, &str)]) -> Vec<u8> {
    let body: String = rest.iter().map(|(t, v)| format!("{t}={v}\x01")).collect();
    let prefix = format!("8=FIX.4.4\x019={}\x01", body.len());
    let pre_checksum = format!("{prefix}{body}");
    let checksum: u32 = pre_checksum.bytes().map(u32::from).sum::<u32>() & 0xFF;
    format!("{pre_checksum}10={checksum:03}\x01").into_bytes()
}

/// 2t — the third field on the wire must be MsgType(35); here SenderCompID(49) usurps that slot.
fn first_three_fields_out_of_order() -> Scenario {
    let raw = raw_new_order_single(&[
        (49, "CLIENT"),
        (35, "D"),
        (56, "SERVER"),
        (34, "2"),
        (52, "20240101-00:00:00"),
        (11, "O1"),
        (21, "1"),
        (55, "AAPL"),
        (54, "1"),
        (60, "20240101-00:00:00"),
        (40, "2"),
    ]);
    scenario_with(
        "2t_FirstThreeFieldsOutOfOrder",
        "FIX.4.4",
        vec![
            Step::Send(logon("FIX.4.4", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::SendRaw(raw),
            Step::Expect(ExpectMsg::of("3").field(373, "14")), // Reject: TagOutOfRequiredOrder
        ],
        SessionTweaks {
            validate_fields_out_of_order: true,
            ..SessionTweaks::default()
        },
    )
}

/// 14g — a trailer-classified field (CheckSum aside) arriving before the body section is done, or
/// a header field arriving after body fields have started, violates header/body/trailer
/// sectioning.
fn header_body_trailer_fields_out_of_order() -> Scenario {
    let raw = raw_new_order_single(&[
        (35, "D"),
        (55, "AAPL"),   // body field
        (49, "CLIENT"), // header field, arriving after a body field already appeared
        (56, "SERVER"),
        (34, "2"),
        (52, "20240101-00:00:00"),
        (11, "O1"),
        (21, "1"),
        (54, "1"),
        (60, "20240101-00:00:00"),
        (40, "2"),
    ]);
    scenario_with(
        "14g_HeaderBodyTrailerFieldsOutOfOrder",
        "FIX.4.4",
        vec![
            Step::Send(logon("FIX.4.4", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::SendRaw(raw),
            Step::Expect(ExpectMsg::of("3").field(373, "14")), // Reject: TagOutOfRequiredOrder
        ],
        SessionTweaks {
            validate_fields_out_of_order: true,
            ..SessionTweaks::default()
        },
    )
}

/// 15 — header and body fields interleaved throughout the message (not just one field out of
/// place), still caught by the same sectioning check.
fn header_and_body_fields_ordered_differently() -> Scenario {
    let raw = raw_new_order_single(&[
        (35, "D"),
        (49, "CLIENT"),
        (55, "AAPL"),   // body field
        (56, "SERVER"), // header field, after a body field already appeared
        (11, "O1"),
        (34, "2"),
        (52, "20240101-00:00:00"),
        (21, "1"),
        (54, "1"),
        (60, "20240101-00:00:00"),
        (40, "2"),
    ]);
    scenario_with(
        "15_HeaderAndBodyFieldsOrderedDifferently",
        "FIX.4.4",
        vec![
            Step::Send(logon("FIX.4.4", 1, true)),
            Step::Expect(ExpectMsg::of("A")),
            Step::SendRaw(raw),
            Step::Expect(ExpectMsg::of("3").field(373, "14")), // Reject: TagOutOfRequiredOrder
        ],
        SessionTweaks {
            validate_fields_out_of_order: true,
            ..SessionTweaks::default()
        },
    )
}

/// Special suite — validateChecksum: TrueFix validates the wire checksum unconditionally at
/// decode time (a documented design decision — see `ValidationOptions::validate_checksum`'s doc
/// comment — checksum enforcement is not an optional safety property), so a bad-checksum frame is
/// always caught via the garbled-message path regardless of any validation toggle. Reuses the
/// existing `garbled_message_dropped`/`garbled_message_rejected` scenarios as this suite's content.
pub fn validate_checksum_suite() -> Vec<Scenario> {
    vec![
        garbled_message_dropped("FIX.4.4"),
        garbled_message_rejected("FIX.4.4"),
    ]
}

/// Special suite — timestamps: SendingTime-accuracy (CheckLatency) validation, the representative
/// timestamp-handling behavior class this harness can assert today (exact wire-format-precision
/// assertions on *outbound* SendingTime — e.g. millisecond vs. second precision on a non-fixed,
/// "now"-derived value — would need `ExpectMsg` to support predicate/format matching rather than
/// only exact-value matching; a disclosed harness-capability gap, not a protocol gap, tracked in
/// `docs/todo-gap-analysis.md`'s TODO-01). Reuses `check_latency_timestamps` as this suite's content.
pub fn timestamps_suite() -> Vec<Scenario> {
    vec![check_latency_timestamps("FIX.4.4")]
}

/// Special suite — resynch: session resynchronization behavior (ResendRequest gap recovery,
/// SequenceReset Reset/GapFill in both directions, out-of-order queueing/draining, and chunked
/// resend) — QuickFIX/J's `resynch` AT category. Reuses the existing resend/reset scenarios as
/// this suite's content (the same scenarios already run per-version inside `server_suite()`; this
/// function groups them as their own discoverable, independently-runnable suite).
pub fn resynch_suite() -> Vec<Scenario> {
    vec![
        resend_request_gap_fill("FIX.4.4"),
        resend_request_bounded_end("FIX.4.4"),
        resend_request_not_duplicated("FIX.4.4"),
        resend_request_begin_zero_ignored("FIX.4.4"),
        resend_request_nothing_to_resend("FIX.4.4"),
        out_of_order_queued_then_drained("FIX.4.4"),
        sequence_reset_reset("FIX.4.4"),
        sequence_reset_gap_fill_advances("FIX.4.4"),
        sequence_reset_gap_fill_backward_ignored("FIX.4.4"),
        resend_request_chunk_size("FIX.4.4"),
        chunked_resend_auto_continues("FIX.4.4"),
    ]
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
        out.push(reverse_route(v));
        out.push(reverse_route_empty_tags(v));
        out.push(unsolicited_logout(v));
        out.push(logon_response_carries_reset_flag(v));
        out.push(logon_adopts_heartbeat_interval(v));
        out.push(resend_request_gap_fill(v));
        out.push(resend_request_bounded_end(v));
        out.push(resend_request_not_duplicated(v));
        out.push(resend_request_begin_zero_ignored(v));
        out.push(out_of_order_queued_then_drained(v));
        out.push(sequence_reset_reset(v));
        out.push(sequence_reset_gap_fill_advances(v));
        out.push(sequence_reset_gap_fill_backward_ignored(v));
        out.push(resend_request_nothing_to_resend(v));
        out.push(reject_message_consumed(v));
        out.push(heartbeat_consumed(v));
        out.push(missing_msg_seq_num(v));
        out.push(idle_heartbeat_emitted(v));
        out.push(test_request_on_silence(v));
        out.push(poss_dup_too_low(v));
        // 005 US3: session-state-machine protocol-correctness safeguards (T021).
        out.push(poss_dup_orig_sending_time_after_sending_time(v));
        out.push(duplicate_logon_rejected(v));
        // 003 US1: identity/CompID/logon-integrity + sequence/PossDup classes (T012/T013).
        out.push(invalid_logon_bad_sending_time(v));
        out.push(qfj648_negative_heart_bt_int(v));
        out.push(msgseqnum_correct(v));
        out.push(poss_dup_not_received(v));
        out.push(seq_reset_new_seq_no_equal(v));
        out.push(seq_reset_new_seq_no_less(v));
        out.push(seq_reset_gap_fill_new_seq_no_equal(v));
        out.push(begin_string_value_unexpected(v));
        out.push(comp_id_does_not_match_profile(v));
        out.push(sending_time_value_out_of_range(v));
    }
    // Field-validation scenarios for FIX.4.2 (its NewOrderSingle subset differs from 4.4).
    out.push(valid_new_order_accepted_42());
    out.push(required_field_missing_42());
    out.push(incorrect_enum_value_42());
    out.push(invalid_tag_number_42());
    out.push(tag_not_defined_for_msg_type_42());
    out.push(tag_specified_without_value_42());
    out.push(incorrect_data_format_42());
    // Field-validation scenarios require the dictionary; authored for FIX.4.4.
    out.push(valid_new_order_accepted("FIX.4.4"));
    out.push(group_count_mismatch("FIX.4.4"));
    out.push(group_out_of_order("FIX.4.4"));
    out.push(nested_group_missing_delimiter("FIX.4.4"));
    out.push(group_zero_count("FIX.4.4"));
    out.push(app_order_executed("FIX.4.4"));
    out.push(app_orders_sequenced("FIX.4.4"));
    out.push(app_message_resent_as_poss_dup("FIX.4.4"));
    out.push(app_mixed_resend_gapfill_then_possdup("FIX.4.4"));
    out.push(acceptor_initiated_logout("FIX.4.4"));
    // 005 US3: resend-veto → GapFill (T021), single-version since it uses new_order_single.
    out.push(app_resend_veto_produces_gap_fill("FIX.4.4"));
    out.push(required_field_missing("FIX.4.4"));
    out.push(incorrect_enum_value("FIX.4.4"));
    out.push(invalid_tag_number("FIX.4.4"));
    out.push(tag_not_defined_for_msg_type("FIX.4.4"));
    out.push(tag_specified_without_value("FIX.4.4"));
    out.push(incorrect_data_format("FIX.4.4"));
    out.push(repeated_tag("FIX.4.4"));
    out.push(unregistered_msg_type("FIX.4.4"));
    // Special-category suites (T086) requiring per-acceptor session-feature toggles.
    out.push(next_expected_msg_seq_num("FIX.4.4"));
    out.push(last_msg_seq_num_processed("FIX.4.4"));
    out.push(check_latency_timestamps("FIX.4.4"));
    out.push(garbled_message_dropped("FIX.4.4"));
    out.push(garbled_message_rejected("FIX.4.4"));
    out.push(resend_request_chunk_size("FIX.4.4"));
    // 005 US4: chunked-resend auto-continuation (T031, GAP-09/FR-011).
    out.push(chunked_resend_auto_continues("FIX.4.4"));
    // 003 US3: field-order validation (T022).
    out.push(first_three_fields_out_of_order());
    out.push(header_body_trailer_fields_out_of_order());
    out.push(header_and_body_fields_ordered_differently());
    // 003 US1: resend/reset + message-type/admin-app classes (T014/T015), dictionary/app-dependent.
    out.push(reject_resent_message());
    out.push(only_admin_messages());
    out.push(only_application_messages());
    out.push(admin_and_application_messages());
    out
}
