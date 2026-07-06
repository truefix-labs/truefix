//! T092/T094 (US2, feature 009, `NEW-70`): `NewSeqNo(36)` is required in gap-fill mode just as
//! it is in reset mode. Missing it must produce a required-tag-missing Reject without advancing
//! either role's incoming sequence number.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn message(msg_type: &str, seq: i64) -> Message {
    let mut message = Message::new();
    message.header.set(Field::string(8, "FIX.4.4"));
    message.header.set(Field::string(35, msg_type));
    message.header.set(Field::string(49, "PEER"));
    message.header.set(Field::string(56, "LOCAL"));
    message.header.set(Field::int(34, seq));
    message.header.set(Field::string(52, "20240101-00:00:00"));
    message
}

fn logged_on(role: Role) -> Session {
    let mut config = SessionConfig::new("FIX.4.4", "LOCAL", "PEER", role);
    config.check_latency = false;
    let mut session = Session::new(config);
    session.handle(Event::Connected);

    let mut logon = message("A", 1);
    logon.body.set(Field::int(98, 0));
    logon.body.set(Field::int(108, 30));
    session.handle(Event::Received(logon));
    assert_eq!(session.state(), SessionState::LoggedOn);
    assert_eq!(session.next_in_seq(), 2);
    session
}

#[test]
fn gap_fill_missing_new_seq_no_is_rejected_for_both_roles() {
    for role in [Role::Acceptor, Role::Initiator] {
        let mut session = logged_on(role);
        let mut gap_fill = message("4", 2);
        gap_fill.body.set(Field::string(123, "Y"));

        let actions = session.handle(Event::Received(gap_fill));
        assert_eq!(
            session.next_in_seq(),
            2,
            "{role:?} must not consume a malformed gap-fill"
        );
        let reject = actions
            .iter()
            .find_map(|action| match action {
                Action::Send(message) if message.msg_type() == Some("3") => Some(message),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{role:?} must reject the missing NewSeqNo: {actions:?}"));
        assert_eq!(
            reject.body.get(373).and_then(|field| field.as_int().ok()),
            Some(1),
            "SessionRejectReason must be RequiredTagMissing"
        );
        assert_eq!(
            reject.body.get(371).and_then(|field| field.as_int().ok()),
            Some(36),
            "RefTagID must identify NewSeqNo"
        );
    }
}
