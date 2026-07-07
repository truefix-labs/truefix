//! Audit 006 application parity coverage for NEW-105/NEW-106.

use truefix_core::{Field, Message};
use truefix_session::{Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn msg(msg_type: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

fn logon(seq: i64) -> Message {
    let mut m = msg("A", seq);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn new_order_with_poss_resend(seq: i64) -> Message {
    let mut m = msg("D", seq);
    m.header.set(Field::string(97, "Y"));
    m.body.set(Field::string(11, "ORD-POSS-RESEND"));
    m.body.set(Field::string(21, "1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::string(54, "1"));
    m.body.set(Field::string(60, "20240101-00:00:00"));
    m.body.set(Field::string(40, "2"));
    m
}

#[test]
fn new_106_poss_resend_application_message_is_not_pre_filtered_by_session_layer() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);

    let inbound = new_order_with_poss_resend(2);
    assert!(
        !s.would_reject_before_processing(&inbound),
        "PossResend(97) must remain visible to application callbacks"
    );

    let actions = s.handle(Event::Received(inbound));
    assert!(actions.is_empty());
    assert_eq!(s.next_in_seq(), 3);
}
