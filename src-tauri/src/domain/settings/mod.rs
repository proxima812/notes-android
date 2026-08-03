//! Small pieces of state that belong to the app rather than to a note.
//!
//! Anything stored here survives a backup and a restore, which is the reason
//! it lives in the database rather than in the WebView's local storage: the
//! theme is a property of this install, but the icon a person chose is part of
//! how they set the app up.

use crate::error::AppResult;

pub trait SettingsRepository: Send + Sync {
    /// # Errors
    /// Fails on a database error.
    fn read(&self, key: &str) -> AppResult<Option<String>>;

    /// # Errors
    /// Fails on a database error.
    fn write(&self, key: &str, value: &str) -> AppResult<()>;
}
