//! Local-only notes, tasks and reminders.
//!
//! The Rust core owns every rule in the product: persistence, search,
//! recurrence, parsing, encryption and validation. React draws the screens and
//! Kotlin reaches for Android APIs; neither holds business logic.

pub mod application;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod platform;
pub mod state;

use std::sync::Arc;

use tauri::Manager as _;

use crate::platform::{
    AlarmClock, AppIconSwitch, DocumentStore, TauriAlarmClock, TauriAppIconSwitch,
    TauriDocumentStore,
};
use crate::state::AppState;

/// Installs logging.
///
/// Note content is never logged at any level: the log is a plain file on the
/// device, and a diagnostic that leaks the user's notes into it is a privacy
/// bug, not a convenience.
fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(if cfg!(debug_assertions) {
            "info"
        } else {
            "warn"
        })
    });

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(false).with_ansi(false))
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_reminders::init())
        .plugin(tauri_plugin_documents::init())
        .plugin(tauri_plugin_appicon::init())
        .plugin(tauri_plugin_speech::init())
        .setup(|app| {
            // The database lives in the app's private directory, which Android
            // wipes on uninstall and keeps out of reach of other apps.
            let data_dir = app.path().app_data_dir()?;
            // Backups are staged in the cache directory: a half-finished copy of
            // every note is exactly the kind of thing the OS should be free to
            // delete when it needs the space.
            let staging_dir = app.path().app_cache_dir()?.join("backup");
            let alarms: Arc<dyn AlarmClock> = Arc::new(TauriAlarmClock::new(app.handle().clone()));
            let documents: Arc<dyn DocumentStore> =
                Arc::new(TauriDocumentStore::new(app.handle().clone()));
            let icons: Arc<dyn AppIconSwitch> =
                Arc::new(TauriAppIconSwitch::new(app.handle().clone()));
            let state = AppState::bootstrap(&data_dir, staging_dir, alarms, documents, icons)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            application::commands::app_info,
            application::commands::links_preview,
            application::commands::links_forget_all,
            application::commands::notes_create,
            application::commands::notes_get,
            application::commands::notes_update,
            application::commands::notes_list,
            application::commands::notes_list_with_reminders,
            application::commands::notes_trash,
            application::commands::notes_restore,
            application::commands::notes_purge,
            application::commands::notes_empty_trash,
            application::commands::notes_purge_expired,
            application::commands::notes_duplicate,
            application::commands::reminders_list_for_note,
            application::commands::reminders_upsert_for_note,
            application::commands::reminders_delete,
            application::commands::reminders_take_launch_target,
            application::commands::reminders_reconcile_zone,
            application::commands::reminders_top_up,
            application::commands::reminder_sounds_list,
            application::commands::reminder_sound_pick_custom,
            application::commands::reminder_sound_delete_custom,
            application::commands::reminder_sound_preview,
            application::commands::reminder_sound_preview_stop,
            application::commands::reminder_time_presets_list,
            application::commands::reminder_time_presets_save,
            application::commands::reminder_snooze_get,
            application::commands::reminder_snooze_save,
            application::commands::quick_notes_settings,
            application::commands::quick_notes_settings_save,
            application::commands::quick_notes_create,
            application::commands::search_run,
            application::commands::search_recent,
            application::commands::search_clear_history,
            application::commands::backup_export,
            application::commands::backup_import,
            application::commands::backup_latest,
            application::commands::tasks_list_for_note,
            application::commands::tasks_create_for_note,
            application::commands::tasks_set_completed,
            application::commands::tasks_delete,
            application::commands::tasks_clear_for_note,
            application::commands::tasks_progress_for_note,
            application::commands::tags_list,
            application::commands::tags_ensure,
            application::commands::tags_delete,
            application::commands::tags_of_note,
            application::commands::tags_set_for_note,
            application::commands::app_icons_list,
            application::commands::app_icons_select,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
