//! Scheduled-reset boundary decisions (FR-018/FR-E3): a pure, sans-IO function that tells the
//! transport what to do when a session's `Schedule` transitions in/out of its active window —
//! disconnect → reset sequence numbers → clear the store on exit, and reconnect/reset on entry —
//! skipping entirely when the session is non-stop.

use time::{Date, OffsetDateTime};

use crate::schedule::Schedule;

/// What the transport should do in response to a schedule-boundary transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleAction {
    /// No boundary was crossed; no action needed.
    None,
    /// The session just entered its active window: reset sequence numbers/store, then connect
    /// (initiator) or otherwise resume accepting logons.
    Enter,
    /// The session just left its active window: disconnect, reset sequence numbers, and clear
    /// the store.
    Exit,
}

/// Decide the action for a transition from `was_in_session` to the schedule's state at `now_utc`.
/// `NonStopSession` (`schedule.non_stop`) never produces `Enter`/`Exit` (FR-018).
///
/// Returns `(action, new_was_in_session)` — the caller stores `new_was_in_session` and passes it
/// back in on the next check.
pub fn decide(
    schedule: &Schedule,
    was_in_session: bool,
    now_utc: OffsetDateTime,
) -> ScheduleAction {
    if schedule.non_stop {
        return ScheduleAction::None;
    }
    let in_session = schedule.is_in_session(now_utc);
    match (was_in_session, in_session) {
        (false, true) => ScheduleAction::Enter,
        (true, false) => ScheduleAction::Exit,
        _ => ScheduleAction::None,
    }
}

/// Decide whether a recurring daily sequence reset (`ResetSeqTime`/`EnableResetSeqTime`, GAP-11)
/// should fire now. This is independent of the Enter/Exit boundary decision above — it applies
/// even to a `non_stop` (24x7) schedule, and even mid-window for a windowed schedule.
///
/// `last_reset_date` is the date (in the schedule's local offset) the reset last fired on, or
/// `None` if it has never fired. Returns `Some(new_date)` when a reset should fire now — the
/// caller performs the reset and stores `new_date` as the new `last_reset_date` — or `None` when
/// no reset should fire (the caller keeps its `last_reset_date` unchanged).
///
/// Fires at most once per local calendar day: once `now_utc`'s local time-of-day has reached
/// `reset_seq_time` and today's date doesn't match `last_reset_date`, it fires and remembers
/// today's date, so subsequent calls later the same day are a no-op until the date rolls over.
pub fn decide_recurring_reset(
    schedule: &Schedule,
    last_reset_date: Option<Date>,
    now_utc: OffsetDateTime,
) -> Option<Date> {
    let reset_time = schedule.reset_seq_time?;
    let local = now_utc.to_offset(schedule.effective_offset(now_utc));
    if local.time() >= reset_time && last_reset_date != Some(local.date()) {
        Some(local.date())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime, time};
    use time::Weekday;

    fn window() -> Schedule {
        Schedule::weekly(Weekday::Monday, time!(9:00), Weekday::Friday, time!(17:00))
    }

    #[test]
    fn entering_the_window_yields_enter() {
        let s = window();
        let now = datetime!(2026-06-29 09:30 UTC); // Monday, inside
        assert_eq!(decide(&s, false, now), ScheduleAction::Enter);
    }

    #[test]
    fn leaving_the_window_yields_exit() {
        let s = window();
        let now = datetime!(2026-07-04 10:00 UTC); // Saturday, outside
        assert_eq!(decide(&s, true, now), ScheduleAction::Exit);
    }

    #[test]
    fn steady_state_yields_none() {
        let s = window();
        let inside = datetime!(2026-06-30 10:00 UTC);
        let outside = datetime!(2026-07-04 10:00 UTC);
        assert_eq!(decide(&s, true, inside), ScheduleAction::None);
        assert_eq!(decide(&s, false, outside), ScheduleAction::None);
    }

    #[test]
    fn recurring_reset_fires_once_after_the_reset_time_is_reached() {
        let s = Schedule::non_stop().with_reset_seq_time(time!(9:00));
        let before = datetime!(2026-06-30 08:59 UTC);
        let after = datetime!(2026-06-30 09:30 UTC);
        // Before 09:00 on 2026-06-30: no fire yet, regardless of when it last fired.
        assert_eq!(
            decide_recurring_reset(&s, Some(date!(2026 - 06 - 29)), before),
            None
        );
        // After 09:00 on 2026-06-30, having last fired on 06-29 (or never): fires, remembering
        // 06-30.
        assert_eq!(
            decide_recurring_reset(&s, Some(date!(2026 - 06 - 29)), after),
            Some(date!(2026 - 06 - 30))
        );
        assert_eq!(
            decide_recurring_reset(&s, None, after),
            Some(date!(2026 - 06 - 30))
        );
    }

    #[test]
    fn recurring_reset_does_not_refire_the_same_day() {
        let s = Schedule::non_stop().with_reset_seq_time(time!(9:00));
        let now = datetime!(2026-06-30 10:00 UTC);
        assert_eq!(
            decide_recurring_reset(&s, Some(date!(2026 - 06 - 30)), now),
            None
        );
    }

    #[test]
    fn recurring_reset_disabled_when_reset_seq_time_unset() {
        let s = window(); // no reset_seq_time set
        assert_eq!(
            decide_recurring_reset(&s, None, datetime!(2026-06-30 10:00 UTC)),
            None
        );
    }

    #[test]
    fn recurring_reset_is_independent_of_enter_exit_window() {
        // A windowed (non-non-stop) schedule still recurs its daily reset even mid-window.
        let s = window().with_reset_seq_time(time!(9:00));
        let now = datetime!(2026-06-30 09:30 UTC); // Tuesday, inside the Mon-Fri window
        assert_eq!(
            decide_recurring_reset(&s, Some(date!(2026 - 06 - 29)), now),
            Some(date!(2026 - 06 - 30))
        );
    }

    #[test]
    fn non_stop_session_never_resets() {
        let s = Schedule::non_stop();
        // Even a nominal "transition" produces no action for a non-stop session.
        assert_eq!(
            decide(&s, false, datetime!(2026-06-29 09:30 UTC)),
            ScheduleAction::None
        );
        assert_eq!(
            decide(&s, true, datetime!(2026-07-04 10:00 UTC)),
            ScheduleAction::None
        );
    }
}
