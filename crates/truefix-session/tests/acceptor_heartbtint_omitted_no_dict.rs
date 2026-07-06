//! T095/T096 (US2, feature 009, `NEW-71`): without dictionary validation, an acceptor must still
//! enforce the Logon protocol requirement that `HeartBtInt(108)` is present and positive.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn config(role: Role) -> SessionConfig {
    let mut config = SessionConfig::new("FIX.4.4", "LOCAL", "PEER", role);
    config.check_latency = false;
    config.reset_on_logon = false;
    config
}

fn logon(heart_bt_int: Option<i64>) -> Message {
    let mut message = Message::new();
    message.header.set(Field::string(8, "FIX.4.4"));
    message.header.set(Field::string(35, "A"));
    message.header.set(Field::string(49, "PEER"));
    message.header.set(Field::string(56, "LOCAL"));
    message.header.set(Field::int(34, 1));
    message.header.set(Field::string(52, "20240101-00:00:00"));
    message.body.set(Field::int(98, 0));
    if let Some(value) = heart_bt_int {
        message.body.set(Field::int(108, value));
    }
    message
}

fn receive_logon(role: Role, heart_bt_int: Option<i64>) -> (Session, Vec<Action>) {
    let mut session = Session::new(config(role));
    session.handle(Event::Connected);
    let actions = session.handle(Event::Received(logon(heart_bt_int)));
    (session, actions)
}

#[test]
fn acceptor_without_dictionary_rejects_logon_missing_heart_bt_int() {
    let (session, actions) = receive_logon(Role::Acceptor, None);
    assert_eq!(session.state(), SessionState::Disconnected);
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, Action::Disconnect)),
        "missing HeartBtInt must reject and disconnect: {actions:?}"
    );
    assert!(
        actions.iter().any(
            |action| matches!(action, Action::Send(message) if message.msg_type() == Some("5"))
        ),
        "Logon rejection must send Logout, not a successful Logon response: {actions:?}"
    );
}

#[test]
fn acceptor_without_dictionary_rejects_zero_heart_bt_int() {
    let (session, actions) = receive_logon(Role::Acceptor, Some(0));
    assert_eq!(session.state(), SessionState::Disconnected);
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, Action::Disconnect))
    );
}

#[test]
fn acceptor_still_accepts_a_positive_heart_bt_int() {
    let (session, actions) = receive_logon(Role::Acceptor, Some(30));
    assert_eq!(session.state(), SessionState::LoggedOn);
    assert!(
        actions.iter().any(
            |action| matches!(action, Action::Send(message) if message.msg_type() == Some("A"))
        )
    );
}

#[test]
fn initiator_logon_response_boundary_is_unchanged() {
    let (session, actions) = receive_logon(Role::Initiator, None);
    assert_eq!(
        session.state(),
        SessionState::LoggedOn,
        "T096 is acceptor-only; initiator response handling keeps its configured heartbeat"
    );
    assert!(
        actions
            .iter()
            .all(|action| !matches!(action, Action::Disconnect))
    );
}
