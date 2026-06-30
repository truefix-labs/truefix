//! Session scheduling: when a session should be active.
//!
//! Supports 24×7 (NonStopSession), daily windows (StartTime/EndTime), a weekday filter, and a
//! fixed UTC offset (TimeZone, simplified to a whole-second offset). Named time zones and full
//! weekly StartDay/EndDay windows can extend this later.

use time::{OffsetDateTime, Time, UtcOffset, Weekday};

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
    /// Local time-zone offset from UTC, in whole seconds.
    pub utc_offset_seconds: i32,
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

    /// Whether `now_utc` falls within the schedule.
    pub fn is_in_session(&self, now_utc: OffsetDateTime) -> bool {
        if self.non_stop {
            return true;
        }
        let offset =
            UtcOffset::from_whole_seconds(self.utc_offset_seconds).unwrap_or(UtcOffset::UTC);
        let local = now_utc.to_offset(offset);
        if let Some(days) = &self.weekdays {
            if !days.contains(&local.weekday()) {
                return false;
            }
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
