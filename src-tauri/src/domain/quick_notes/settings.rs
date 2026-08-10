//! What the user set up once, so dictating stays a single press.
//!
//! Both values answer a question the spoken phrase deliberately does not:
//! "встреча, 15:00" says when the meeting is, never when to be warned about it,
//! and "купить молоко" says nothing about time at all. Guessing differently on
//! each phrase would make the feature unpredictable, so the answers are settings
//! and the phrase only overrides what it actually names.

use serde::{Deserialize, Serialize};

use crate::domain::reminders::time_presets::TimePreset;
use crate::error::{AppError, AppResult, ValidationError};

pub const QUICK_NOTES_SETTING_KEY: &str = "quick_notes.settings";

/// How long before the spoken time the reminder may be placed.
///
/// A day is the ceiling: past that the reminder stops being "before the
/// meeting" and becomes a different appointment, which is what the ordinary
/// reminder panel is for.
pub const MAX_LEAD_MINUTES: u16 = 24 * 60;

/// The lead shipped with the app: half an hour is enough to finish what you are
/// doing and get somewhere, and short enough that the reminder still reads as
/// being about the thing that was said.
pub const DEFAULT_LEAD_MINUTES: u16 = 30;

/// Where a phrase with no time in it lands. Evening, because an errand
/// dictated during the day is one you act on when the day frees up.
pub const DEFAULT_FALLBACK_HOUR: u8 = 19;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickNoteSettings {
    /// Minutes between the reminder and the time the phrase named.
    lead_minutes: u16,
    /// The time of day used when the phrase named none.
    fallback_time: TimePreset,
}

impl Default for QuickNoteSettings {
    fn default() -> Self {
        Self {
            lead_minutes: DEFAULT_LEAD_MINUTES,
            fallback_time: TimePreset::new(DEFAULT_FALLBACK_HOUR, 0)
                .expect("the shipped fallback hour is a real time"),
        }
    }
}

impl QuickNoteSettings {
    /// # Errors
    /// Returns a validation error when the lead is longer than a day.
    pub fn new(lead_minutes: u16, fallback_time: TimePreset) -> AppResult<Self> {
        if lead_minutes > MAX_LEAD_MINUTES {
            return Err(invalid());
        }
        Ok(Self {
            lead_minutes,
            fallback_time,
        })
    }

    #[must_use]
    pub const fn lead_minutes(self) -> u16 {
        self.lead_minutes
    }

    #[must_use]
    pub const fn fallback_time(self) -> TimePreset {
        self.fallback_time
    }
}

/// The leads the picker offers.
///
/// Not a free number field: the answer is always one of a handful of round
/// amounts, and a stepper on a phone is six taps to reach half an hour. Nought
/// is on the list because "ring at the time I said" is a real preference rather
/// than a missing setting.
pub const OFFERED_LEADS: &[u16] = &[0, 5, 10, 15, 30, 60, 120];

/// The offered leads, with the one in use added if it is not among them.
///
/// A backup restored from another build, or a value some future client wrote,
/// must still be visible on the screen that edits it — a picker where nothing
/// is selected reads as a setting that is off.
#[must_use]
pub fn offered_leads(current: u16) -> Vec<u16> {
    let mut leads = OFFERED_LEADS.to_vec();
    if !leads.contains(&current) {
        leads.push(current);
        leads.sort_unstable();
    }
    leads
}

/// The stored form: JSON, so a value added later does not need a migration.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSettings {
    lead_minutes: u16,
    fallback_time: String,
}

/// Reads the stored settings, falling back to the shipped ones when unset.
///
/// # Errors
/// Returns a validation error when the stored value is not the JSON this module
/// writes, or holds a lead or a time this build refuses.
pub fn parse_stored(raw: Option<&str>) -> AppResult<QuickNoteSettings> {
    let Some(raw) = raw else {
        return Ok(QuickNoteSettings::default());
    };
    let stored: StoredSettings = serde_json::from_str(raw).map_err(|_| invalid())?;
    QuickNoteSettings::new(
        stored.lead_minutes,
        TimePreset::parse(&stored.fallback_time)?,
    )
}

/// # Errors
/// Returns an internal error only if serialising two scalars fails.
pub fn serialise(settings: QuickNoteSettings) -> AppResult<String> {
    serde_json::to_string(&StoredSettings {
        lead_minutes: settings.lead_minutes(),
        fallback_time: settings.fallback_time().label(),
    })
    .map_err(|_| invalid())
}

fn invalid() -> AppError {
    AppError::Validation(ValidationError::Invalid {
        field: "quick_note_settings",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_setting_falls_back_to_the_shipped_pair() {
        let settings = parse_stored(None).expect("defaults resolve");
        assert_eq!(settings.lead_minutes(), DEFAULT_LEAD_MINUTES);
        assert_eq!(settings.fallback_time().label(), "19:00");
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let settings =
            QuickNoteSettings::new(15, TimePreset::parse("08:30").expect("parses")).expect("valid");
        let stored = serialise(settings).expect("serialises");
        assert_eq!(parse_stored(Some(&stored)).expect("parses"), settings);
    }

    #[test]
    fn no_lead_at_all_is_a_real_choice_rather_than_an_unset_value() {
        let settings =
            QuickNoteSettings::new(0, TimePreset::parse("19:00").expect("parses")).expect("valid");
        let stored = serialise(settings).expect("serialises");
        assert_eq!(
            parse_stored(Some(&stored)).expect("parses").lead_minutes(),
            0
        );
    }

    #[test]
    fn a_lead_longer_than_a_day_is_rejected() {
        let time = TimePreset::parse("19:00").expect("parses");
        assert!(QuickNoteSettings::new(MAX_LEAD_MINUTES, time).is_ok());
        assert!(QuickNoteSettings::new(MAX_LEAD_MINUTES + 1, time).is_err());
    }

    #[test]
    fn the_offered_leads_are_the_seven_the_product_asked_for() {
        assert_eq!(
            offered_leads(DEFAULT_LEAD_MINUTES),
            [0, 5, 10, 15, 30, 60, 120]
        );
    }

    #[test]
    fn a_lead_from_somewhere_else_is_added_to_the_list_rather_than_hidden() {
        // Otherwise the picker shows nothing selected and the setting reads as
        // switched off.
        assert_eq!(offered_leads(45), [0, 5, 10, 15, 30, 45, 60, 120]);
    }

    #[test]
    fn a_stored_value_that_is_not_our_json_is_a_validation_error() {
        let error = parse_stored(Some("30")).expect_err("must reject");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn a_stored_time_that_does_not_exist_on_a_clock_is_a_validation_error() {
        let error = parse_stored(Some(r#"{"leadMinutes":30,"fallbackTime":"25:00"}"#))
            .expect_err("must reject");
        assert_eq!(error.code(), "validation_invalid");
    }
}
