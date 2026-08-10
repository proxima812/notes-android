//! The five calls the dictation screen makes.
//!
//! Each one is a pass-through to the platform half. No decision is taken here:
//! what counts as a time, what the note is called and when the alarm goes are
//! all the core's, and this plugin never sees any of them.

use tauri::{ipc::Channel, AppHandle, Runtime};

use crate::models::{
    Availability, DictationRequest, LanguageSupport, PermissionOutcome, StartRequest,
};
use crate::SpeechExt as _;

#[tauri::command]
pub(crate) async fn availability<R: Runtime>(app: AppHandle<R>) -> crate::Result<Availability> {
    app.speech().availability()
}

#[tauri::command]
pub(crate) async fn request_permission<R: Runtime>(
    app: AppHandle<R>,
) -> crate::Result<PermissionOutcome> {
    app.speech().request_permission()
}

#[tauri::command]
pub(crate) async fn language_support<R: Runtime>(
    app: AppHandle<R>,
) -> crate::Result<LanguageSupport> {
    app.speech().language_support()
}

#[tauri::command]
pub(crate) async fn download_language<R: Runtime>(
    app: AppHandle<R>,
    language: String,
) -> crate::Result<()> {
    app.speech().download_language(language)
}

#[tauri::command]
pub(crate) async fn open_app_settings<R: Runtime>(app: AppHandle<R>) -> crate::Result<()> {
    app.speech().open_app_settings()
}

#[tauri::command]
pub(crate) async fn take_dictation_request<R: Runtime>(
    app: AppHandle<R>,
) -> crate::Result<DictationRequest> {
    app.speech().take_dictation_request()
}

#[tauri::command]
pub(crate) async fn start<R: Runtime>(
    app: AppHandle<R>,
    language: String,
    prefer_offline: bool,
    on_event: Channel<serde_json::Value>,
) -> crate::Result<()> {
    app.speech().start(StartRequest {
        language,
        prefer_offline,
        on_event,
    })
}

#[tauri::command]
pub(crate) async fn stop<R: Runtime>(app: AppHandle<R>) -> crate::Result<()> {
    app.speech().stop()
}

#[tauri::command]
pub(crate) async fn cancel<R: Runtime>(app: AppHandle<R>) -> crate::Result<()> {
    app.speech().cancel()
}
