//! T060 (US8) — StartTime/EndTime/Weekdays/TimeZone/StartDay/EndDay/NonStopSession map from a
//! settings file into a runnable `Schedule` (FR-018/FR-E1).

use time::macros::datetime;
use truefix_config::SessionSettings;

fn resolved_schedule(cfg: &str) -> Option<truefix_session::Schedule> {
    SessionSettings::parse(cfg)
        .unwrap()
        .resolve()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .session
        .schedule
}

fn base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

#[test]
fn non_stop_session_wins_over_other_schedule_keys() {
    let s = resolved_schedule(&base(
        "NonStopSession=Y\nStartTime=09:00:00\nEndTime=17:00:00\n",
    ))
    .unwrap();
    assert!(s.non_stop);
}

#[test]
fn daily_window_and_weekdays_and_timezone_are_mapped() {
    let s = resolved_schedule(&base(
        "StartTime=09:00:00\nEndTime=17:00:00\nWeekdays=Mon,Tue,Wed,Thu,Fri\nTimeZone=+05:00\n",
    ))
    .unwrap();
    assert!(!s.non_stop);
    // 05:00 UTC + 5h offset = 10:00 local -> inside 09:00-17:00 on a Monday.
    assert!(s.is_in_session(datetime!(2026-06-29 05:00 UTC)));
    // Saturday is excluded by Weekdays even during the daily window.
    assert!(!s.is_in_session(datetime!(2026-07-04 10:00 UTC)));
}

#[test]
fn weekly_start_day_end_day_is_mapped() {
    let s = resolved_schedule(&base(
        "StartDay=Monday\nEndDay=Friday\nStartTime=09:00:00\nEndTime=17:00:00\n",
    ))
    .unwrap();
    assert!(s.is_in_session(datetime!(2026-06-30 10:00 UTC))); // Tuesday, inside
    assert!(!s.is_in_session(datetime!(2026-07-04 10:00 UTC))); // Saturday, outside
}

#[test]
fn no_schedule_keys_means_always_active() {
    assert!(resolved_schedule(&base("")).is_none());
}

#[test]
fn invalid_timezone_is_a_typed_error() {
    // T082 (US8, feature 006): GAP-10 added IANA-name recognition, so this must now be something
    // that's neither a valid numeric offset NOR a real IANA zone name (America/New_York, used
    // here before GAP-10, is now a *valid* value -- see `named_iana_timezone_is_mapped` below).
    let cfg = base("StartTime=09:00:00\nEndTime=17:00:00\nTimeZone=Not/A_Real_Zone\n");
    let err = SessionSettings::parse(&cfg).unwrap().resolve().unwrap_err();
    match err {
        truefix_config::ConfigError::InvalidValue { key, .. } => assert_eq!(key, "TimeZone"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

// --- T082 (US8, feature 006): IANA zone names for `TimeZone` (GAP-10) ---

#[test]
fn named_iana_timezone_is_mapped_and_dst_aware() {
    let s = resolved_schedule(&base(
        "StartTime=09:00:00\nEndTime=17:00:00\nTimeZone=America/New_York\n",
    ))
    .unwrap();
    // Winter (EST, UTC-5): 09:00 local == 14:00 UTC.
    assert!(s.is_in_session(datetime!(2026-01-15 14:00 UTC)));
    // Summer (EDT, UTC-4): 08:59 local == 12:59 UTC, still before the 09:00 open -- proves the
    // offset shifted with the date (in winter this same UTC instant would already be in-window),
    // which a fixed numeric `TimeZone=` offset could never do.
    assert!(!s.is_in_session(datetime!(2026-07-15 12:59 UTC)));
    assert!(s.is_in_session(datetime!(2026-07-15 13:00 UTC)));
}

#[test]
fn named_iana_timezone_takes_precedence_over_a_stale_fixed_offset_field() {
    // A named zone entirely replaces the fixed-offset representation (Schedule::named_time_zone
    // takes precedence over Schedule::utc_offset_seconds, which stays at its default 0/UTC).
    let s = resolved_schedule(&base(
        "StartTime=09:00:00\nEndTime=17:00:00\nTimeZone=Europe/London\n",
    ))
    .unwrap();
    assert!(s.named_time_zone.is_some());
    assert_eq!(s.utc_offset_seconds, 0);
}
