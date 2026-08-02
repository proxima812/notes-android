use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::{ExportRequest, ImportRequest, PickerOutcome};

/// Binds the Kotlin class registered under this identifier.
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Documents<R>> {
    let handle = api.register_android_plugin("dev.local.organizer.documents", "DocumentsPlugin")?;
    Ok(Documents(handle))
}

/// Access to the Android document picker.
pub struct Documents<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Documents<R> {
    /// Shows the "create document" picker and copies the file out.
    ///
    /// Blocks until the user decides, so it belongs on a blocking task rather
    /// than on the thread serving the WebView.
    ///
    /// # Errors
    /// Fails when the file cannot be read or the chosen destination cannot be
    /// written. Backing out of the picker is reported in the outcome instead.
    pub fn export(&self, request: &ExportRequest) -> crate::Result<PickerOutcome> {
        self.0
            .run_mobile_plugin("exportDocument", request.clone())
            .map_err(Into::into)
    }

    /// Shows the "open document" picker and copies the file in.
    ///
    /// # Errors
    /// Fails when the chosen file cannot be read or the destination cannot be
    /// written. Backing out of the picker is reported in the outcome instead.
    pub fn import(&self, request: &ImportRequest) -> crate::Result<PickerOutcome> {
        self.0
            .run_mobile_plugin("importDocument", request.clone())
            .map_err(Into::into)
    }
}
