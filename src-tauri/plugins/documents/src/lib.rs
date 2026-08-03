//! The Android document picker, for the Rust core.
//!
//! The plugin carries bytes and nothing else: which file to write, and whether
//! a file read back is one the app may accept, are decisions the core makes.
//! Because every file is one the user pointed at in the system picker, the app
//! needs no storage permission at all.

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Documents;
#[cfg(mobile)]
use mobile::Documents;

/// Reaches the picker from [`tauri::App`], [`tauri::AppHandle`] and
/// [`tauri::Window`].
pub trait DocumentsExt<R: Runtime> {
    fn documents(&self) -> &Documents<R>;
}

impl<R: Runtime, T: Manager<R>> crate::DocumentsExt<R> for T {
    fn documents(&self) -> &Documents<R> {
        self.state::<Documents<R>>().inner()
    }
}

/// Initialises the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("documents")
        .setup(|app, api| {
            #[cfg(mobile)]
            let documents = mobile::init(app, api)?;
            #[cfg(desktop)]
            let documents = desktop::init(app, api)?;
            app.manage(documents);
            Ok(())
        })
        .build()
}
