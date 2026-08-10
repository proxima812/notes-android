use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::{
    Availability, DictationRequest, Empty, LanguageArgs, LanguageSupport, PermissionOutcome,
    StartRequest,
};

/// Binds the Kotlin class registered under this identifier.
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Speech<R>> {
    let handle = api.register_android_plugin("dev.local.organizer.speech", "SpeechPlugin")?;
    Ok(Speech(handle))
}

/// Access to Android's speech recogniser.
pub struct Speech<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Speech<R> {
    /// Whether the device can recognise speech, and whether it is allowed to
    /// listen. Both can change behind the app's back, so this is asked each
    /// time the screen opens rather than cached.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn availability(&self) -> crate::Result<Availability> {
        self.0
            .run_mobile_plugin("availability", ())
            .map_err(Into::into)
    }

    /// Shows the system microphone prompt.
    ///
    /// Answered with the permission state afterwards rather than with the
    /// generic map `requestPermissions` returns: one permission is asked for,
    /// and one boolean is what the screen does anything with.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn request_permission(&self) -> crate::Result<PermissionOutcome> {
        self.0
            .run_mobile_plugin("requestMicrophone", ())
            .map_err(Into::into)
    }

    /// The languages this device can recognise with no network.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn language_support(&self) -> crate::Result<LanguageSupport> {
        self.0
            .run_mobile_plugin("supportedLanguages", ())
            .map_err(Into::into)
    }

    /// Asks Android to fetch the offline model for a language.
    ///
    /// # Errors
    /// Fails on Android 12 and below, where there is no way to ask.
    pub fn download_language(&self, language: String) -> crate::Result<()> {
        self.0
            .run_mobile_plugin::<Empty>("downloadLanguage", LanguageArgs { language })
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Opens this app's page in the Android settings.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn open_app_settings(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin::<Empty>("openAppSettings", ())
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Whether the launcher shortcut asked for dictation. Answering clears it.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn take_dictation_request(&self) -> crate::Result<DictationRequest> {
        self.0
            .run_mobile_plugin("takeDictationRequest", ())
            .map_err(Into::into)
    }

    /// Begins listening. Everything heard arrives on the channel in the
    /// request; this call itself answers as soon as the recogniser is armed.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached or refuses to start.
    pub fn start(&self, request: StartRequest) -> crate::Result<()> {
        self.0
            .run_mobile_plugin::<Empty>("start", request)
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Stops listening and keeps what was heard, which is what a person
    /// pressing "done" means.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn stop(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin::<Empty>("stop", ())
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Stops listening and throws away what was heard. Cancelling when nothing
    /// is running succeeds, so leaving the screen never has to check first.
    ///
    /// # Errors
    /// Fails when the Kotlin side cannot be reached.
    pub fn cancel(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin::<Empty>("cancel", ())
            .map(|_| ())
            .map_err(Into::into)
    }
}
