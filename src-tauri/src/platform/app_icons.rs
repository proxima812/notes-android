//! The core's view of the launcher icon.

use crate::error::AppResult;

pub trait AppIconSwitch: Send + Sync {
    /// Enables `alias` and disables every other alias in `known`.
    ///
    /// # Errors
    /// Fails when the platform refuses to change the component state.
    fn select(&self, alias: &str, known: &[String]) -> AppResult<()>;

    /// The alias currently enabled, if the platform reports one.
    ///
    /// # Errors
    /// Fails when the platform call itself fails.
    fn current(&self, known: &[String]) -> AppResult<Option<String>>;
}
