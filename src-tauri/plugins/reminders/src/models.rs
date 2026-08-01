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
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleResponse {
    /// False when the alarm had to fall back to an inexact one.
    pub scheduled_exact: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    pub request_code: i32,
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
            request_code: 7,
            trigger_at_millis: 1_000,
            title: "Title".into(),
            body: "Body".into(),
            exact: true,
            sound_id: "death_and_rebirth".into(),
            sound_label: "Death & Rebirth".into(),
            vibrate: true,
        })
        .expect("serializes");

        assert_eq!(value["soundId"], "death_and_rebirth");
        assert_eq!(value["soundLabel"], "Death & Rebirth");
        assert_eq!(value["vibrate"], true);
    }
}
