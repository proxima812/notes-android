use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::{
    Availability, DictationRequest, LanguageSupport, PermissionOutcome, StartRequest,
};

/// The desktop half exists so the crate compiles on the development machine.
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Speech<R>> {
    Ok(Speech(app.clone()))
}

/// Access to Android's speech recogniser.
pub struct Speech<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Speech<R> {
    /// Off-device there is no recogniser, and saying so beats refusing: the
    /// screen asks this to decide whether to offer dictation at all, and it
    /// must still render when the answer is no.
    ///
    /// # Errors
    /// Never.
    pub fn availability(&self) -> crate::Result<Availability> {
        Ok(Availability::default())
    }

    /// # Errors
    /// Always: there is no microphone permission to ask for off-device.
    pub fn request_permission(&self) -> crate::Result<PermissionOutcome> {
        Err(crate::Error::Unsupported)
    }

    /// Off-device nothing is known about any language, which is the same answer
    /// an Android 12 phone gives.
    ///
    /// # Errors
    /// Never.
    pub fn language_support(&self) -> crate::Result<LanguageSupport> {
        Ok(LanguageSupport::default())
    }

    /// # Errors
    /// Always: there is no model to fetch off-device.
    pub fn download_language(&self, _language: String) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    /// # Errors
    /// Always: there is no Android settings screen to open.
    pub fn open_app_settings(&self) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    /// Off-device there is no launcher and so no shortcut.
    ///
    /// # Errors
    /// Never.
    pub fn take_dictation_request(&self) -> crate::Result<DictationRequest> {
        Ok(DictationRequest::default())
    }

    /// # Errors
    /// Always: there is no recogniser off-device.
    pub fn start(&self, _request: StartRequest) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    /// Stopping something that never started is not a failure — the screen
    /// unmounting calls this without asking whether it had begun.
    ///
    /// # Errors
    /// Never.
    pub fn stop(&self) -> crate::Result<()> {
        Ok(())
    }

    /// # Errors
    /// Never, for the same reason as [`Self::stop`].
    pub fn cancel(&self) -> crate::Result<()> {
        Ok(())
    }
}
