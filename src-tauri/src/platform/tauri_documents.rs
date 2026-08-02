//! Adapter from [`DocumentStore`] to the Android plugin.
//!
//! The only file in the core that knows the documents plugin exists.

use tauri::{AppHandle, Runtime};
use tauri_plugin_documents::{DocumentsExt as _, ExportRequest, ImportRequest};

use crate::error::{AppResult, PlatformError};
use crate::platform::documents::{DocumentStore, PickedDocument};

pub struct TauriDocumentStore<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriDocumentStore<R> {
    #[must_use]
    pub const fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> DocumentStore for TauriDocumentStore<R> {
    fn export(
        &self,
        source_path: &str,
        suggested_name: &str,
        mime_type: &str,
    ) -> AppResult<PickedDocument> {
        self.app
            .documents()
            .export(&ExportRequest {
                source_path: source_path.to_owned(),
                suggested_name: suggested_name.to_owned(),
                mime_type: mime_type.to_owned(),
            })
            .map(|outcome| PickedDocument {
                completed: outcome.completed,
                display_name: outcome.display_name,
            })
            .map_err(|error| {
                PlatformError::PluginCall {
                    reason: error.to_string(),
                }
                .into()
            })
    }

    fn import(&self, destination_path: &str, mime_type: &str) -> AppResult<PickedDocument> {
        self.app
            .documents()
            .import(&ImportRequest {
                destination_path: destination_path.to_owned(),
                mime_type: mime_type.to_owned(),
            })
            .map(|outcome| PickedDocument {
                completed: outcome.completed,
                display_name: outcome.display_name,
            })
            .map_err(|error| {
                PlatformError::PluginCall {
                    reason: error.to_string(),
                }
                .into()
            })
    }
}
