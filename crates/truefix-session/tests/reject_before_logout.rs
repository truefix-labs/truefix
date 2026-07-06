//! T155 (NEW-87) — a non-Logon message that triggers a latency, CompID-mismatch, or
//! PossDup-falsification disconnect sends a session-level `Reject` *before* the `Logout`,
//! matching QuickFIX/J's `doBadTime`/`doBadCompID` two-message shape, rather than a `Logout` alone.

use time::OffsetDateTime;
use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig};

fn format_timestamp(t: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}-{:02}:{:02}:{:02}",
        t.year(),
        u8::from(t.month()),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

fn now_timestamp() -> String {
    format_timestamp(OffsetDateTime::now_utc())
}

fn acceptor_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "M", "Y", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = true;
    c.max_latency = 120;
    c
}

fn logon() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "Y"));
    m.header.set(Field::string(56, "M"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(52, &now_timestamp()));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn heartbeat(sender: &str, target: &str, sending_time: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::string(49, sender));
    m.header.set(Field::string(56, target));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(52, sending_time));
    m
}

fn actions_of(session: &mut Session, msg: Message) -> Vec<Action> {
    session.handle(Event::Received(msg))
}

fn find_msg_type<'a>(actions: &'a [Action], msg_type: &str) -> Option<&'a Message> {
    actions.iter().find_map(|a| match a {
        Action::Send(m) if m.msg_type() == Some(msg_type) => Some(m),
        _ => None,
    })
}

#[test]
fn latency_failure_sends_reject_then_logout() {
    let mut session = Session::new(acceptor_cfg());
    session.handle(Event::Connected);
    actions_of(&mut session, logon());

    let stale_time = "20000101-00:00:00";
    let actions = actions_of(&mut session, heartbeat("Y", "M", stale_time, 2));

    let reject = find_msg_type(&actions, "3").expect("a session Reject must be sent");
    assert_eq!(
        reject.body.get(373).and_then(|f| f.as_int().ok()),
        Some(10),
        "SessionRejectReason(373)=10 (SendingTime accuracy problem)"
    );
    assert_eq!(reject.body.get(371).and_then(|f| f.as_int().ok()), Some(52));

    let logout = find_msg_type(&actions, "5").expect("a Logout must still be sent");
    let reject_seq = reject.header.get(34).and_then(|f| f.as_int().ok()).unwrap();
    let logout_seq = logout.header.get(34).and_then(|f| f.as_int().ok()).unwrap();
    assert!(
        reject_seq < logout_seq,
        "the Reject must be sent (and sequenced) before the Logout"
    );
}

#[test]
fn comp_id_mismatch_sends_reject_then_logout() {
    let mut session = Session::new(acceptor_cfg());
    session.handle(Event::Connected);
    actions_of(&mut session, logon());

    // Wrong SenderCompID ("Z" instead of "Y").
    let actions = actions_of(&mut session, heartbeat("Z", "M", &now_timestamp(), 2));

    let reject = find_msg_type(&actions, "3").expect("a session Reject must be sent");
    assert_eq!(
        reject.body.get(373).and_then(|f| f.as_int().ok()),
        Some(9),
        "SessionRejectReason(373)=9 (CompID problem)"
    );
    assert_eq!(reject.body.get(371).and_then(|f| f.as_int().ok()), Some(49));

    let logout = find_msg_type(&actions, "5").expect("a Logout must still be sent");
    let reject_seq = reject.header.get(34).and_then(|f| f.as_int().ok()).unwrap();
    let logout_seq = logout.header.get(34).and_then(|f| f.as_int().ok()).unwrap();
    assert!(reject_seq < logout_seq);
}

#[test]
fn incorrect_begin_string_stays_logout_only() {
    // Control: NEW-87/FR-073 doesn't cover BeginString mismatch -- must remain unchanged.
    let mut session = Session::new(acceptor_cfg());
    session.handle(Event::Connected);
    actions_of(&mut session, logon());

    let mut bad = heartbeat("Y", "M", &now_timestamp(), 2);
    bad.header.set(Field::string(8, "FIX.4.2"));
    let actions = actions_of(&mut session, bad);

    assert!(
        find_msg_type(&actions, "3").is_none(),
        "an incorrect BeginString must not send a session Reject"
    );
    assert!(
        find_msg_type(&actions, "5").is_some(),
        "a Logout must still be sent"
    );
}

#[test]
fn a_logon_specific_bad_sending_time_stays_logout_only() {
    // Control: NEW-87/FR-073's own source note distinguishes the Logon-specific bad-time/bad-CompID
    // case (QFJ's `logoutWithErrorMessage`, Logout-only) from every other message type -- this fix
    // must not add a Reject in front of a Logon's own Logout.
    let mut session = Session::new(acceptor_cfg());
    session.handle(Event::Connected);

    let mut bad_logon = logon();
    bad_logon.header.set(Field::string(52, "20000101-00:00:00"));
    let actions = actions_of(&mut session, bad_logon);

    assert!(
        find_msg_type(&actions, "3").is_none(),
        "a Logon-specific bad SendingTime must not send a session Reject"
    );
    assert!(
        find_msg_type(&actions, "5").is_some(),
        "a Logout must still be sent"
    );
}

#[test]
fn poss_dup_falsification_sends_reject_then_logout() {
    let mut session = Session::new(acceptor_cfg());
    session.handle(Event::Connected);
    actions_of(&mut session, logon());

    // PossDup=Y with OrigSendingTime later than SendingTime -- a falsification signal. Both stay
    // close to "now" so the (unrelated) latency check doesn't fire first.
    let now = OffsetDateTime::now_utc();
    let mut msg = heartbeat("Y", "M", &format_timestamp(now), 2);
    msg.header.set(Field::string(43, "Y"));
    msg.header.set(Field::string(
        122,
        &format_timestamp(now + time::Duration::seconds(5)),
    ));
    let actions = actions_of(&mut session, msg);

    let reject = find_msg_type(&actions, "3").expect("a session Reject must be sent");
    assert_eq!(reject.body.get(373).and_then(|f| f.as_int().ok()), Some(5));

    let logout = find_msg_type(&actions, "5").expect("a Logout must still be sent");
    let reject_seq = reject.header.get(34).and_then(|f| f.as_int().ok()).unwrap();
    let logout_seq = logout.header.get(34).and_then(|f| f.as_int().ok()).unwrap();
    assert!(reject_seq < logout_seq);
}
