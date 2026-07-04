//! T063/T064 (US2, feature 007): an initiator's first connection sends `ResetSeqNumFlag=Y` when
//! any of `ResetOnLogon`/`ResetOnLogout`/`ResetOnDisconnect` is set, not only `ResetOnLogon`
//! (`BUG-92`/`BUG-109`/FR-025) — matching QuickFIX/J's `isResetNeeded()`/QuickFIX/Go's
//! `shouldSendReset()`, which both check all three configs (and require both sequence numbers to
//! already be 1, so a reset-on-logout/disconnect config that hasn't actually triggered a reset yet
//! doesn't spuriously claim `ResetSeqNumFlag=Y` on an unrelated non-1 sequence).

use truefix_core::Message;
use truefix_session::{Action, Event, Role, Session, SessionConfig};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    c.heartbeat_interval = 30;
    c.reset_on_logon = false;
    c
}

fn logon_out(actions: &[Action]) -> Option<&Message> {
    actions.iter().find_map(|a| match a {
        Action::Send(m) if m.msg_type() == Some("A") => Some(m),
        _ => None,
    })
}

#[test]
fn reset_on_logout_alone_sends_reset_seq_num_flag_on_a_fresh_connection() {
    let mut c = cfg();
    c.reset_on_logout = true; // NOT reset_on_logon
    let mut s = Session::new(c);
    let actions = s.handle(Event::Connected);
    let logon = logon_out(&actions).expect("Logon should be sent");
    assert_eq!(
        logon.body.get(141).and_then(|f| f.as_str().ok()),
        Some("Y"),
        "ResetOnLogout alone should still cause ResetSeqNumFlag=Y on a fresh (seq 1/1) connection"
    );
}

#[test]
fn reset_on_disconnect_alone_sends_reset_seq_num_flag_on_a_fresh_connection() {
    let mut c = cfg();
    c.reset_on_disconnect = true; // NOT reset_on_logon
    let mut s = Session::new(c);
    let actions = s.handle(Event::Connected);
    let logon = logon_out(&actions).expect("Logon should be sent");
    assert_eq!(
        logon.body.get(141).and_then(|f| f.as_str().ok()),
        Some("Y"),
        "ResetOnDisconnect alone should still cause ResetSeqNumFlag=Y on a fresh (seq 1/1) connection"
    );
}

#[test]
fn no_reset_config_at_all_never_sends_reset_seq_num_flag() {
    let mut s = Session::new(cfg()); // all three configs false
    let actions = s.handle(Event::Connected);
    let logon = logon_out(&actions).expect("Logon should be sent");
    assert!(
        logon.body.get(141).is_none(),
        "no ResetSeqNumFlag should be sent when none of the three reset configs are set"
    );
}

#[test]
fn reset_on_logout_does_not_send_the_flag_once_sequences_have_advanced_past_one() {
    // Simulate a session that has already progressed past seq 1 (e.g. resumed from a store) --
    // ResetOnLogout being configured doesn't mean a reset has actually happened yet, so the flag
    // must not be sent on an unrelated non-1 sequence (matching QFJ/QFGo's "both seqs == 1" gate).
    let mut c = cfg();
    c.reset_on_logout = true;
    let mut s = Session::new(c);
    s.seed_sequences(5, 5);
    let actions = s.handle(Event::Connected);
    let logon = logon_out(&actions).expect("Logon should be sent");
    assert!(
        logon.body.get(141).is_none(),
        "ResetSeqNumFlag must not be sent when sequences aren't both 1, even with a reset config \
         set, since no actual reset has happened"
    );
}
