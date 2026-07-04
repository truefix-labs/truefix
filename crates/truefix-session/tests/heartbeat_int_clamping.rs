//! T097/T098 (US3, feature 007): an adopted peer `HeartBtInt` value exceeding TrueFix's
//! representable range (`u32`) is clamped to `u32::MAX`, not silently truncated via `as u32`
//! (BUG-67, FR-038) — previously `hbi as u32` (where `hbi: i64`) would wrap an out-of-range value
//! to an arbitrary, unrelated small number instead of the sane "very long heartbeat interval" the
//! peer's oversized value actually implies.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig};

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn logon_with_hbi(hbi: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, hbi));
    m
}

fn sent_heartbeat(actions: &[Action]) -> bool {
    actions
        .iter()
        .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("0")))
}

/// No direct accessor exposes the adopted `heartbeat_interval` (`Session`'s config is private), so
/// this observes the effect indirectly through `on_tick`'s own cadence (`ticks_since_send >= hb`
/// triggers an outbound Heartbeat): `hbi = u32::MAX + 5` truncates (buggy `as u32`) to exactly `4`
/// (the low 32 bits of `0x1_0000_0004`), a small, easily-observable threshold within a handful of
/// ticks -- versus `u32::MAX` itself when correctly clamped, which no realistic tick count in a
/// test could ever reach.
#[test]
fn a_heart_bt_int_beyond_u32_max_is_clamped_not_truncated() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    let hbi: i64 = i64::from(u32::MAX) + 5; // `as u32` would wrap this to exactly 4
    s.handle(Event::Received(logon_with_hbi(hbi)));

    // Ticks 1..=4: with correct clamping (hb = u32::MAX), never send a heartbeat this soon.
    for _ in 0..4 {
        let actions = s.handle(Event::Tick);
        assert!(
            !sent_heartbeat(&actions),
            "an out-of-range HeartBtInt must clamp to u32::MAX (never firing this soon), not \
             wrap/truncate via `as u32` to a small unrelated threshold"
        );
    }
}

#[test]
fn a_heart_bt_int_within_u32_range_is_adopted_unchanged() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon_with_hbi(3)));

    // With hb = 3 (unchanged, no clamping needed), a heartbeat is sent by the 3rd idle tick.
    let mut sent = false;
    for _ in 0..3 {
        if sent_heartbeat(&s.handle(Event::Tick)) {
            sent = true;
            break;
        }
    }
    assert!(
        sent,
        "an in-range HeartBtInt must still be adopted exactly, not altered"
    );
}
