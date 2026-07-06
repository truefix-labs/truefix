//! T053/T055 (US2, feature 009, `NEW-18`): a residual issue from BUG-42's own fix (feature 007).
//! `validLogonState` rejects non-Logon messages before logon, but allows `Some("A" | "5" | "4" |
//! "3")` through. A Logout(5), Reject(3), or gap-fill SequenceReset(4) with a too-high sequence
//! number arriving in `AwaitingLogon` state still reached the `Ordering::Greater` branch and
//! emitted a `ResendRequest` -- contradicting "no ResendRequests should be sent before logon".

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn header(mut m: Message, msg_type: &str, seq: i64) -> Message {
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

fn no_resend_request_sent(actions: &[Action]) -> bool {
    !actions
        .iter()
        .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("2")))
}

#[test]
fn pre_logon_too_high_logout_does_not_trigger_a_resend_request() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    assert_eq!(s.state(), SessionState::AwaitingLogon);

    let actions = s.handle(Event::Received(header(Message::new(), "5", 10)));
    assert!(
        no_resend_request_sent(&actions),
        "a too-high pre-logon Logout must not trigger a ResendRequest (NEW-18), got: {actions:?}"
    );
}

#[test]
fn pre_logon_too_high_reject_does_not_trigger_a_resend_request() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(header(Message::new(), "3", 10)));
    assert!(
        no_resend_request_sent(&actions),
        "a too-high pre-logon Reject must not trigger a ResendRequest (NEW-18), got: {actions:?}"
    );
}

#[test]
fn pre_logon_too_high_gap_fill_sequence_reset_does_not_trigger_a_resend_request() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    let mut m = header(Message::new(), "4", 10);
    m.body.set(Field::string(123, "Y")); // GapFillFlag
    m.body.set(Field::int(36, 11)); // NewSeqNo
    let actions = s.handle(Event::Received(m));
    assert!(
        no_resend_request_sent(&actions),
        "a too-high pre-logon gap-fill SequenceReset must not trigger a ResendRequest (NEW-18), \
         got: {actions:?}"
    );
}
