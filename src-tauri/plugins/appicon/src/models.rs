use serde::{Deserialize, Serialize};

/// Chooses the alias to show and names the ones to switch off.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectIconRequest {
    pub alias: String,
    pub known: Vec<String>,
    /// The alias the manifest enables on a fresh install, so the platform can
    /// tell a user's choice from the one it put there itself.
    pub fallback: String,
}

/// The alias that ended up enabled. Empty when none is.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentIcon {
    pub alias: String,
}
