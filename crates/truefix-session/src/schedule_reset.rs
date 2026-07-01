//! Scheduled-reset boundary decisions (FR-018/FR-E3): a pure, sans-IO function that tells the
//! transport what to do when a session's `Schedule` transitions in/out of its active window —
//! disconnect → reset sequence numbers → clear the store on exit, and reconnect/reset on entry —
//! skipping entirely when the session is non-stop.

use time::OffsetDateTime;

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

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{datetime, time};
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
