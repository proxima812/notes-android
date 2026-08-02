use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::{
    CancelAllResponse, CancelRequest, LaunchTarget, PermissionRequestResponse, ReminderPermissions,
    ScheduleRequest, ScheduleResponse,
};

/// The desktop half exists so the crate compiles on the development machine —
/// `cargo check`, clippy and the test suite all run on the host. It refuses
/// loudly rather than pretending to schedule anything, because a reminder that
/// silently never fires is worse than one that never existed.
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Reminders<R>> {
    Ok(Reminders(app.clone()))
}

/// Access to the reminder APIs.
pub struct Reminders<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Reminders<R> {
    /// # Errors
    /// Always: there is no alarm clock to talk to off-device.
    pub fn schedule(&self, _request: &ScheduleRequest) -> crate::Result<ScheduleResponse> {
        Err(crate::Error::Unsupported)
    }

    /// # Errors
    /// Always: there is no alarm clock to talk to off-device.
    pub fn cancel(&self, _request: CancelRequest) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    /// Off-device there is nothing armed, so taking everything back succeeds
    /// having done nothing. Refusing here would make restoring a backup fail on
    /// the development machine for no reason.
    ///
    /// # Errors
    /// Never.
    pub fn cancel_all(&self) -> crate::Result<CancelAllResponse> {
        Ok(CancelAllResponse::default())
    }

    /// Off-device nothing can tap a notification, so there is never a target.
    ///
    /// This one answers rather than refusing: the app asks on every start, and
    /// a hard error would turn "opened normally" into a failure on the
    /// development machine.
    ///
    /// # Errors
    /// Never.
    pub fn take_launch_target(&self) -> crate::Result<LaunchTarget> {
        Ok(LaunchTarget::default())
    }

    /// # Errors
    /// Always: there is no alarm clock to talk to off-device.
    pub fn permissions(&self) -> crate::Result<ReminderPermissions> {
        Err(crate::Error::Unsupported)
    }

    /// # Errors
    /// Always: there is no alarm clock to talk to off-device.
    pub fn request_notification_permission(&self) -> crate::Result<PermissionRequestResponse> {
        Err(crate::Error::Unsupported)
    }
}
