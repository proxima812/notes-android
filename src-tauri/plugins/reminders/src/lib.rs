//! Android alarms and notifications for the Rust core.
//!
//! The plugin is deliberately thin: it arms, cancels and reports permissions,
//! and holds no rule about when a reminder should fire. Deciding that is the
//! core's job, which is why nothing here reads the database and why the WebView
//! is given no command to call.

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
use desktop::Reminders;
#[cfg(mobile)]
use mobile::Reminders;

/// Reaches the reminder APIs from [`tauri::App`], [`tauri::AppHandle`] and
/// [`tauri::Window`].
pub trait RemindersExt<R: Runtime> {
    fn reminders(&self) -> &Reminders<R>;
}

impl<R: Runtime, T: Manager<R>> crate::RemindersExt<R> for T {
    fn reminders(&self) -> &Reminders<R> {
        self.state::<Reminders<R>>().inner()
    }
}

/// Initialises the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("reminders")
        .setup(|app, api| {
            #[cfg(mobile)]
            let reminders = mobile::init(app, api)?;
            #[cfg(desktop)]
            let reminders = desktop::init(app, api)?;
            app.manage(reminders);
            Ok(())
        })
        .build()
}
