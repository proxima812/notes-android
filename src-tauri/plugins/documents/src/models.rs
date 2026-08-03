use serde::{Deserialize, Serialize};

/// Asks the user where to put a file the core has already written.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    /// File in the app's private directory to copy out.
    pub source_path: String,
    /// Name to offer in the picker. The user may change it.
    pub suggested_name: String,
    pub mime_type: String,
}

/// Asks the user for a file and copies it where the core can read it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    /// Path in the app's private directory to write the chosen file to.
    pub destination_path: String,
    pub mime_type: String,
}

/// What came of showing the picker.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickerOutcome {
    /// False when the user backed out, which is not an error.
    pub completed: bool,
    /// The name the user sees for the file, for showing back to them.
    pub display_name: Option<String>,
}
