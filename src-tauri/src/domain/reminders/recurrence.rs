//! Reminders that come back.
//!
//! Repetition is computed on the wall clock, not by adding a fixed number of
//! milliseconds. "Every day at 09:00" has to stay 09:00 across the two nights a
//! year that are twenty-three and twenty-five hours long, and adding 24 hours
//! would quietly walk the reminder an hour off and leave it there.
//!
//! The stored form is an RFC 5545 `RRULE` so the column keeps its documented
//! meaning and a later build can widen the set without a migration. Only the
//! handful of rules the app can actually offer are understood; anything else is
//! refused rather than half-honoured.

use chrono::{Datelike as _, NaiveDate, NaiveDateTime, Weekday};
use chrono_tz::Tz;

use crate::domain::clock::Timestamp;
use crate::error::{AppError, AppResult, ReminderError};

use super::zones::resolve;

/// How many future occurrences are kept armed at once.
///
/// Android forgets alarms it has already delivered, and nothing wakes the core
/// when one fires, so a repeating reminder survives on the alarms armed ahead
/// of it. Four is a fortnight of "every weekday" and a month of "every week" —
/// long enough that a phone left alone keeps reminding, short enough not to
/// fill the alarm table with a year of them.
pub const WINDOW: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recurrence {
    Daily,
    /// Monday to Friday.
    Weekdays,
    Weekly,
    Monthly,
    Yearly,
}

impl Recurrence {
    /// The stored `RRULE`.
    #[must_use]
    pub const fn rule(self) -> &'static str {
        match self {
            Self::Daily => "FREQ=DAILY",
            Self::Weekdays => "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR",
            Self::Weekly => "FREQ=WEEKLY",
            Self::Monthly => "FREQ=MONTHLY",
            Self::Yearly => "FREQ=YEARLY",
        }
    }

    /// # Errors
    /// Returns [`ReminderError::InvalidRecurrence`] for a rule outside the set
    /// this build understands — including a valid RFC 5545 rule it cannot
    /// honour, which is worse to approximate than to refuse.
    pub fn parse(rule: &str) -> AppResult<Self> {
        [
            Self::Daily,
            Self::Weekdays,
            Self::Weekly,
            Self::Monthly,
            Self::Yearly,
        ]
        .into_iter()
        .find(|candidate| candidate.rule() == rule)
        .ok_or_else(|| {
            AppError::Reminder(ReminderError::InvalidRecurrence {
                reason: format!("`{rule}` is not one of the rules this build offers"),
            })
        })
    }

    /// The `step`-th wall-clock reading, counting the original as zero.
    ///
    /// Every step is measured from the reading the user chose rather than from
    /// the one before it. Walking forward one at a time would let a clamped
    /// date poison the rest of the series: "the 31st of every month" would
    /// become the 28th of every month for good the first time it passed
    /// February.
    ///
    /// It works on the reading rather than the instant, so the time of day is
    /// carried across a daylight-saving change untouched.
    #[must_use]
    pub fn nth(self, anchor: NaiveDateTime, step: usize) -> NaiveDateTime {
        let time = anchor.time();
        let date = anchor.date();
        let offset = i64::try_from(step).unwrap_or(i64::MAX);
        let moved = match self {
            Self::Daily => date + chrono::Duration::days(offset),
            Self::Weekly => date + chrono::Duration::days(offset * 7),
            Self::Monthly => add_months(date, u32::try_from(step).unwrap_or(u32::MAX)),
            Self::Yearly => add_months(
                date,
                u32::try_from(step).unwrap_or(u32::MAX).saturating_mul(12),
            ),
            Self::Weekdays => {
                let mut candidate = date;
                for _ in 0..step {
                    loop {
                        candidate = candidate.succ_opt().unwrap_or(candidate);
                        if !matches!(candidate.weekday(), Weekday::Sat | Weekday::Sun) {
                            break;
                        }
                    }
                }
                candidate
            }
        };
        moved.and_time(time)
    }
}

/// Adds whole months, keeping the day of the month where the month is long
/// enough for it.
///
/// The 31st in a thirty-day month becomes the 30th rather than spilling into
/// the next one: "the 31st of every month" is a request to be reminded at the
/// end of the month, and skipping February entirely would be a stranger reading
/// of it than landing on the last day.
fn add_months(date: NaiveDate, months: u32) -> NaiveDate {
    let zero_based = date.month0() + months;
    let year = date.year() + i32::try_from(zero_based / 12).unwrap_or(0);
    let month = zero_based % 12 + 1;

    let mut day = date.day();
    loop {
        if let Some(moved) = NaiveDate::from_ymd_opt(year, month, day) {
            return moved;
        }
        // Only ever runs for the 29th to the 31st of a short month.
        day -= 1;
        if day == 0 {
            return date;
        }
    }
}

/// The instants to arm for a reminder, starting at `first` and repeating.
///
/// Occurrences already in the past are stepped over rather than armed: after a
/// phone has been off for a week, the user wants the next "every day at 09:00",
/// not seven notifications for the mornings they missed.
///
/// A reminder that does not repeat yields its one instant, or nothing at all if
/// that instant has passed.
#[must_use]
pub fn window(
    first: NaiveDateTime,
    zone: Tz,
    recurrence: Option<Recurrence>,
    now: Timestamp,
    count: usize,
) -> Vec<Timestamp> {
    let Some(recurrence) = recurrence else {
        let instant = resolve(first, zone);
        return if instant > now {
            vec![instant]
        } else {
            Vec::new()
        };
    };

    let mut instants = Vec::with_capacity(count);
    // Bounded so a rule that somehow fails to advance cannot spin: the worst
    // real case is a daily reminder abandoned for a few years.
    for step in 0..MAX_STEPS {
        if instants.len() == count {
            break;
        }
        let instant = resolve(recurrence.nth(first, step), zone);
        if instant > now {
            instants.push(instant);
        }
    }
    instants
}

/// Ceiling on how far the search for future occurrences will walk.
const MAX_STEPS: usize = 4_000;

#[cfg(test)]
mod tests {
    use super::*;

    const ALMATY: Tz = chrono_tz::Asia::Almaty;

    fn wall(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .expect("valid date")
            .and_hms_opt(hour, minute, 0)
            .expect("valid time")
    }

    fn readings(instants: &[Timestamp], zone: Tz) -> Vec<String> {
        instants
            .iter()
            .map(|instant| {
                instant
                    .to_zoned(zone)
                    .expect("representable")
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .collect()
    }

    fn before(instant: NaiveDateTime, zone: Tz) -> Timestamp {
        Timestamp::from_millis(resolve(instant, zone).as_millis() - 1)
    }

    #[test]
    fn every_rule_survives_a_round_trip_through_its_stored_form() {
        for recurrence in [
            Recurrence::Daily,
            Recurrence::Weekdays,
            Recurrence::Weekly,
            Recurrence::Monthly,
            Recurrence::Yearly,
        ] {
            assert_eq!(
                Recurrence::parse(recurrence.rule()).expect("parses"),
                recurrence
            );
        }
    }

    #[test]
    fn a_rule_this_build_cannot_honour_is_refused_rather_than_approximated() {
        let error = Recurrence::parse("FREQ=HOURLY;INTERVAL=3").expect_err("must refuse");
        assert_eq!(error.code(), "reminder_invalid_recurrence");
    }

    #[test]
    fn daily_keeps_the_time_of_day() {
        let start = wall(2026, 8, 3, 9, 0);
        let instants = window(
            start,
            ALMATY,
            Some(Recurrence::Daily),
            before(start, ALMATY),
            3,
        );
        assert_eq!(
            readings(&instants, ALMATY),
            ["2026-08-03 09:00", "2026-08-04 09:00", "2026-08-05 09:00"]
        );
    }

    #[test]
    fn weekdays_steps_over_the_weekend() {
        // 7 August 2026 is a Friday.
        let start = wall(2026, 8, 7, 9, 0);
        let instants = window(
            start,
            ALMATY,
            Some(Recurrence::Weekdays),
            before(start, ALMATY),
            3,
        );
        assert_eq!(
            readings(&instants, ALMATY),
            ["2026-08-07 09:00", "2026-08-10 09:00", "2026-08-11 09:00"],
            "Friday is followed by Monday"
        );
    }

    #[test]
    fn the_end_of_a_long_month_lands_on_the_end_of_a_short_one() {
        let start = wall(2026, 1, 31, 9, 0);
        let instants = window(
            start,
            ALMATY,
            Some(Recurrence::Monthly),
            before(start, ALMATY),
            3,
        );
        assert_eq!(
            readings(&instants, ALMATY),
            ["2026-01-31 09:00", "2026-02-28 09:00", "2026-03-31 09:00"],
            "February must not be skipped, and March must not stay clamped"
        );
    }

    #[test]
    fn the_twenty_ninth_of_february_falls_back_in_ordinary_years() {
        let start = wall(2028, 2, 29, 9, 0);
        let instants = window(
            start,
            ALMATY,
            Some(Recurrence::Yearly),
            before(start, ALMATY),
            2,
        );
        assert_eq!(
            readings(&instants, ALMATY),
            ["2028-02-29 09:00", "2029-02-28 09:00"]
        );
    }

    #[test]
    fn a_daily_reminder_keeps_its_hour_across_a_daylight_saving_change() {
        // Berlin puts the clocks forward on 29 March 2026.
        let berlin = chrono_tz::Europe::Berlin;
        let start = wall(2026, 3, 28, 9, 0);
        let instants = window(
            start,
            berlin,
            Some(Recurrence::Daily),
            before(start, berlin),
            2,
        );
        assert_eq!(
            readings(&instants, berlin),
            ["2026-03-28 09:00", "2026-03-29 09:00"],
            "adding a fixed 24 hours would have produced 10:00 on the Sunday"
        );
    }

    #[test]
    fn mornings_missed_while_the_phone_was_off_are_not_all_delivered_at_once() {
        let start = wall(2026, 8, 3, 9, 0);
        // A week later: the first six occurrences are behind us.
        let now = resolve(wall(2026, 8, 9, 12, 0), ALMATY);
        let instants = window(start, ALMATY, Some(Recurrence::Daily), now, 2);
        assert_eq!(
            readings(&instants, ALMATY),
            ["2026-08-10 09:00", "2026-08-11 09:00"]
        );
    }

    #[test]
    fn a_one_off_reminder_yields_exactly_one_instant() {
        let start = wall(2026, 8, 3, 9, 0);
        let instants = window(start, ALMATY, None, before(start, ALMATY), WINDOW);
        assert_eq!(instants.len(), 1);
    }

    #[test]
    fn a_one_off_reminder_that_has_passed_yields_nothing() {
        let start = wall(2026, 8, 3, 9, 0);
        let now = resolve(wall(2026, 8, 4, 9, 0), ALMATY);
        assert!(window(start, ALMATY, None, now, WINDOW).is_empty());
    }
}
