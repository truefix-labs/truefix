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
    let cfg = base("StartTime=09:00:00\nEndTime=17:00:00\nTimeZone=America/New_York\n");
    let err = SessionSettings::parse(&cfg).unwrap().resolve().unwrap_err();
    match err {
        truefix_config::ConfigError::InvalidValue { key, .. } => assert_eq!(key, "TimeZone"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}
