//! How long the notification's "later" button moves a reminder by.
//!
//! One number for the whole app rather than one per reminder. The database has
//! carried a per-reminder column since the first schema, but nothing ever wrote
//! it: "later" is a habit — how long this person needs before being asked
//! again — and not a property of any one thing they were reminded about.

use crate::error::{AppError, AppResult, ValidationError};

pub const SNOOZE_SETTING_KEY: &str = "reminders.snooze_minutes";

/// An hour: long enough that the reminder comes back when the thing that
/// interrupted it is over, rather than in the middle of it.
pub const DEFAULT_SNOOZE_MINUTES: i64 = 60;

/// A day is the ceiling. Past that "later" stops being a postponement and
/// becomes a different reminder, which the panel already knows how to set.
pub const MAX_SNOOZE_MINUTES: i64 = 24 * 60;

/// The amounts the picker offers.
///
/// The same reasoning as the dictation leads: the answer is always one of a
/// handful of round amounts, and a stepper on a phone is a dozen taps to reach
/// an hour.
pub const OFFERED_SNOOZES: &[i64] = &[5, 10, 15, 30, 60, 120];

/// The offered amounts, with the one in use added if it is not among them.
///
/// A value restored from another build must still be visible on the screen that
/// edits it — a picker where nothing is selected reads as a setting that is off.
#[must_use]
pub fn offered(current: i64) -> Vec<i64> {
    let mut amounts = OFFERED_SNOOZES.to_vec();
    if !amounts.contains(&current) {
        amounts.push(current);
        amounts.sort_unstable();
    }
    amounts
}

/// # Errors
/// Returns a validation error for nought, a negative amount, or more than a day.
pub fn validate(minutes: i64) -> AppResult<i64> {
    if minutes <= 0 || minutes > MAX_SNOOZE_MINUTES {
        return Err(invalid());
    }
    Ok(minutes)
}

/// Reads the stored amount, falling back to the shipped one when unset.
///
/// # Errors
/// Returns a validation error when the stored value is not a number this build
/// accepts.
pub fn parse_stored(raw: Option<&str>) -> AppResult<i64> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_SNOOZE_MINUTES);
    };
    validate(raw.trim().parse::<i64>().map_err(|_| invalid())?)
}

fn invalid() -> AppError {
    AppError::Validation(ValidationError::Invalid {
        field: "snoozeMinutes",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_setting_falls_back_to_an_hour() {
        assert_eq!(parse_stored(None).expect("defaults resolve"), 60);
    }

    #[test]
    fn a_stored_amount_is_read_back() {
        assert_eq!(parse_stored(Some("15")).expect("parses"), 15);
    }

    #[test]
    fn nought_minutes_is_not_a_postponement() {
        // Zero would put the reminder back at the instant it was dismissed,
        // which is the notification refusing to go away rather than a setting.
        assert_eq!(
            parse_stored(Some("0")).expect_err("must reject").code(),
            "validation_invalid"
        );
    }

    #[test]
    fn longer_than_a_day_is_rejected() {
        assert!(validate(MAX_SNOOZE_MINUTES).is_ok());
        assert!(validate(MAX_SNOOZE_MINUTES + 1).is_err());
    }

    #[test]
    fn a_value_that_is_not_a_number_is_a_validation_error() {
        assert_eq!(
            parse_stored(Some("час")).expect_err("must reject").code(),
            "validation_invalid"
        );
    }

    #[test]
    fn the_offered_amounts_are_the_six_the_product_asked_for() {
        assert_eq!(offered(DEFAULT_SNOOZE_MINUTES), [5, 10, 15, 30, 60, 120]);
    }

    #[test]
    fn an_amount_from_somewhere_else_is_added_to_the_list_rather_than_hidden() {
        assert_eq!(offered(45), [5, 10, 15, 30, 45, 60, 120]);
    }
}
