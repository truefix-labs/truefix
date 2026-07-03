//! T024 (US4) — the 12 previously-inert session config switches (FR-008). Session-level behavior
//! is tested here; three switches (`RefreshOnLogon`'s actual store re-fetch,
//! `ForceResendWhenCorruptedStore`, `LogMessageWhenSessionNotFound`) need the transport/store layer
//! and are tested in `crates/truefix-transport/tests/config_switches.rs` instead.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig};

fn inbound(msg_type: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

/// `hbi` is the HeartBtInt(108) the Logon carries. The acceptor adopts this as its effective
/// heartbeat interval (`on_logon`), overriding whatever `SessionConfig::heartbeat_interval` was
/// configured — so tests exercising heartbeat timing must pass the value they actually want.
fn logon(seq: i64, hbi: i64) -> Message {
    let mut m = inbound("A", seq);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, hbi));
    m
}

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn sends(actions: Vec<Action>) -> Vec<Message> {
    actions
        .into_iter()
        .filter_map(|a| match a {
            Action::Send(m) | Action::Resend(m, _) => Some(m),
            Action::Disconnect | Action::ResetStore => None,
        })
        .collect()
}

fn has_disconnect(actions: &[Action]) -> bool {
    actions.iter().any(|a| matches!(a, Action::Disconnect))
}

fn has_reset_store(actions: &[Action]) -> bool {
    actions.iter().any(|a| matches!(a, Action::ResetStore))
}

// --- SendRedundantResendRequests ---

#[test]
fn redundant_resend_requests_suppressed_by_default() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 30)));
    // First gap: seq 5 while expecting 2 -> triggers a ResendRequest.
    let first = sends(s.handle(Event::Received(inbound("0", 5))));
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].msg_type(), Some("2"));
    // A second gap arriving while the first resend is still pending: suppressed by default.
    let second = sends(s.handle(Event::Received(inbound("0", 7))));
    assert!(
        second.is_empty(),
        "redundant resend should be suppressed by default"
    );
}

#[test]
fn redundant_resend_requests_sent_when_enabled() {
    let mut cfg = acc_cfg();
    cfg.send_redundant_resend_requests = true;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 30)));
    let first = sends(s.handle(Event::Received(inbound("0", 5))));
    assert_eq!(first.len(), 1);
    let second = sends(s.handle(Event::Received(inbound("0", 7))));
    assert_eq!(
        second.len(),
        1,
        "SendRedundantResendRequests=true should send another ResendRequest"
    );
    assert_eq!(second[0].msg_type(), Some("2"));
}

// --- DisableHeartBeatCheck ---

#[test]
fn heartbeat_timeout_disconnects_by_default() {
    let mut cfg = acc_cfg();
    cfg.heartbeat_interval = 1;
    cfg.heartbeat_timeout_multiplier = 1;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 1)));
    // Timeout threshold is heartbeat_interval * multiplier + 2 = 3 ticks.
    let mut disconnected = false;
    for _ in 0..10 {
        if has_disconnect(&s.handle(Event::Tick)) {
            disconnected = true;
            break;
        }
    }
    assert!(
        disconnected,
        "a silent peer should eventually be disconnected by default"
    );
}

#[test]
fn disable_heart_beat_check_never_times_out() {
    let mut cfg = acc_cfg();
    cfg.heartbeat_interval = 1;
    cfg.heartbeat_timeout_multiplier = 1;
    cfg.disable_heart_beat_check = true;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 1)));
    // Without DisableHeartBeatCheck, this same interval/multiplier combo times out at tick 3
    // (see heartbeat_timeout_disconnects_by_default) — 20 ticks here proves the check is truly off.
    for _ in 0..20 {
        assert!(
            !has_disconnect(&s.handle(Event::Tick)),
            "DisableHeartBeatCheck should prevent timeout-driven disconnects"
        );
    }
}

// --- ResetOnError / DisconnectOnError ---

fn bad_begin_string(seq: i64) -> Message {
    let mut m = inbound("0", seq);
    m.header.set(Field::string(8, "FIX.9.9"));
    m
}

#[test]
fn reset_on_error_resets_on_identity_failure() {
    let mut cfg = acc_cfg();
    cfg.reset_on_error = true;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 30)));
    let actions = s.handle(Event::Received(bad_begin_string(2)));
    assert!(
        has_reset_store(&actions),
        "ResetOnError should emit Action::ResetStore"
    );
    assert!(
        has_disconnect(&actions),
        "identity failures still disconnect"
    );
}

#[test]
fn without_reset_on_error_identity_failure_only_disconnects() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 30)));
    let actions = s.handle(Event::Received(bad_begin_string(2)));
    assert!(!has_reset_store(&actions));
    assert!(has_disconnect(&actions));
}

#[test]
fn disconnect_on_error_disconnects_after_a_validation_reject() {
    let mut cfg = acc_cfg();
    cfg.disconnect_on_error = true;
    cfg.check_comp_id = false; // isolate: reach validate_app, not identity_problem
    let mut s = Session::new(cfg);
    s.set_dictionary(
        truefix_dict::load_fix44().unwrap(),
        truefix_dict::ValidationOptions::default(),
    );
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 30)));
    // An unregistered/invalid MsgType at the expected seq draws a session reject via validate_app.
    let mut bad = inbound("Z", 2); // "Z" is not a recognised admin type and not in the dictionary
    bad.body.set(Field::string(999, "x"));
    let actions = s.handle(Event::Received(bad));
    assert!(
        has_disconnect(&actions),
        "DisconnectOnError should disconnect after a validation reject, got {actions:?}"
    );
}

#[test]
fn without_disconnect_on_error_a_validation_reject_does_not_disconnect() {
    let mut cfg = acc_cfg();
    cfg.check_comp_id = false;
    let mut s = Session::new(cfg);
    s.set_dictionary(
        truefix_dict::load_fix44().unwrap(),
        truefix_dict::ValidationOptions::default(),
    );
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 30)));
    let mut bad = inbound("Z", 2);
    bad.body.set(Field::string(999, "x"));
    let actions = s.handle(Event::Received(bad));
    assert!(!has_disconnect(&actions));
}

// --- LogonTag ---

#[test]
fn logon_tag_is_appended_to_outbound_logon() {
    let mut cfg = acc_cfg();
    cfg.logon_tag = Some((9001, "HOUSE-ID".to_owned()));
    let mut s = Session::new(cfg);
    let actions = s.handle(Event::Connected); // acceptor sends nothing yet; use initiator instead
    assert!(sends(actions).is_empty());

    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.logon_tag = Some((9001, "HOUSE-ID".to_owned()));
    let mut init = Session::new(init_cfg);
    let sent = sends(init.handle(Event::Connected));
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].body.get(9001).unwrap().as_str().unwrap(),
        "HOUSE-ID"
    );
}

#[test]
fn no_logon_tag_by_default() {
    let mut init = Session::new(SessionConfig::new(
        "FIX.4.4",
        "CLIENT",
        "SERVER",
        Role::Initiator,
    ));
    let sent = sends(init.handle(Event::Connected));
    assert!(sent[0].body.get(9001).is_none());
}

// --- RefreshOnLogon (pure mechanism: Session::refresh_sequences) ---

#[test]
fn refresh_sequences_is_unconditional_unlike_seed_sequences() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 30))); // logon_sent becomes true here
    s.seed_sequences(50, 50); // no-op: logon already sent
    assert_ne!(s.next_out_seq(), 50);
    s.refresh_sequences(50, 60);
    assert_eq!(s.next_out_seq(), 50);
    assert_eq!(s.next_in_seq(), 60);
}

#[test]
fn refresh_on_logon_getter_reflects_config() {
    let mut cfg = acc_cfg();
    assert!(!Session::new(cfg.clone()).refresh_on_logon());
    cfg.refresh_on_logon = true;
    assert!(Session::new(cfg).refresh_on_logon());
}

// --- Recognised-for-parity, intentionally-no-op switches (documented design decisions) ---

#[test]
fn closed_resend_interval_defaults_false_and_has_no_observable_effect() {
    let default_cfg = acc_cfg();
    let mut enabled_cfg = acc_cfg();
    enabled_cfg.closed_resend_interval = true;

    for cfg in [default_cfg, enabled_cfg] {
        let mut s = Session::new(cfg);
        s.handle(Event::Connected);
        s.handle(Event::Received(logon(1, 30)));
        let out = sends(s.handle(Event::Received(inbound("0", 5))));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].msg_type(), Some("2"));
    }
}

#[test]
fn reject_message_on_unhandled_exception_and_max_scheduled_write_requests_and_continue_initialization_on_error_round_trip(
) {
    let mut cfg = acc_cfg();
    cfg.reject_message_on_unhandled_exception = true;
    cfg.max_scheduled_write_requests = Some(100);
    cfg.continue_initialization_on_error = true;
    // These fields are recognised for QuickFIX/J config-key parity but are documented as
    // intentionally not honored in truefix-session (see config.rs doc comments) — this test only
    // proves they round-trip through SessionConfig without affecting ordinary session behavior.
    let mut s = Session::new(cfg);
    let sent = sends(s.handle(Event::Connected));
    assert!(sent.is_empty()); // acceptor: no output yet, just proves no panic/side effect
}
