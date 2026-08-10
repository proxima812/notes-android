use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

/// Asks for one recognition.
///
/// `prefer_offline` is on by default and is the whole reason this plugin exists
/// rather than the system dictation dialog: the app promises to work with no
/// server, and Android's recogniser will happily reach one unless told not to.
/// A device with no offline model installed reports an error instead, which the
/// screen can say out loud.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    /// BCP 47 tag — `ru-RU`, `en-US`. The interface language, unless the caller
    /// says otherwise.
    pub language: String,
    pub prefer_offline: bool,
    /// Where loudness readings, partial text and the final line are sent.
    pub on_event: Channel<serde_json::Value>,
}

/// Whether dictation is possible at all, before anything is offered on screen.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    /// False on a device with no recognition service — an AOSP build with no
    /// Google app, for instance.
    pub available: bool,
    /// Whether the microphone permission is already granted.
    pub granted: bool,
    /// False when the only recogniser available is one that may reach a server.
    ///
    /// The app promises to work on the device alone, and on Android 11 and
    /// below `EXTRA_PREFER_OFFLINE` is the strongest thing it can say. Saying so
    /// on the screen beats claiming a guarantee the platform will not give.
    pub offline_guaranteed: bool,
}

/// What the device can recognise without a network.
///
/// `known` is false where Android cannot be asked — before Android 13 — and an
/// empty list then means "no idea", not "nothing". The caller keeps its own
/// preference in that case rather than falling back to a language nobody chose.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSupport {
    pub known: bool,
    /// Models already on the device.
    pub installed: Vec<String>,
    /// Models the device could fetch.
    pub supported: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOutcome {
    pub granted: bool,
    /// True when Android will no longer show the prompt: the person refused for
    /// good, and the only way back is the settings screen.
    pub blocked: bool,
}

/// Whether the app was opened by the launcher's "Dictate" shortcut.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictationRequest {
    pub requested: bool,
}

/// Names the language a model is wanted for.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageArgs {
    pub language: String,
}

/// Answer of a call that only had to succeed.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Empty {}
