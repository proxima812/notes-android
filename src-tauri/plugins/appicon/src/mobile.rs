use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::{CurrentIcon, SelectIconRequest};

/// Binds the Kotlin class registered under this identifier.
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<AppIcon<R>> {
    let handle = api.register_android_plugin("dev.local.organizer.appicon", "AppIconPlugin")?;
    Ok(AppIcon(handle))
}

/// Access to the launcher aliases.
pub struct AppIcon<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> AppIcon<R> {
    /// # Errors
    /// Fails when the alias is not one the build ships, or when Android
    /// refuses to change the component state.
    pub fn select(&self, request: &SelectIconRequest) -> crate::Result<CurrentIcon> {
        self.0
            .run_mobile_plugin("selectIcon", request.clone())
            .map_err(Into::into)
    }

    /// # Errors
    /// Fails when the platform call itself fails.
    pub fn current(&self, request: &SelectIconRequest) -> crate::Result<CurrentIcon> {
        self.0
            .run_mobile_plugin("currentIcon", request.clone())
            .map_err(Into::into)
    }
}
