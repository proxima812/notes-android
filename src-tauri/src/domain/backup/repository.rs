//! Persistence boundary for backups.

use std::path::Path;

use crate::domain::clock::Timestamp;
use crate::error::AppResult;

use super::BackupContents;

/// Making a copy of the database and putting one back.
///
/// A trait rather than a call into SQLite so the use case can be tested with a
/// stub, the same way the alarm clock is.
pub trait BackupArchive: Send + Sync {
    /// Writes a consistent copy of the live database to `destination`.
    ///
    /// # Errors
    /// Fails when the destination cannot be written.
    fn snapshot_to(&self, destination: &Path) -> AppResult<()>;

    /// Reads a candidate file without touching the live database.
    ///
    /// # Errors
    /// Fails when the file is unreadable or is not one of ours.
    fn inspect(&self, path: &Path) -> AppResult<BackupContents>;

    /// Replaces the live database with the contents of `source`.
    ///
    /// # Errors
    /// Fails when the copy or the migration that follows it fails.
    fn restore_from(&self, source: &Path) -> AppResult<()>;
}

/// A backup that was actually written somewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRecord {
    /// Where it went, as far as the app can tell — the picker hands back a name
    /// rather than a path, so this is for the user to recognise, not to open.
    pub location: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub note_count: i64,
    pub created_at: Timestamp,
}

pub trait BackupRepository: Send + Sync {
    /// # Errors
    /// Fails on a database error.
    fn record(&self, entry: &BackupRecord) -> AppResult<()>;

    /// The most recent successful backup, for telling the user how long it has
    /// been since they made one.
    ///
    /// # Errors
    /// Fails on a database error.
    fn latest(&self) -> AppResult<Option<BackupRecord>>;
}
