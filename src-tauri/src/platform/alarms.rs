//! The core's view of the device alarm clock.
//!
//! Expressed as a trait so the use cases can be tested without an Android
//! device, and so the only place that knows about Tauri plugins is the adapter
//! next door.

use crate::domain::clock::Timestamp;
use crate::error::AppResult;

/// One alarm to arm on the device.
#[derive(Debug, Clone)]
pub struct Alarm {
    pub occurrence_id: String,
    /// Note the notification opens when tapped.
    pub note_id: String,
    /// Stable key the OS uses to replace or cancel this alarm.
    pub request_code: i32,
    pub trigger_at: Timestamp,
    pub title: String,
    pub body: String,
    /// Whether to ask for an exact alarm. The answer comes back from
    /// [`AlarmClock::schedule`], because the OS is free to refuse.
    pub exact: bool,
    pub sound_id: String,
    pub sound_label: String,
    pub vibrate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlarmPermissions {
    pub notifications_granted: bool,
    pub exact_allowed: bool,
}

pub trait AlarmClock: Send + Sync {
    /// Arms the alarm and reports whether it ended up exact.
    ///
    /// # Errors
    /// Fails when the platform refuses to schedule.
    fn schedule(&self, alarm: &Alarm) -> AppResult<bool>;

    /// Cancels an alarm. Cancelling one that is not armed succeeds.
    ///
    /// # Errors
    /// Fails when the platform call itself fails.
    fn cancel(&self, request_code: i32) -> AppResult<()>;

    /// Cancels every alarm this app has armed.
    ///
    /// Restoring a backup replaces the reminders wholesale, and the alarms the
    /// OS still holds refer to occurrences that no longer exist.
    ///
    /// # Errors
    /// Fails when the platform call itself fails.
    fn cancel_all(&self) -> AppResult<()>;

    /// Collects the note a notification tap asked to open, clearing it so the
    /// same tap is not honoured twice.
    ///
    /// # Errors
    /// Fails when the platform call itself fails.
    fn take_launch_target(&self) -> AppResult<Option<String>>;

    /// # Errors
    /// Fails when the platform call itself fails.
    fn permissions(&self) -> AppResult<AlarmPermissions>;

    /// Shows the system prompt and reports whether notifications may now be
    /// posted.
    ///
    /// # Errors
    /// Fails when the platform call itself fails.
    fn request_notification_permission(&self) -> AppResult<bool>;
}
