//! Times offered for one-tap picking when setting a reminder.
//!
//! The set is the user's, not the app's: the six shipped times are a starting
//! point they can add to and delete from, including deleting all of them. That
//! is why an empty list is a real stored state and only the *absence* of the
//! setting means "never touched, use ours".

use crate::error::{AppError, AppResult, ValidationError};

pub const TIME_PRESETS_SETTING_KEY: &str = "reminders.time_presets";

/// Bound on how many presets are kept.
///
/// A picker is only useful while it can be read at a glance, and the value also
/// stops a stuck client from growing the settings row without limit.
pub const MAX_TIME_PRESETS: usize = 24;

/// A time of day, with no date and no timezone attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimePreset {
    minutes_from_midnight: u16,
}

impl TimePreset {
    /// # Errors
    /// Returns a validation error for a time that does not exist on a clock.
    pub fn new(hour: u8, minute: u8) -> AppResult<Self> {
        if hour > 23 || minute > 59 {
            return Err(invalid());
        }
        Ok(Self {
            minutes_from_midnight: u16::from(hour) * 60 + u16::from(minute),
        })
    }

    /// Parses the `HH:MM` form the picker stores and the UI puts in an input.
    ///
    /// # Errors
    /// Returns a validation error for anything that is not exactly `HH:MM` with
    /// a real time in it.
    pub fn parse(text: &str) -> AppResult<Self> {
        let (hour, minute) = text.split_once(':').ok_or_else(invalid)?;
        // Length is checked rather than left to `parse`, which would happily
        // accept `7:0`, `+7:00` and ` 07:00` and then round-trip them into a
        // list whose entries no longer compare equal to the same time typed
        // the ordinary way.
        if hour.len() != 2 || minute.len() != 2 {
            return Err(invalid());
        }
        let hour: u8 = hour.parse().map_err(|_| invalid())?;
        let minute: u8 = minute.parse().map_err(|_| invalid())?;
        Self::new(hour, minute)
    }

    // Both fit a `u8` by construction: the field is only ever set from a
    // checked time, so it stays below 24 * 60.
    #[must_use]
    pub const fn hour(self) -> u8 {
        (self.minutes_from_midnight / 60) as u8
    }

    #[must_use]
    pub const fn minute(self) -> u8 {
        (self.minutes_from_midnight % 60) as u8
    }

    /// The `HH:MM` form, zero-padded so `<input type="time">` accepts it.
    #[must_use]
    pub fn label(self) -> String {
        format!("{:02}:{:02}", self.hour(), self.minute())
    }
}

/// The times shipped with the app, in the order they read on a clock.
pub const DEFAULT_TIME_PRESETS: &[u16] = &[7 * 60, 9 * 60, 12 * 60, 16 * 60, 18 * 60, 23 * 60];

#[must_use]
pub fn default_time_presets() -> Vec<TimePreset> {
    DEFAULT_TIME_PRESETS
        .iter()
        .map(|&minutes_from_midnight| TimePreset {
            minutes_from_midnight,
        })
        .collect()
}

/// Turns client-supplied times into the canonical set to store.
///
/// Duplicates are dropped and the result is sorted: two clients that added the
/// same times in a different order must end up with the same stored value, and
/// a picker listing 18:00 before 09:00 reads as a bug.
///
/// # Errors
/// Returns a validation error when a value is not a real `HH:MM` time or when
/// there are more of them than [`MAX_TIME_PRESETS`].
pub fn normalise<S: AsRef<str>>(values: &[S]) -> AppResult<Vec<TimePreset>> {
    let mut presets = values
        .iter()
        .map(|value| TimePreset::parse(value.as_ref()))
        .collect::<AppResult<Vec<_>>>()?;
    presets.sort_unstable();
    presets.dedup();

    if presets.len() > MAX_TIME_PRESETS {
        return Err(invalid());
    }
    Ok(presets)
}

/// Reads the stored setting, falling back to the shipped times when unset.
///
/// # Errors
/// Returns a validation error when the stored value is not the JSON array of
/// `HH:MM` strings this module writes.
pub fn parse_stored(raw: Option<&str>) -> AppResult<Vec<TimePreset>> {
    let Some(raw) = raw else {
        return Ok(default_time_presets());
    };
    let values: Vec<String> = serde_json::from_str(raw).map_err(|_| invalid())?;
    normalise(&values)
}

/// # Errors
/// Returns an internal error only if serialising a list of strings fails.
pub fn serialise(presets: &[TimePreset]) -> AppResult<String> {
    let labels: Vec<String> = presets.iter().map(|preset| preset.label()).collect();
    serde_json::to_string(&labels).map_err(|_| invalid())
}

fn invalid() -> AppError {
    AppError::Validation(ValidationError::Invalid {
        field: "time_presets",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_times_are_the_six_the_product_asked_for() {
        let labels: Vec<String> = default_time_presets()
            .iter()
            .map(|preset| preset.label())
            .collect();
        assert_eq!(
            labels,
            ["07:00", "09:00", "12:00", "16:00", "18:00", "23:00"]
        );
    }

    #[test]
    fn an_unset_setting_falls_back_to_the_shipped_times() {
        let presets = parse_stored(None).expect("defaults resolve");
        assert_eq!(presets, default_time_presets());
    }

    #[test]
    fn deleting_every_preset_is_stored_and_not_read_back_as_the_defaults() {
        let stored = serialise(&[]).expect("serialises");
        let presets = parse_stored(Some(&stored)).expect("empty list is valid");
        assert!(presets.is_empty());
    }

    #[test]
    fn presets_are_sorted_and_deduplicated() {
        let presets = normalise(&["23:00", "07:00", "23:00"]).expect("normalises");
        let labels: Vec<String> = presets.iter().map(|preset| preset.label()).collect();
        assert_eq!(labels, ["07:00", "23:00"]);
    }

    #[test]
    fn a_custom_time_survives_a_round_trip() {
        let presets = normalise(&["06:45"]).expect("normalises");
        let stored = serialise(&presets).expect("serialises");
        assert_eq!(parse_stored(Some(&stored)).expect("parses"), presets);
    }

    #[test]
    fn times_that_do_not_exist_on_a_clock_are_rejected() {
        for text in ["24:00", "07:60", "7:00", "0700", "", "aa:bb", "-1:00"] {
            assert!(
                TimePreset::parse(text).is_err(),
                "{text} must not parse as a time"
            );
        }
    }

    #[test]
    fn a_stored_value_that_is_not_our_json_is_a_validation_error() {
        let error = parse_stored(Some("07:00,09:00")).expect_err("must reject");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn more_presets_than_the_cap_are_rejected() {
        let values: Vec<String> = (0..=MAX_TIME_PRESETS)
            .map(|index| format!("00:{index:02}"))
            .collect();
        assert!(normalise(&values).is_err());
    }

    #[test]
    fn midnight_and_the_last_minute_of_the_day_are_both_valid() {
        assert_eq!(TimePreset::parse("00:00").expect("parses").label(), "00:00");
        assert_eq!(TimePreset::parse("23:59").expect("parses").label(), "23:59");
    }
}
