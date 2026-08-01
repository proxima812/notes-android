//! Adapter from [`AlarmClock`] to the Android plugin.
//!
//! The only file in the core that knows the plugin exists.

use tauri::{AppHandle, Runtime};
use tauri_plugin_reminders::{CancelRequest, RemindersExt as _, ScheduleRequest};

use crate::error::{AppResult, NotificationError, PlatformError};
use crate::platform::alarms::{Alarm, AlarmClock, AlarmPermissions};

pub struct TauriAlarmClock<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriAlarmClock<R> {
    #[must_use]
    pub const fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> AlarmClock for TauriAlarmClock<R> {
    fn schedule(&self, alarm: &Alarm) -> AppResult<bool> {
        let request = ScheduleRequest {
            occurrence_id: alarm.occurrence_id.clone(),
            request_code: alarm.request_code,
            trigger_at_millis: alarm.trigger_at.as_millis(),
            title: alarm.title.clone(),
            body: alarm.body.clone(),
            exact: alarm.exact,
            sound_id: alarm.sound_id.clone(),
            sound_label: alarm.sound_label.clone(),
            vibrate: alarm.vibrate,
        };

        self.app
            .reminders()
            .schedule(&request)
            .map(|response| response.scheduled_exact)
            .map_err(|error| {
                NotificationError::ScheduleFailed {
                    reason: error.to_string(),
                }
                .into()
            })
    }

    fn cancel(&self, request_code: i32) -> AppResult<()> {
        self.app
            .reminders()
            .cancel(CancelRequest { request_code })
            .map_err(|error| {
                PlatformError::PluginCall {
                    reason: error.to_string(),
                }
                .into()
            })
    }

    fn permissions(&self) -> AppResult<AlarmPermissions> {
        self.app
            .reminders()
            .permissions()
            .map(|state| AlarmPermissions {
                notifications_granted: state.notifications.is_granted(),
                exact_allowed: state.exact_alarms,
            })
            .map_err(|error| {
                PlatformError::PluginCall {
                    reason: error.to_string(),
                }
                .into()
            })
    }

    fn request_notification_permission(&self) -> AppResult<bool> {
        self.app
            .reminders()
            .request_notification_permission()
            .map(|response| response.notifications.is_granted())
            .map_err(|error| {
                PlatformError::PluginCall {
                    reason: error.to_string(),
                }
                .into()
            })
    }
}
