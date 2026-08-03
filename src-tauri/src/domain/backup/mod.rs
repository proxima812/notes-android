//! Copies of the whole database that the user can keep somewhere else.
//!
//! The format is the SQLite file itself rather than an export of its contents.
//! That keeps the backup exactly as complete as the app is — every note, every
//! reminder, every setting, including whatever a later version adds — and means
//! restoring is not a merge with its own conflict rules but a replacement.
//! Migrations run on a restored file the same as on any other, so a backup made
//! by an older build still opens.

pub mod repository;

use chrono_tz::Tz;

pub use repository::{BackupArchive, BackupRecord, BackupRepository};

use crate::domain::clock::Timestamp;
use crate::error::{AppError, AppResult, BackupError};

/// The extension is `.sqlite` on purpose: an opaque one would hide from the
/// user that their notes are in a format any SQLite tool can read, which is
/// most of the point of keeping the data local.
pub const BACKUP_EXTENSION: &str = "sqlite";
pub const BACKUP_MIME_TYPE: &str = "application/vnd.sqlite3";

/// What a candidate file turned out to hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupContents {
    pub schema_version: i64,
    pub note_count: i64,
    pub reminder_count: i64,
    pub size_bytes: u64,
    pub sha256: String,
}

/// A name that sorts chronologically and says what the file is.
///
/// The user's own zone is used, not UTC: a backup they made at nine in the
/// evening should not be dated the next day in the file list.
#[must_use]
pub fn file_name_for(now: Timestamp, zone: Tz) -> String {
    now.to_zoned(zone).map_or_else(
        |_| format!("xima-keeps-{}.{BACKUP_EXTENSION}", now.as_millis()),
        |local| {
            format!(
                "xima-keeps-{}.{BACKUP_EXTENSION}",
                local.format("%Y-%m-%d-%H%M")
            )
        },
    )
}

/// Decides whether a file may replace the live database.
///
/// # Errors
/// Returns [`BackupError::UnsupportedVersion`] for a file from a newer build —
/// migrations only run forwards, so restoring it would leave the app looking at
/// a schema it cannot understand. Returns [`BackupError::Corrupt`] for a file
/// that has our tables but no schema version at all.
pub fn ensure_restorable(contents: &BackupContents, supported_version: i64) -> AppResult<()> {
    if contents.schema_version <= 0 {
        return Err(AppError::Backup(BackupError::Corrupt));
    }
    if contents.schema_version > supported_version {
        return Err(AppError::Backup(BackupError::UnsupportedVersion {
            found: u32::try_from(contents.schema_version).unwrap_or(u32::MAX),
        }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contents(schema_version: i64) -> BackupContents {
        BackupContents {
            schema_version,
            note_count: 3,
            reminder_count: 1,
            size_bytes: 4096,
            sha256: "abc".into(),
        }
    }

    #[test]
    fn the_name_is_dated_in_the_users_own_zone() {
        // Half past eight in the evening UTC is already the next day in Almaty,
        // and the file name has to agree with the clock the user was looking at.
        let instant = Timestamp::from_utc(
            chrono::DateTime::parse_from_rfc3339("2026-08-02T20:30:00Z")
                .expect("parses")
                .with_timezone(&chrono::Utc),
        );
        assert_eq!(
            file_name_for(instant, chrono_tz::Asia::Almaty),
            "xima-keeps-2026-08-03-0130.sqlite"
        );
    }

    #[test]
    fn a_backup_from_this_build_restores() {
        ensure_restorable(&contents(1), 1).expect("same version is restorable");
    }

    #[test]
    fn a_backup_from_an_older_build_restores_and_is_migrated_later() {
        ensure_restorable(&contents(1), 5).expect("older version is restorable");
    }

    #[test]
    fn a_backup_from_a_newer_build_is_refused() {
        let error = ensure_restorable(&contents(6), 5).expect_err("must refuse");
        assert_eq!(error.code(), "backup_unsupported_version");
    }

    #[test]
    fn a_file_with_our_tables_but_no_version_is_corrupt() {
        let error = ensure_restorable(&contents(0), 1).expect_err("must refuse");
        assert_eq!(error.code(), "backup_corrupt");
    }
}
