//! T087/T088 (US3, feature 007): `HeartBtIntTimeoutMultiplier` (`heartbeat_timeout_multiplier`)
//! preserves fractional precision, and the timeout arithmetic that uses it cannot overflow for
//! realistic (or even pathological) heartbeat intervals (BUG-36, FR-033). Previously the field was
//! `u32` (QFJ's own `HeartBtIntTimeoutMultiplier` is a `double`, default `1.4`), so a fractional
//! `.cfg` value truncated silently, and `hb * multiplier.max(1) + 2` was plain `u32` arithmetic
//! that could panic (debug) or silently wrap (release) for large values.

use truefix_core::{Field, Message};
use truefix_session::{Event, Role, Session, SessionConfig};

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 1;
    c.check_latency = false;
    c
}

fn logon(seq: i64, hb: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, hb));
    m
}

fn has_disconnect(actions: &[truefix_session::Action]) -> bool {
    actions
        .iter()
        .any(|a| matches!(a, truefix_session::Action::Disconnect))
}

/// A fractional multiplier (1.4) must genuinely change the disconnect threshold, not truncate to
/// 1 (which would disconnect at tick 3: `1*1+2`) or round up to 2 (tick 4: `1*2+2`). With
/// `heartbeat_interval=1` and `multiplier=1.4`, the threshold is `1*1.4+2=3.4`: tick 3
/// (`ticks_since_recv==3`) must NOT disconnect (3 < 3.4) but tick 4 (`ticks_since_recv==4`) must
/// (4 >= 3.4) -- a boundary only a genuinely-fractional multiplier produces.
#[test]
fn a_fractional_multiplier_moves_the_disconnect_threshold_precisely() {
    let mut cfg = acc_cfg();
    cfg.heartbeat_timeout_multiplier = 1.4;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 1)));

    // Ticks 1..=3: must not disconnect yet (threshold is 3.4).
    for _ in 0..3 {
        let actions = s.handle(Event::Tick);
        assert!(
            !has_disconnect(&actions),
            "must not disconnect before ticks_since_recv reaches the 3.4 threshold"
        );
    }
    // Tick 4: ticks_since_recv == 4 >= 3.4 -- must disconnect now.
    let actions = s.handle(Event::Tick);
    assert!(
        has_disconnect(&actions),
        "a multiplier of 1.4 (not truncated to 1, nor rounded to 2) must disconnect exactly once \
         ticks_since_recv reaches 3.4, i.e. on the 4th idle tick"
    );
}

/// A multiplier at exactly its old truncated value (1) must still work identically to before
/// (regression guard for the common/default-adjacent case).
#[test]
fn an_integer_valued_multiplier_still_behaves_as_before() {
    let mut cfg = acc_cfg();
    cfg.heartbeat_timeout_multiplier = 1.0;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 1)));

    for _ in 0..2 {
        let actions = s.handle(Event::Tick);
        assert!(!has_disconnect(&actions));
    }
    let actions = s.handle(Event::Tick);
    assert!(
        has_disconnect(&actions),
        "multiplier=1.0 should disconnect at threshold 1*1+2=3, on the 3rd idle tick"
    );
}

/// A pathologically large heartbeat interval and multiplier must not panic (the pre-fix `u32`
/// arithmetic would overflow-panic in a debug build for values like these).
#[test]
fn a_pathologically_large_heartbeat_and_multiplier_does_not_panic() {
    let mut cfg = acc_cfg();
    cfg.heartbeat_interval = 1_000_000;
    cfg.heartbeat_timeout_multiplier = 1_000_000.0;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 1_000_000)));

    // Must not panic, and obviously must not disconnect after just one idle tick against such a
    // huge threshold.
    let actions = s.handle(Event::Tick);
    assert!(
        !has_disconnect(&actions),
        "one idle tick against a threshold of ~1e12 must not disconnect"
    );
}
