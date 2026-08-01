//! Local-only notes, tasks and reminders.
//!
//! The Rust core owns every rule in the product: persistence, search,
//! recurrence, parsing, encryption and validation. React draws the screens and
//! Kotlin reaches for Android APIs; neither holds business logic.

pub mod application;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod state;

use tauri::Manager as _;

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
        .setup(|app| {
            // The database lives in the app's private directory, which Android
            // wipes on uninstall and keeps out of reach of other apps.
            let data_dir = app.path().app_data_dir()?;
            let state = AppState::bootstrap(&data_dir)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            application::commands::app_info,
            application::commands::notes_create,
            application::commands::notes_get,
            application::commands::notes_update,
            application::commands::notes_list,
            application::commands::notes_trash,
            application::commands::notes_restore,
            application::commands::notes_purge,
            application::commands::notes_empty_trash,
            application::commands::notes_duplicate,
            application::commands::search_run,
            application::commands::search_recent,
            application::commands::search_clear_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
