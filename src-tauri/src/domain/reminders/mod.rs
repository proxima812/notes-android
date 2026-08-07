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

/// Marks a sound the OS ships: `system:<uri>`.
pub const SYSTEM_SOUND_PREFIX: &str = "system:";
/// Marks a file the user imported: `custom:<fileName>`.
pub const CUSTOM_SOUND_PREFIX: &str = "custom:";

/// Label used when an imported sound is no longer on the device list.
pub const CUSTOM_SOUND_FALLBACK_LABEL: &str = "Свой звук";
/// Label used when a system sound is no longer on the device list.
pub const SYSTEM_SOUND_FALLBACK_LABEL: &str = "Системный звук";

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

/// What a stored sound selection turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSound {
    /// A preset bundled with this build.
    Preset(SoundPreset),
    /// A sound the device holds — `system:<uri>` or `custom:<fileName>`. The
    /// id is passed through as-is; only the device can say whether it still
    /// exists, so the core does not second-guess it.
    Device { id: String },
}

impl ResolvedSound {
    /// The catalog id this selection stands for.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Preset(preset) => preset.id,
            Self::Device { id } => id,
        }
    }
}

/// True for ids that name a device sound rather than a bundled preset.
#[must_use]
pub fn is_device_sound_id(id: &str) -> bool {
    id.starts_with(SYSTEM_SOUND_PREFIX) || id.starts_with(CUSTOM_SOUND_PREFIX)
}

/// Resolves a stored selection to a concrete sound.
///
/// # Errors
/// Returns a validation error when either the explicit selection or configured
/// default is a bare id that does not name a preset shipped by this build.
/// Prefixed ids (`system:`/`custom:`) are always accepted as-is.
pub fn resolve_sound(selected: &str, configured_default: Option<&str>) -> AppResult<ResolvedSound> {
    let concrete = if selected == "default" {
        configured_default.unwrap_or(FALLBACK_SOUND_ID)
    } else {
        selected
    };

    if is_device_sound_id(concrete) {
        return Ok(ResolvedSound::Device {
            id: concrete.to_owned(),
        });
    }

    SOUND_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.id == concrete)
        .map(ResolvedSound::Preset)
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
        let resolved = resolve_sound("default", None).expect("default resolves");
        let ResolvedSound::Preset(preset) = resolved else {
            panic!("the fallback is a bundled preset");
        };
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
        let resolved = resolve_sound("default", Some("death_and_rebirth"))
            .expect("configured default resolves");
        let ResolvedSound::Preset(preset) = resolved else {
            panic!("a preset id resolves to a preset");
        };
        assert_eq!(preset.label, "Death & Rebirth");
    }

    #[test]
    fn a_system_sound_id_passes_through_untouched() {
        let resolved =
            resolve_sound("system:content://media/1", None).expect("system id resolves");
        assert_eq!(
            resolved,
            ResolvedSound::Device {
                id: "system:content://media/1".to_owned()
            }
        );
    }

    #[test]
    fn a_custom_sound_id_passes_through_untouched() {
        let resolved = resolve_sound("custom:beep.ogg", None).expect("custom id resolves");
        assert_eq!(resolved.id(), "custom:beep.ogg");
    }

    #[test]
    fn a_configured_device_default_resolves_for_the_default_selection() {
        let resolved = resolve_sound("default", Some("custom:beep.ogg"))
            .expect("configured device default resolves");
        assert_eq!(resolved.id(), "custom:beep.ogg");
    }
}
