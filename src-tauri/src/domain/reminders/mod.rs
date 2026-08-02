//! One-shot reminders, the bundled notification-sound catalog and the times
//! offered for one-tap picking.

pub mod recurrence;
pub mod repository;
pub mod time_presets;
pub mod zones;

use crate::domain::clock::Timestamp;
use crate::domain::ids::{NoteId, ReminderId, ReminderOccurrenceId};
use crate::error::{AppError, AppResult, ValidationError};

pub use recurrence::{Recurrence, WINDOW};
pub use repository::ReminderRepository;
pub use time_presets::{TimePreset, TIME_PRESETS_SETTING_KEY};
pub use zones::{parse_zone, reinterpret, resolve};

pub const DEFAULT_SOUND_SETTING_KEY: &str = "reminders.default_sound";
pub const FALLBACK_SOUND_ID: &str = "death_and_rebirth";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub resource_name: &'static str,
}

pub const SOUND_PRESETS: &[SoundPreset] = &[SoundPreset {
    id: "death_and_rebirth",
    label: "Death & Rebirth",
    resource_name: "death_and_rebirth",
}];

#[must_use]
pub const fn sound_presets() -> &'static [SoundPreset] {
    SOUND_PRESETS
}

/// Resolves a stored selection to a concrete bundled preset.
///
/// # Errors
/// Returns a validation error when either the explicit selection or configured
/// default does not name a preset shipped by this build.
pub fn resolve_sound(selected: &str, configured_default: Option<&str>) -> AppResult<SoundPreset> {
    let concrete = if selected == "default" {
        configured_default.unwrap_or(FALLBACK_SOUND_ID)
    } else {
        selected
    };

    SOUND_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.id == concrete)
        .ok_or(AppError::Validation(ValidationError::Invalid {
            field: "sound",
        }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reminder {
    pub id: ReminderId,
    pub note_id: NoteId,
    pub title: String,
    pub body: String,
    /// The reading the user picked, as an instant in [`Self::timezone`]. Every
    /// repeat is measured from here rather than from the last one that fired.
    pub scheduled_at: Timestamp,
    pub timezone: String,
    pub sound: String,
    /// `None` for a reminder that happens once.
    pub recurrence: Option<Recurrence>,
    /// Minutes the notification's "later" button moves this reminder by.
    pub snooze_minutes: i64,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderOccurrence {
    pub id: ReminderOccurrenceId,
    pub reminder_id: ReminderId,
    pub occurrence_at: Timestamp,
    pub alarm_request_code: i32,
    pub is_exact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledReminder {
    pub reminder: Reminder,
    pub occurrence: ReminderOccurrence,
}

#[derive(Debug, Clone)]
pub struct ReminderDraft {
    /// The reminder being edited, or `None` to add another one to the note.
    pub reminder_id: Option<ReminderId>,
    pub note_id: NoteId,
    pub title: String,
    pub body: String,
    pub scheduled_at: Timestamp,
    pub timezone: String,
    pub sound: String,
    pub recurrence: Option<Recurrence>,
    /// The instants to arm, already expanded from the recurrence by the use
    /// case. Persistence writes what it is given and works out none of it.
    pub occurrences: Vec<Timestamp>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_the_bundled_default() {
        let preset = resolve_sound("default", None).expect("default resolves");
        assert_eq!(preset.id, "death_and_rebirth");
        assert_eq!(preset.resource_name, "death_and_rebirth");
    }

    #[test]
    fn unknown_sound_is_rejected() {
        let error = resolve_sound("missing", None).expect_err("unknown sound must fail");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn stored_default_uses_the_configured_catalog_entry() {
        let preset = resolve_sound("default", Some("death_and_rebirth"))
            .expect("configured default resolves");
        assert_eq!(preset.label, "Death & Rebirth");
    }
}
