use serde::{Deserialize, Serialize};

/// A single alarm to arm.
///
/// Everything the notification needs to render travels with the alarm, because
/// it fires in a broadcast receiver that may run with the app process dead and
/// no database open.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleRequest {
    /// Occurrence this alarm belongs to; handed back when the user taps.
    pub occurrence_id: String,
    /// Note the notification opens, carried so the tap lands on the note rather
    /// than on whatever screen the app happened to be showing.
    pub note_id: String,
    /// Stable key `AlarmManager` uses to replace or cancel this alarm.
    pub request_code: i32,
    pub trigger_at_millis: i64,
    pub title: String,
    pub body: String,
    /// Request an exact alarm. The OS may refuse, so the response reports what
    /// was actually armed rather than what was asked for.
    pub exact: bool,
    /// Android raw-resource name selected by the validated Rust catalog.
    pub sound_id: String,
    /// Human-readable channel label shown in Android settings.
    pub sound_label: String,
    pub vibrate: bool,
    /// Minutes the notification's "later" button moves the reminder by.
    pub snooze_minutes: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleResponse {
    /// False when the alarm had to fall back to an inexact one.
    pub scheduled_exact: bool,
}

/// The note a notification tap asked to open, if there is one waiting.
///
/// Reading it clears it: a tap is a single navigation, and returning it twice
/// would drag the user back into the note every time they leave it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTarget {
    /// Absent both when the app was opened normally and when the tap has
    /// already been collected.
    #[serde(default)]
    pub note_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    pub request_code: i32,
}

/// How many alarms were taken back, for the log.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAllResponse {
    pub cancelled: u32,
}

/// One selectable notification sound as the device reports it.
///
/// The id carries its origin as a prefix — `system:<uri>` for a ringtone the
/// OS ships, `custom:<fileName>` for a file the user imported — so the Rust
/// core can tell them apart without a second field.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundOption {
    pub id: String,
    pub label: String,
}

/// Every sound the device offers beyond the bundled presets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSounds {
    #[serde(default)]
    pub system: Vec<SoundOption>,
    #[serde(default)]
    pub custom: Vec<SoundOption>,
}

/// What came back from the system file picker.
///
/// Mirrors [`BackupOutcomeDto`]-style reporting: backing out of the picker is
/// `completed: false`, not an error.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickCustomOutcome {
    pub completed: bool,
    #[serde(default)]
    pub sound: Option<SoundOption>,
}

/// Names one sound for the commands that act on a single id.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundIdArgs {
    pub id: String,
}

/// Mirrors the states Tauri's Android permission layer reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionState {
    Granted,
    Denied,
    Prompt,
    PromptWithRationale,
}

impl PermissionState {
    #[must_use]
    pub const fn is_granted(self) -> bool {
        matches!(self, Self::Granted)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderPermissions {
    /// Whether notifications may be posted at all. Denied does not stop an
    /// alarm from being armed — it only means nobody will see it.
    pub notifications: PermissionState,
    /// Whether the OS currently allows exact alarms. Revocable by the user at
    /// any time, so it is read before every scheduling decision.
    pub exact_alarms: bool,
}

#[cfg(mobile)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestPermissionArgs {
    pub permissions: &'static [&'static str],
}

/// Result of a permission prompt, keyed by the alias declared in Kotlin.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestResponse {
    pub notifications: PermissionState,
}

/// Stands in for a command that returns nothing.
///
/// An empty JSON object deserialises into this, whereas `()` would require a
/// JSON null; being explicit keeps the two sides from drifting.
#[cfg(mobile)]
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct Empty {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_request_serializes_the_sound_contract_in_camel_case() {
        let value = serde_json::to_value(ScheduleRequest {
            occurrence_id: "occurrence".into(),
            note_id: "note".into(),
            request_code: 7,
            trigger_at_millis: 1_000,
            title: "Title".into(),
            body: "Body".into(),
            exact: true,
            sound_id: "death_and_rebirth".into(),
            sound_label: "Death & Rebirth".into(),
            vibrate: true,
            snooze_minutes: 10,
        })
        .expect("serializes");

        assert_eq!(value["soundId"], "death_and_rebirth");
        assert_eq!(value["soundLabel"], "Death & Rebirth");
        assert_eq!(value["vibrate"], true);
        assert_eq!(value["noteId"], "note");
        assert_eq!(
            value["snoozeMinutes"], 10,
            "the Kotlin side reads this key, so the casing is part of the contract"
        );
    }

    #[test]
    fn launch_target_reads_an_object_without_the_key_as_nothing_pending() {
        // Kotlin's `JSObject.put` drops the key when the value is null, so the
        // "nothing to open" answer arrives as `{}` rather than as an explicit
        // null.
        let target: LaunchTarget = serde_json::from_str("{}").expect("deserializes");
        assert_eq!(target.note_id, None);
    }

    #[test]
    fn device_sounds_read_an_empty_object_as_no_sounds() {
        // Kotlin's `JSObject.put` drops empty answers, so both lists default.
        let sounds: DeviceSounds = serde_json::from_str("{}").expect("deserializes");
        assert!(sounds.system.is_empty());
        assert!(sounds.custom.is_empty());
    }

    #[test]
    fn a_cancelled_pick_arrives_without_a_sound() {
        let outcome: PickCustomOutcome =
            serde_json::from_str(r#"{"completed":false}"#).expect("deserializes");
        assert!(!outcome.completed);
        assert!(outcome.sound.is_none());
    }

    #[test]
    fn a_completed_pick_carries_the_sound_in_camel_case() {
        let outcome: PickCustomOutcome = serde_json::from_str(
            r#"{"completed":true,"sound":{"id":"custom:beep.ogg","label":"beep"}}"#,
        )
        .expect("deserializes");
        assert!(outcome.completed);
        let sound = outcome.sound.expect("sound present");
        assert_eq!(sound.id, "custom:beep.ogg");
        assert_eq!(sound.label, "beep");
    }
}
