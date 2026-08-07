use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::{
    CancelAllResponse, CancelRequest, DeviceSounds, Empty, LaunchTarget, PermissionRequestResponse,
    PickCustomOutcome, ReminderPermissions, RequestPermissionArgs, ScheduleRequest,
    ScheduleResponse, SoundIdArgs,
};

/// Alias declared on the Kotlin plugin for `POST_NOTIFICATIONS`.
const NOTIFICATION_ALIAS: &str = "notifications";

/// Binds the Kotlin class registered under this identifier.
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Reminders<R>> {
    let handle = api.register_android_plugin("dev.local.organizer.reminders", "RemindersPlugin")?;
    Ok(Reminders(handle))
}

/// Access to the Android alarm and notification APIs.
pub struct Reminders<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Reminders<R> {
    /// Arms an alarm, replacing any alarm already using the same request code.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot reach `AlarmManager`.
    pub fn schedule(&self, request: &ScheduleRequest) -> crate::Result<ScheduleResponse> {
        self.0
            .run_mobile_plugin("schedule", request.clone())
            .map_err(Into::into)
    }

    /// Cancels an alarm. Cancelling one that never existed is not an error,
    /// which is what makes replaying the pending set after a reboot safe.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot reach `AlarmManager`.
    pub fn cancel(&self, request: CancelRequest) -> crate::Result<()> {
        self.0
            .run_mobile_plugin::<Empty>("cancel", request)
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Takes back every alarm the plugin has armed.
    ///
    /// Used when restoring a backup replaces the reminders wholesale, so the OS
    /// stops holding alarms for occurrences that no longer exist.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot reach `AlarmManager`.
    pub fn cancel_all(&self) -> crate::Result<CancelAllResponse> {
        self.0
            .run_mobile_plugin("cancelAll", ())
            .map_err(Into::into)
    }

    /// Collects the note a notification tap asked to open, clearing it.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn take_launch_target(&self) -> crate::Result<LaunchTarget> {
        self.0
            .run_mobile_plugin("takeLaunchTarget", ())
            .map_err(Into::into)
    }

    /// Reads the current permission situation. Both answers can change behind
    /// the app's back, so this is asked again rather than cached.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn permissions(&self) -> crate::Result<ReminderPermissions> {
        self.0
            .run_mobile_plugin("permissionState", ())
            .map_err(Into::into)
    }

    /// Lists the sounds the device offers beyond the bundled presets.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn list_device_sounds(&self) -> crate::Result<DeviceSounds> {
        self.0
            .run_mobile_plugin("listDeviceSounds", ())
            .map_err(Into::into)
    }

    /// Opens the system file picker so the user can import a sound of their
    /// own. Backing out is `completed: false`, not an error.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn pick_custom_sound(&self) -> crate::Result<PickCustomOutcome> {
        self.0
            .run_mobile_plugin("pickCustomSound", ())
            .map_err(Into::into)
    }

    /// Removes an imported sound file.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn delete_custom_sound(&self, id: String) -> crate::Result<()> {
        self.0
            .run_mobile_plugin::<Empty>("deleteCustomSound", SoundIdArgs { id })
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Plays a sound once so the user can hear what they are choosing.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn preview_sound(&self, id: String) -> crate::Result<()> {
        self.0
            .run_mobile_plugin::<Empty>("previewSound", SoundIdArgs { id })
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Stops whatever preview is playing. Stopping silence succeeds.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn stop_preview(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin::<Empty>("stopPreview", ())
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Shows the system prompt for posting notifications.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn request_notification_permission(&self) -> crate::Result<PermissionRequestResponse> {
        self.0
            .run_mobile_plugin(
                "requestPermissions",
                RequestPermissionArgs {
                    permissions: &[NOTIFICATION_ALIAS],
                },
            )
            .map_err(Into::into)
    }
}
