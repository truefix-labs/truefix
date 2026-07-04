//! Session scheduling: when a session should be active.
//!
//! Supports 24×7 (NonStopSession), daily windows (StartTime/EndTime), a weekday filter, a weekly
//! window (StartDay/EndDay, spanning potentially several days — e.g. "open Sunday 18:00, close
//! Friday 17:00"), and either a fixed UTC offset or a named IANA time zone (`TimeZone`, GAP-10) —
//! the latter resolves to a DST-aware offset that varies by date, unlike the fixed form.

use time::{OffsetDateTime, Time, UtcOffset, Weekday};
use time_tz::{Offset, TimeZone as _};

/// A session activity schedule.
#[derive(Debug, Clone, Default)]
pub struct Schedule {
    /// Always in session (24×7).
    pub non_stop: bool,
    /// Daily window start (local time), if any.
    pub start_time: Option<Time>,
    /// Daily window end (local time), if any.
    pub end_time: Option<Time>,
    /// Allowed weekdays; `None` means every day.
    pub weekdays: Option<Vec<Weekday>>,
    /// Local time-zone offset from UTC, in whole seconds. Ignored when [`named_time_zone`] is
    /// set (GAP-10).
    ///
    /// [`named_time_zone`]: Self::named_time_zone
    pub utc_offset_seconds: i32,
    /// A named IANA time zone (`TimeZone=America/New_York`, GAP-10), taking precedence over
    /// [`utc_offset_seconds`] when set — its offset is resolved dynamically per date (DST-aware),
    /// unlike the fixed-offset form.
    ///
    /// [`utc_offset_seconds`]: Self::utc_offset_seconds
    pub named_time_zone: Option<&'static time_tz::Tz>,
    /// Weekly window start day (StartDay), paired with `start_time`. When set together with
    /// `end_day`, the window spans from this day/time to `end_day`/`end_time`, potentially
    /// crossing several days (e.g. Sunday evening to Friday evening).
    pub start_day: Option<Weekday>,
    /// Weekly window end day (EndDay), paired with `end_time`.
    pub end_day: Option<Weekday>,
    /// `ResetSeqTime`/`EnableResetSeqTime` (GAP-11): a daily local time-of-day at which sequence
    /// numbers are reset once per day, independent of any Enter/Exit window transition (and
    /// independent of `non_stop`, since a 24x7 session still wants a recurring daily reset).
    pub reset_seq_time: Option<Time>,
}

impl Schedule {
    /// A 24×7 schedule.
    pub fn non_stop() -> Self {
        Self {
            non_stop: true,
            ..Self::default()
        }
    }

    /// A daily window `[start, end)` in local time (wraps past midnight if `end < start`).
    pub fn daily(start: Time, end: Time) -> Self {
        Self {
            start_time: Some(start),
            end_time: Some(end),
            ..Self::default()
        }
    }

    /// A weekly window from `start_day`/`start_time` to `end_day`/`end_time` (StartDay/EndDay),
    /// potentially spanning several days (e.g. Sunday 18:00 to Friday 17:00).
    pub fn weekly(start_day: Weekday, start_time: Time, end_day: Weekday, end_time: Time) -> Self {
        Self {
            start_time: Some(start_time),
            end_time: Some(end_time),
            start_day: Some(start_day),
            end_day: Some(end_day),
            ..Self::default()
        }
    }

    /// Restrict to the given weekdays.
    pub fn with_weekdays(mut self, days: Vec<Weekday>) -> Self {
        self.weekdays = Some(days);
        self
    }

    /// Set the local UTC offset in whole seconds.
    pub fn with_utc_offset_seconds(mut self, seconds: i32) -> Self {
        self.utc_offset_seconds = seconds;
        self
    }

    /// Set a named IANA time zone (`TimeZone=America/New_York`, GAP-10), taking precedence over
    /// [`Self::with_utc_offset_seconds`].
    pub fn with_named_time_zone(mut self, tz: &'static time_tz::Tz) -> Self {
        self.named_time_zone = Some(tz);
        self
    }

    /// Set the recurring daily sequence-reset time (`ResetSeqTime`, GAP-11).
    pub fn with_reset_seq_time(mut self, t: Time) -> Self {
        self.reset_seq_time = Some(t);
        self
    }

    /// The local UTC offset to apply at `now_utc` — the named zone's DST-aware offset for that
    /// instant when [`Self::named_time_zone`] is set (GAP-10), else the fixed
    /// [`Self::utc_offset_seconds`].
    pub(crate) fn effective_offset(&self, now_utc: OffsetDateTime) -> UtcOffset {
        if let Some(tz) = self.named_time_zone {
            return tz.get_offset_utc(&now_utc).to_utc();
        }
        UtcOffset::from_whole_seconds(self.utc_offset_seconds).unwrap_or(UtcOffset::UTC)
    }

    /// Whether `now_utc` falls within the schedule.
    pub fn is_in_session(&self, now_utc: OffsetDateTime) -> bool {
        if self.non_stop {
            return true;
        }
        let offset = self.effective_offset(now_utc);
        let local = now_utc.to_offset(offset);

        if let (Some(start_day), Some(end_day)) = (self.start_day, self.end_day) {
            let (Some(start_time), Some(end_time)) = (self.start_time, self.end_time) else {
                return true;
            };
            return in_weekly_window(local, start_day, start_time, end_day, end_time);
        }

        if let Some(days) = &self.weekdays
            && !days.contains(&local.weekday())
        {
            return false;
        }
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => in_window(local.time(), start, end),
            _ => true,
        }
    }
}

fn in_window(t: Time, start: Time, end: Time) -> bool {
    if start <= end {
        t >= start && t < end
    } else {
        // window wraps past midnight
        t >= start || t < end
    }
}

/// Minutes since Sunday 00:00 (0..10_080), for weekly-window arithmetic.
fn minutes_since_week_start(weekday: Weekday, t: Time) -> i64 {
    let day_index = weekday.number_days_from_sunday() as i64; // 0=Sunday .. 6=Saturday
    day_index * 24 * 60 + i64::from(t.hour()) * 60 + i64::from(t.minute())
}

/// Whether `local` falls within the weekly window `[start_day/start_time, end_day/end_time)`,
/// which may wrap across the week boundary (e.g. start=Friday, end=Sunday means the window
/// spans Fri→Sat→Sun, and also wraps if `local` is early Monday relative to a window that started
/// the *previous* week — handled by the circular (mod one week) comparison below).
fn in_weekly_window(
    local: OffsetDateTime,
    start_day: Weekday,
    start_time: Time,
    end_day: Weekday,
    end_time: Time,
) -> bool {
    let now = minutes_since_week_start(local.weekday(), local.time());
    let start = minutes_since_week_start(start_day, start_time);
    let end = minutes_since_week_start(end_day, end_time);

    if start <= end {
        now >= start && now < end
    } else {
        // The window wraps across the week boundary (e.g. starts Friday, ends Sunday).
        now >= start || now < end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{datetime, time};

    #[test]
    fn named_time_zone_is_dst_aware_unlike_a_fixed_offset() {
        // T082 (US8, feature 006): GAP-10. America/New_York is EST (UTC-5) in January and EDT
        // (UTC-4) in July -- a fixed offset could only ever get one of these two dates right.
        let tz = time_tz::timezones::db::america::NEW_YORK;
        let s = Schedule::daily(time!(9:00), time!(17:00)).with_named_time_zone(tz);

        // Winter (EST, UTC-5): 09:00 local == 14:00 UTC.
        assert!(s.is_in_session(datetime!(2026-01-15 14:00 UTC)));
        assert!(!s.is_in_session(datetime!(2026-01-15 13:00 UTC))); // 08:00 local, before open

        // Summer (EDT, UTC-4): 09:00 local == 13:00 UTC -- one hour earlier in UTC than winter,
        // proving the offset actually shifts with the date rather than staying fixed.
        assert!(s.is_in_session(datetime!(2026-07-15 13:00 UTC)));
        assert!(!s.is_in_session(datetime!(2026-07-15 12:00 UTC))); // 08:00 local, before open
    }

    #[test]
    fn weekly_window_within_same_week() {
        // Open Monday 09:00, close Friday 17:00.
        let s = Schedule::weekly(Weekday::Monday, time!(9:00), Weekday::Friday, time!(17:00));
        assert!(s.is_in_session(datetime!(2026-06-30 10:00 UTC))); // Tuesday, mid-window
        assert!(!s.is_in_session(datetime!(2026-07-04 10:00 UTC))); // Saturday, outside
        assert!(!s.is_in_session(datetime!(2026-06-29 08:00 UTC))); // Monday, before open
    }

    #[test]
    fn weekly_window_wraps_the_week_boundary() {
        // A window from Friday 18:00 to Monday 08:00: since Friday(day 5) sorts *after*
        // Monday(day 1) in the Sunday-indexed week, this genuinely wraps past the week
        // boundary in the underlying minutes-since-Sunday arithmetic.
        let s = Schedule::weekly(Weekday::Friday, time!(18:00), Weekday::Monday, time!(8:00));
        assert!(s.is_in_session(datetime!(2026-06-26 19:00 UTC))); // Friday evening, just after open
        assert!(s.is_in_session(datetime!(2026-06-27 10:00 UTC))); // Saturday, inside
        assert!(s.is_in_session(datetime!(2026-06-28 20:00 UTC))); // Sunday evening, inside
        assert!(!s.is_in_session(datetime!(2026-06-26 17:00 UTC))); // Friday, before open
        assert!(!s.is_in_session(datetime!(2026-06-29 10:00 UTC))); // Monday, after close
        assert!(!s.is_in_session(datetime!(2026-07-01 12:00 UTC))); // Wednesday, outside
    }
}
