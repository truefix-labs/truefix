//! T032–T035 — sequence recovery, resend, SequenceReset, and NextExpectedMsgSeqNum.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg(role: Role) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", role);
    c.heartbeat_interval = 30;
    c.reset_on_logon = true;
    c.check_latency = false; // fixtures use fixed timestamps; not testing latency here
    c
}

fn msg(msg_type: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

fn with(mut m: Message, tag: u32, value: &str) -> Message {
    m.body.set(Field::string(tag, value));
    m
}

fn with_header(mut m: Message, tag: u32, value: &str) -> Message {
    m.header.set(Field::string(tag, value));
    m
}

fn sends(actions: &[Action]) -> Vec<&Message> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Send(m) | Action::Resend(m, _) => Some(m),
            Action::Disconnect | Action::ResetStore => None,
        })
        .collect()
}

fn logged_on_acceptor() -> Session {
    let mut s = Session::new(cfg(Role::Acceptor));
    s.handle(Event::Connected);
    // Logon with ResetSeqNumFlag=Y, seq 1
    let logon = with(msg("A", 1), 108, "30");
    let logon = with(logon, 141, "Y");
    s.handle(Event::Received(logon));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.next_in_seq(), 2);
    s
}

#[test]
fn high_seq_triggers_resend_request_and_queues() {
    let mut s = logged_on_acceptor();
    // expected = 2; receive seq 5 -> gap
    let actions = s.handle(Event::Received(msg("0", 5)));
    let out = sends(&actions);
    assert_eq!(out.len(), 1);
    let rr = out[0];
    assert_eq!(rr.msg_type(), Some("2")); // ResendRequest
    assert_eq!(rr.body.get(7).unwrap().as_int().unwrap(), 2); // BeginSeqNo = expected
                                                              // still expecting 2 (the high message is queued, not processed)
    assert_eq!(s.next_in_seq(), 2);
}

#[test]
fn gap_fills_then_queued_message_processed() {
    let mut s = logged_on_acceptor();
    s.handle(Event::Received(msg("0", 5))); // queue 5, request resend from 2
                                            // peer gap-fills 2..5 via SequenceReset-GapFill NewSeqNo=5
    let sr = with(with(msg("4", 2), 123, "Y"), 36, "5");
    s.handle(Event::Received(sr));
    // now expected should have advanced through the queued 5 -> 6
    assert_eq!(s.next_in_seq(), 6);
}

#[test]
fn low_seq_without_possdup_disconnects() {
    let mut s = logged_on_acceptor(); // expected 2
    let actions = s.handle(Event::Received(msg("0", 1))); // too low, no PossDup
    assert_eq!(s.state(), SessionState::Disconnected);
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn low_seq_with_possdup_is_ignored() {
    let mut s = logged_on_acceptor(); // expected 2
    let dup = with_header(msg("0", 1), 43, "Y"); // PossDupFlag=Y (header field)
    let actions = s.handle(Event::Received(dup));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert!(actions.is_empty());
    assert_eq!(s.next_in_seq(), 2);
}

// --- T018 (US3, feature 005): PossDup anti-replay check (GAP-08/FR-008/FR-009) ---

#[test]
fn possdup_with_orig_sending_time_later_than_sending_time_disconnects() {
    let mut s = logged_on_acceptor(); // expected 2
    let mut dup = with_header(msg("0", 1), 43, "Y"); // PossDupFlag=Y
    dup.header.set(Field::string(52, "20240101-00:00:00")); // SendingTime
    dup.header.set(Field::string(122, "20240101-00:05:00")); // OrigSendingTime, LATER than SendingTime
    let actions = s.handle(Event::Received(dup));
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a falsified OrigSendingTime on a PossDup message must trigger logout+disconnect \
         (same severity as duplicate-Logon)"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("5"))),
        "expected a Logout"
    );
}

#[test]
fn possdup_with_orig_sending_time_not_later_is_still_silently_ignored() {
    let mut s = logged_on_acceptor();
    let mut dup = with_header(msg("0", 1), 43, "Y");
    dup.header.set(Field::string(52, "20240101-00:05:00"));
    dup.header.set(Field::string(122, "20240101-00:00:00")); // earlier, legitimate
    let actions = s.handle(Event::Received(dup));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert!(actions.is_empty());
}

#[test]
fn requires_orig_sending_time_on_low_seq_rejects_a_missing_orig_sending_time() {
    let mut c = cfg(Role::Acceptor);
    c.requires_orig_sending_time_on_low_seq = true;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    let logon = with(msg("A", 1), 108, "30");
    let logon = with(logon, 141, "Y");
    s.handle(Event::Received(logon));
    assert_eq!(s.next_in_seq(), 2);

    let dup = with_header(msg("0", 1), 43, "Y"); // PossDupFlag=Y, no OrigSendingTime at all
    let actions = s.handle(Event::Received(dup));
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a missing OrigSendingTime must be rejected when the switch is enabled"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn resend_request_resends_app_with_possdup_and_gapfills_admin() {
    // Build an initiator that has sent: logon(1, admin), an app msg(2), a heartbeat(3, admin).
    let mut s = Session::new(cfg(Role::Initiator));
    s.handle(Event::Connected); // logon seq 1 (admin)
    s.send_app(with(msg("D", 0), 55, "AAPL")); // app seq 2 (seq stamped by engine)
                                               // log on so test requests/heartbeats are valid; simulate counter logon
    let logon = with(with(msg("A", 1), 108, "30"), 141, "Y");
    s.handle(Event::Received(logon));

    // Peer asks us to resend 1..3
    let rr = {
        let mut m = msg("2", 2);
        m.body.set(Field::int(7, 1));
        m.body.set(Field::int(16, 3));
        m
    };
    let actions = s.handle(Event::Received(rr));
    let out = sends(&actions);
    // Expect: gap-fill for the admin logon(1), then resent app(2) with PossDup, then gap-fill for 3+
    assert!(
        out.iter().any(|m| m.msg_type() == Some("4")),
        "expected a SequenceReset-GapFill for admin messages"
    );
    let resent_app = out
        .iter()
        .find(|m| m.msg_type() == Some("D"))
        .expect("app message should be resent");
    assert_eq!(resent_app.header.get(43).unwrap().as_str().unwrap(), "Y"); // PossDupFlag
    assert!(resent_app.header.get(122).is_some(), "OrigSendingTime set"); // OrigSendingTime
}

// --- T017/T023 (US3, feature 005): resend veto + GapFill substitution (GAP-07/FR-007) ---

#[test]
fn resent_app_messages_use_action_resend_not_action_send() {
    // The veto point (`Application::to_app`) already fires for every `Action::Send`; the actual
    // GAP-07 gap is that a vetoed *resend* needs a compensating GapFill, unlike a vetoed *live*
    // send. `Action::Resend` is how `build_resend` marks a message as resend-originated so the
    // transport can tell the two cases apart.
    let mut s = Session::new(cfg(Role::Initiator));
    s.handle(Event::Connected); // logon seq 1 (admin)
    s.send_app(with(msg("D", 0), 55, "AAPL")); // app seq 2
    let logon = with(with(msg("A", 1), 108, "30"), 141, "Y");
    s.handle(Event::Received(logon));

    let rr = {
        let mut m = msg("2", 2);
        m.body.set(Field::int(7, 1));
        m.body.set(Field::int(16, 3));
        m
    };
    let actions = s.handle(Event::Received(rr));
    let resend = actions
        .iter()
        .find_map(|a| match a {
            Action::Resend(m, seq) => Some((m, *seq)),
            _ => None,
        })
        .expect("the resent app message should be Action::Resend, not Action::Send");
    assert_eq!(resend.0.msg_type(), Some("D"));
    assert_eq!(resend.1, 2);
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("D"))),
        "the app message must not also appear as a plain Action::Send"
    );
}

#[test]
fn gap_fill_after_veto_covers_exactly_the_vetoed_sequence_number() {
    let mut s = logged_on_acceptor(); // next_in_seq = 2, next_out_seq starts at 2 after Logon(1)
    let action = s.gap_fill_after_veto(5);
    match action {
        Action::Send(m) => {
            assert_eq!(m.msg_type(), Some("4")); // SequenceReset
            assert_eq!(m.body.get(123).unwrap().as_str().unwrap(), "Y"); // GapFillFlag
            assert_eq!(m.header.get(34).unwrap().as_int().unwrap(), 5); // MsgSeqNum = the vetoed seq
            assert_eq!(m.body.get(36).unwrap().as_int().unwrap(), 6); // NewSeqNo = vetoed seq + 1
        }
        other => panic!("expected a GapFill Action::Send, got {other:?}"),
    }
}

#[test]
fn chunked_resend_request_caps_end() {
    let mut c = cfg(Role::Acceptor);
    c.resend_request_chunk_size = 3;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    let logon = with(with(msg("A", 1), 108, "30"), 141, "Y");
    s.handle(Event::Received(logon)); // expected 2

    let actions = s.handle(Event::Received(msg("0", 10))); // big gap from 2
    let out = sends(&actions);
    let rr = out.iter().find(|m| m.msg_type() == Some("2")).unwrap();
    assert_eq!(rr.body.get(7).unwrap().as_int().unwrap(), 2); // begin
    assert_eq!(rr.body.get(16).unwrap().as_int().unwrap(), 4); // end = 2 + 3 - 1
}

// --- T030 (US4, feature 005): chunked-resend auto-continuation (GAP-09/FR-011) ---

#[test]
fn multi_chunk_inbound_resend_auto_continues_without_an_external_resend_request() {
    let mut c = cfg(Role::Acceptor);
    c.resend_request_chunk_size = 3;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    let logon = with(with(msg("A", 1), 108, "30"), 141, "Y");
    s.handle(Event::Received(logon)); // expected 2

    let actions = s.handle(Event::Received(msg("0", 10))); // gap: expected 2, got 10
    let out = sends(&actions);
    let rr = out.iter().find(|m| m.msg_type() == Some("2")).unwrap();
    assert_eq!(rr.body.get(7).unwrap().as_int().unwrap(), 2); // chunk 1 begin
    assert_eq!(rr.body.get(16).unwrap().as_int().unwrap(), 4); // chunk 1 end = 2 + 3 - 1

    // Peer gap-fills chunk 1 (2..4) -> NewSeqNo=5. No external nudge: the session must
    // auto-issue chunk 2's ResendRequest on its own.
    let sr = with(with(msg("4", 2), 123, "Y"), 36, "5");
    let actions = s.handle(Event::Received(sr));
    assert_eq!(s.next_in_seq(), 5);
    let out = sends(&actions);
    let rr2 = out
        .iter()
        .find(|m| m.msg_type() == Some("2"))
        .expect("expected an auto-issued ResendRequest continuing the chunked resend");
    assert_eq!(rr2.body.get(7).unwrap().as_int().unwrap(), 5);
    assert_eq!(rr2.body.get(16).unwrap().as_int().unwrap(), 7); // 5 + 3 - 1

    // Peer gap-fills chunk 2 (5..7) -> NewSeqNo=8. The full gap (target=10) isn't closed yet
    // (8 <= 10), so a third chunk must auto-issue too, capped by the known target.
    let sr2 = with(with(msg("4", 3), 123, "Y"), 36, "8");
    let actions = s.handle(Event::Received(sr2));
    assert_eq!(s.next_in_seq(), 8);
    let out = sends(&actions);
    let rr3 = out
        .iter()
        .find(|m| m.msg_type() == Some("2"))
        .expect("expected a third auto-issued ResendRequest, since target(10) not yet reached");
    assert_eq!(rr3.body.get(7).unwrap().as_int().unwrap(), 8);
    assert_eq!(rr3.body.get(16).unwrap().as_int().unwrap(), 10); // capped by target=10, not 10+3-1

    // Peer gap-fills the final chunk (8..10) -> NewSeqNo=11, past the target: the gap is now
    // fully closed, so no further ResendRequest should be auto-issued.
    let sr3 = with(with(msg("4", 4), 123, "Y"), 36, "11");
    let actions = s.handle(Event::Received(sr3));
    assert_eq!(s.next_in_seq(), 11);
    let out = sends(&actions);
    assert!(
        !out.iter().any(|m| m.msg_type() == Some("2")),
        "gap fully closed; no further ResendRequest expected"
    );
}

#[test]
fn sequence_reset_reset_mode_sets_expected() {
    let mut s = logged_on_acceptor(); // expected 2
                                      // Reset mode (no GapFill): NewSeqNo=10 authoritative
    let sr = with(msg("4", 99), 36, "10");
    s.handle(Event::Received(sr));
    assert_eq!(s.next_in_seq(), 10);
}

#[test]
fn last_msg_seq_num_processed_stamped_when_enabled() {
    let mut c = cfg(Role::Initiator);
    c.enable_last_msg_seq_num_processed = true;
    let mut s = Session::new(c);
    let actions = s.handle(Event::Connected);
    let logon = sends(&actions)[0];
    // next_in starts at 1, so last processed = 0
    assert_eq!(logon.header.get(369).unwrap().as_int().unwrap(), 0);
}

#[test]
fn next_expected_msg_seq_num_triggers_resend() {
    let mut s = Session::new(cfg(Role::Initiator));
    s.handle(Event::Connected); // logon seq 1 (admin)
    s.send_app(with(msg("D", 0), 55, "AAPL")); // app seq 2 -> next_out = 3

    // Counter logon reporting NextExpectedMsgSeqNum=2 (peer hasn't seen seq 2 yet).
    let mut logon = with(with(msg("A", 1), 108, "30"), 141, "Y");
    logon.body.set(Field::int(789, 2));
    let actions = s.handle(Event::Received(logon));
    let out = sends(&actions);

    let resent = out
        .iter()
        .find(|m| m.msg_type() == Some("D"))
        .expect("app message resent because peer expects from seq 2");
    assert_eq!(resent.header.get(43).unwrap().as_str().unwrap(), "Y"); // PossDupFlag
}

#[test]
fn seed_sequences_resumes_outbound_seq() {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Initiator);
    c.reset_on_logon = false; // otherwise logon resets to 1
    let mut s = Session::new(c);
    s.seed_sequences(5, 7);
    let actions = s.handle(Event::Connected);
    let logon = sends(&actions)[0];
    assert_eq!(logon.header.get(34).unwrap().as_int().unwrap(), 5); // resumes from seeded value
    assert_eq!(s.next_in_seq(), 7);
}

#[test]
fn next_expected_msg_seq_num_included_on_logon_when_enabled() {
    let mut c = cfg(Role::Initiator);
    c.enable_next_expected_msg_seq_num = true;
    let mut s = Session::new(c);
    let actions = s.handle(Event::Connected);
    let out = sends(&actions);
    let logon = out.iter().find(|m| m.msg_type() == Some("A")).unwrap();
    assert!(
        logon.body.get(789).is_some(),
        "NextExpectedMsgSeqNum (789) should be present on logon"
    );
    assert_eq!(logon.body.get(789).unwrap().as_int().unwrap(), 1);
}
