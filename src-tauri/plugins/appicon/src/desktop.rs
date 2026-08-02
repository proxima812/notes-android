use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::{CurrentIcon, SelectIconRequest};

/// The desktop half exists so the crate compiles on the development machine.
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<AppIcon<R>> {
    Ok(AppIcon(app.clone()))
}

/// Access to the launcher aliases.
pub struct AppIcon<R: Runtime>(AppHandle<R>);

impl<R: Runtime> AppIcon<R> {
    /// # Errors
    /// Always: there is no launcher off-device.
    pub fn select(&self, _request: &SelectIconRequest) -> crate::Result<CurrentIcon> {
        Err(crate::Error::Unsupported)
    }

    /// Off-device no alias is enabled, and saying so beats refusing: the
    /// settings screen asks on every visit and must still render.
    ///
    /// # Errors
    /// Never.
    pub fn current(&self, _request: &SelectIconRequest) -> crate::Result<CurrentIcon> {
        Ok(CurrentIcon::default())
    }
}
