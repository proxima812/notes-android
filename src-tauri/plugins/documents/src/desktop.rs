use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::{ExportRequest, ImportRequest, PickerOutcome};

/// The desktop half exists so the crate compiles on the development machine.
/// It refuses rather than writing somewhere of its own choosing: a backup the
/// user cannot find is not a backup.
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Documents<R>> {
    Ok(Documents(app.clone()))
}

/// Access to the document picker.
pub struct Documents<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Documents<R> {
    /// # Errors
    /// Always: there is no document picker off-device.
    pub fn export(&self, _request: &ExportRequest) -> crate::Result<PickerOutcome> {
        Err(crate::Error::Unsupported)
    }

    /// # Errors
    /// Always: there is no document picker off-device.
    pub fn import(&self, _request: &ImportRequest) -> crate::Result<PickerOutcome> {
        Err(crate::Error::Unsupported)
    }
}
