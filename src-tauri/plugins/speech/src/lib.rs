//! On-device speech recognition, for dictated notes.
//!
//! The plugin carries audio nowhere: Android's recogniser turns sound into text
//! inside the device, and what crosses this boundary is a loudness reading, the
//! words heard so far, and the finished line. Nothing is written to disk and
//! nothing is kept after the screen closes.
//!
//! Offline is asked for on every recognition. The app has no server and no
//! account, and a dictation that quietly went to one would break that promise
//! in the one place a person is least likely to look.

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

mod commands;
#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Speech;
#[cfg(mobile)]
use mobile::Speech;

/// Reaches the recogniser from [`tauri::App`], [`tauri::AppHandle`] and
/// [`tauri::Window`].
pub trait SpeechExt<R: Runtime> {
    fn speech(&self) -> &Speech<R>;
}

impl<R: Runtime, T: Manager<R>> crate::SpeechExt<R> for T {
    fn speech(&self) -> &Speech<R> {
        self.state::<Speech<R>>().inner()
    }
}

/// Initialises the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("speech")
        .invoke_handler(tauri::generate_handler![
            commands::availability,
            commands::request_permission,
            commands::language_support,
            commands::download_language,
            commands::open_app_settings,
            commands::take_dictation_request,
            commands::start,
            commands::stop,
            commands::cancel,
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let speech = mobile::init(app, api)?;
            #[cfg(desktop)]
            let speech = desktop::init(app, api)?;
            app.manage(speech);
            Ok(())
        })
        .build()
}
