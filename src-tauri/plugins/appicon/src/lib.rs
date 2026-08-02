//! The launcher icon, for the Rust core.
//!
//! Android cannot change an icon; it can only enable and disable components.
//! Each icon is an `activity-alias` over the one real activity, and this turns
//! them on and off. Which icons exist, and which one is chosen, is the core's
//! business.

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
use desktop::AppIcon;
#[cfg(mobile)]
use mobile::AppIcon;

/// Reaches the launcher aliases from [`tauri::App`], [`tauri::AppHandle`] and
/// [`tauri::Window`].
pub trait AppIconExt<R: Runtime> {
    fn app_icon(&self) -> &AppIcon<R>;
}

impl<R: Runtime, T: Manager<R>> crate::AppIconExt<R> for T {
    fn app_icon(&self) -> &AppIcon<R> {
        self.state::<AppIcon<R>>().inner()
    }
}

/// Initialises the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("appicon")
        .setup(|app, api| {
            #[cfg(mobile)]
            let icon = mobile::init(app, api)?;
            #[cfg(desktop)]
            let icon = desktop::init(app, api)?;
            app.manage(icon);
            Ok(())
        })
        .build()
}
