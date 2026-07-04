//! T064 — session scheduling (daily window, weekdays, NonStop).
//! T056 (US8) — weekly StartDay/EndDay windows across day boundaries (FR-018/FR-E1).
//! T076 (US8) — recurring daily sequence reset (`ResetSeqTime`/`EnableResetSeqTime`, GAP-11).

use time::macros::{date, datetime, time};
use time::{Time, Weekday};
use truefix_session::{Schedule, decide_recurring_reset};

#[test]
fn non_stop_is_always_in_session() {
    let s = Schedule::non_stop();
    assert!(s.is_in_session(datetime!(2024-01-01 03:00:00 UTC)));
    assert!(s.is_in_session(datetime!(2024-06-15 23:59:59 UTC)));
}

#[test]
fn daily_window_includes_and_excludes() {
    // 09:00:00 .. 17:00:00 UTC
    let s = Schedule::daily(
        Time::from_hms(9, 0, 0).unwrap(),
        Time::from_hms(17, 0, 0).unwrap(),
    );
    assert!(s.is_in_session(datetime!(2024-01-01 12:00:00 UTC)));
    assert!(!s.is_in_session(datetime!(2024-01-01 08:59:59 UTC)));
    assert!(!s.is_in_session(datetime!(2024-01-01 17:00:00 UTC)));
}

#[test]
fn window_wrapping_midnight() {
    // 22:00 .. 06:00 (overnight)
    let s = Schedule::daily(
        Time::from_hms(22, 0, 0).unwrap(),
        Time::from_hms(6, 0, 0).unwrap(),
    );
    assert!(s.is_in_session(datetime!(2024-01-01 23:00:00 UTC)));
    assert!(s.is_in_session(datetime!(2024-01-01 02:00:00 UTC)));
    assert!(!s.is_in_session(datetime!(2024-01-01 12:00:00 UTC)));
}

#[test]
fn utc_offset_shifts_local_time() {
    // 09:00..17:00 local, offset +5h. 05:00 UTC == 10:00 local => in session.
    let s = Schedule::daily(
        Time::from_hms(9, 0, 0).unwrap(),
        Time::from_hms(17, 0, 0).unwrap(),
    )
    .with_utc_offset_seconds(5 * 3600);
    assert!(s.is_in_session(datetime!(2024-01-01 05:00:00 UTC)));
    assert!(!s.is_in_session(datetime!(2024-01-01 03:00:00 UTC))); // 08:00 local
}

#[test]
fn weekday_filter_excludes_weekend() {
    // 2024-01-06 is a Saturday, 2024-01-08 a Monday.
    let s = Schedule::daily(
        Time::from_hms(0, 0, 0).unwrap(),
        Time::from_hms(23, 59, 59).unwrap(),
    )
    .with_weekdays(vec![
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
    ]);
    assert!(!s.is_in_session(datetime!(2024-01-06 12:00:00 UTC))); // Saturday
    assert!(s.is_in_session(datetime!(2024-01-08 12:00:00 UTC))); // Monday
}

#[test]
fn weekly_window_within_same_week_across_integration_boundary() {
    // Open Monday 09:00, close Friday 17:00 (T056/US8).
    let s = Schedule::weekly(Weekday::Monday, time!(9:00), Weekday::Friday, time!(17:00));
    assert!(s.is_in_session(datetime!(2026-06-30 10:00 UTC))); // Tuesday, inside
    assert!(!s.is_in_session(datetime!(2026-07-04 10:00 UTC))); // Saturday, outside
}

#[test]
fn weekly_window_wrapping_the_week_boundary_across_integration() {
    // Open Friday 18:00, close Monday 08:00 — genuinely wraps in the Sunday-indexed week.
    let s = Schedule::weekly(Weekday::Friday, time!(18:00), Weekday::Monday, time!(8:00));
    assert!(s.is_in_session(datetime!(2026-06-27 10:00 UTC))); // Saturday, inside
    assert!(!s.is_in_session(datetime!(2026-07-01 12:00 UTC))); // Wednesday, outside
}

#[test]
fn recurring_reset_time_fires_once_per_day_regardless_of_window_state() {
    // T076 (US8): a non-stop session with a `ResetSeqTime` of 00:00 still gets a recurring daily
    // reset, even though non_stop schedules never produce Enter/Exit boundary transitions.
    let s = Schedule::non_stop().with_reset_seq_time(Time::from_hms(0, 0, 0).unwrap());
    assert_eq!(
        decide_recurring_reset(
            &s,
            Some(date!(2026 - 06 - 29)),
            datetime!(2026-06-30 00:05 UTC)
        ),
        Some(date!(2026 - 06 - 30))
    );
    // Same calendar day, already fired: no second reset.
    assert_eq!(
        decide_recurring_reset(
            &s,
            Some(date!(2026 - 06 - 30)),
            datetime!(2026-06-30 12:00 UTC)
        ),
        None
    );
}

#[test]
fn recurring_reset_time_unset_never_fires() {
    let s = Schedule::non_stop();
    assert_eq!(
        decide_recurring_reset(&s, None, datetime!(2026-06-30 12:00 UTC)),
        None
    );
}
