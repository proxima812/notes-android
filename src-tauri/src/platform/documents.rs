//! The core's view of the system file picker.
//!
//! A trait for the same reason the alarm clock is one: the use cases must be
//! testable without a device, and only the adapter next door may know that a
//! Tauri plugin exists.

use crate::error::AppResult;

/// What came of showing a picker.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PickedDocument {
    /// False when the user backed out. Not an error: changing your mind about
    /// where to put a file is an ordinary thing to do.
    pub completed: bool,
    /// The name the user sees for the file, for reporting back to them.
    pub display_name: Option<String>,
}

pub trait DocumentStore: Send + Sync {
    /// Asks the user where to keep `source_path` and copies it there.
    ///
    /// # Errors
    /// Fails when the file cannot be read or the chosen destination cannot be
    /// written.
    fn export(
        &self,
        source_path: &str,
        suggested_name: &str,
        mime_type: &str,
    ) -> AppResult<PickedDocument>;

    /// Asks the user for a file and copies it to `destination_path`.
    ///
    /// # Errors
    /// Fails when the chosen file cannot be read or the destination cannot be
    /// written.
    fn import(&self, destination_path: &str, mime_type: &str) -> AppResult<PickedDocument>;
}
