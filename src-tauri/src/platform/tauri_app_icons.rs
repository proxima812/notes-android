//! Adapter from [`AppIconSwitch`] to the Android plugin.

use tauri::{AppHandle, Runtime};
use tauri_plugin_appicon::{AppIconExt as _, SelectIconRequest};

use crate::error::{AppResult, PlatformError};
use crate::platform::app_icons::AppIconSwitch;

pub struct TauriAppIconSwitch<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriAppIconSwitch<R> {
    #[must_use]
    pub const fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }

    fn request(alias: &str, known: &[String], fallback: &str) -> SelectIconRequest {
        SelectIconRequest {
            alias: alias.to_owned(),
            known: known.to_vec(),
            fallback: fallback.to_owned(),
        }
    }
}

impl<R: Runtime> AppIconSwitch for TauriAppIconSwitch<R> {
    fn select(&self, alias: &str, known: &[String], fallback: &str) -> AppResult<()> {
        self.app
            .app_icon()
            .select(&Self::request(alias, known, fallback))
            .map(|_| ())
            .map_err(|error| {
                PlatformError::PluginCall {
                    reason: error.to_string(),
                }
                .into()
            })
    }

    fn current(&self, known: &[String], fallback: &str) -> AppResult<Option<String>> {
        self.app
            .app_icon()
            .current(&Self::request("", known, fallback))
            .map(|current| {
                if current.alias.is_empty() {
                    None
                } else {
                    Some(current.alias)
                }
            })
            .map_err(|error| {
                PlatformError::PluginCall {
                    reason: error.to_string(),
                }
                .into()
            })
    }
}
