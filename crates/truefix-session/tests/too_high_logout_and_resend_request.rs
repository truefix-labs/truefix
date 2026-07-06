use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn config() -> SessionConfig {
    let mut config = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    config.check_latency = false;
    config.reset_on_logon = false;
    config
}

fn message(msg_type: &str, seq: i64) -> Message {
    let mut message = Message::new();
    message.header.set(Field::string(8, "FIX.4.4"));
    message.header.set(Field::string(35, msg_type));
    message.header.set(Field::int(34, seq));
    message.header.set(Field::string(49, "YOU"));
    message.header.set(Field::string(56, "ME"));
    message.header.set(Field::string(52, "20240101-00:00:00"));
    message
}

fn logged_on() -> Session {
    let mut session = Session::new(config());
    session.handle(Event::Connected);
    let mut logon = message("A", 1);
    logon.body.set(Field::int(98, 0));
    logon.body.set(Field::int(108, 30));
    session.handle(Event::Received(logon));
    assert_eq!(session.state(), SessionState::LoggedOn);
    assert_eq!(session.next_in_seq(), 2);
    session
}

fn sent_types(actions: &[Action]) -> Vec<&str> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Send(message) | Action::Resend(message, _) => message.msg_type(),
            Action::Disconnect | Action::ResetStore => None,
        })
        .collect()
}

#[test]
fn too_high_logout_is_processed_immediately() {
    let mut session = logged_on();

    let actions = session.handle(Event::Received(message("5", 5)));

    assert_eq!(session.state(), SessionState::Disconnected);
    assert_eq!(session.queued_len(), 0);
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, Action::Disconnect))
    );
    assert_eq!(
        sent_types(&actions),
        ["5"],
        "reply with Logout immediately; do not queue it or request the missing sequence range"
    );
}

#[test]
fn too_high_resend_request_is_not_answered_again_when_drained() {
    let mut session = logged_on();
    let mut request = message("2", 4);
    request.body.set(Field::int(7, 1));
    request.body.set(Field::int(16, 1));

    let initial = session.handle(Event::Received(request));
    assert_eq!(
        sent_types(&initial),
        ["4", "2"],
        "answer the ResendRequest immediately, then request our own gap"
    );

    session.handle(Event::Received(message("0", 2)));
    let drained = session.handle(Event::Received(message("0", 3)));

    assert!(
        !sent_types(&drained).contains(&"4"),
        "the already-answered queued ResendRequest must not emit a duplicate GapFill"
    );
    assert_eq!(session.next_in_seq(), 5);
    assert_eq!(session.queued_len(), 0);
}
