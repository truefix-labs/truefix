//! T049/T050 (US2, feature 007): a non-Logon message arriving before the session completes its
//! Logon exchange is rejected (BUG-42/FR-018) — QFJ's `validLogonState()` rejects any message
//! other than Logon/Logout/SequenceReset/Reject while not yet logged on, disconnecting the
//! session; TrueFix previously had no such check at all, so e.g. a too-high-seq Heartbeat arriving
//! pre-Logon would be queued and draw a ResendRequest -- a protocol violation (no ResendRequests
//! before logon).

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
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

#[test]
fn a_heartbeat_arriving_before_logon_is_rejected_and_disconnects() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    assert_eq!(s.state(), SessionState::AwaitingLogon);

    let actions = s.handle(Event::Received(msg("0", 1))); // Heartbeat, not a Logon
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a non-Logon message before the session logs on must be rejected, not processed"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn a_too_high_seq_heartbeat_before_logon_does_not_queue_or_request_resend() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);

    // Before the fix: a too-high seq here would be queued and draw a ResendRequest -- a protocol
    // violation, since no ResendRequests should be sent before logon.
    let actions = s.handle(Event::Received(msg("0", 5)));
    let sent_resend_request = actions.iter().any(|a| match a {
        Action::Send(m) => m.msg_type() == Some("2"),
        _ => false,
    });
    assert!(
        !sent_resend_request,
        "must not send a ResendRequest for a too-high-seq message received before logon -- got \
         {actions:?}"
    );
    assert_eq!(s.state(), SessionState::Disconnected);
}

#[test]
fn logon_sequence_reset_and_reject_are_still_processed_normally_before_logon() {
    // These three must reach their own normal handling (not the new validLogonState rejection) --
    // none of them naturally disconnects the session on its own, unlike Logout.
    for mt in ["A", "4", "3"] {
        let mut s = Session::new(cfg());
        s.handle(Event::Connected);
        let _ = s.handle(Event::Received(msg(mt, 1)));
        assert_ne!(
            s.state(),
            SessionState::Disconnected,
            "MsgType {mt} must still be allowed before logon (validLogonState's own allow-list), \
             but the session disconnected"
        );
    }
}

#[test]
fn a_logout_before_logon_is_processed_normally_not_rejected_by_valid_logon_state() {
    // Logout naturally disconnects the session either way -- what matters here is that it reaches
    // its own normal Logout handling (a Logout *response*), not the new validLogonState rejection
    // path (whose Logout instead carries "Logon state is not valid").
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(msg("5", 1)));
    assert_eq!(s.state(), SessionState::Disconnected);
    let logout_text: Vec<_> = actions
        .iter()
        .filter_map(|a| match a {
            Action::Send(m) if m.msg_type() == Some("5") => {
                m.body.get(58).and_then(|f| f.as_str().ok())
            }
            _ => None,
        })
        .collect();
    assert!(
        !logout_text.contains(&"Logon state is not valid"),
        "an inbound Logout before logon must be processed by its own normal handling, not \
         rejected by the new validLogonState check -- got {actions:?}"
    );
}
