//! Making a copy of the database and putting one back.
//!
//! Both directions go through SQLite's own online backup API rather than
//! copying the file. Copying would race with the app's own writes and would
//! miss whatever is still sitting in the write-ahead log; the backup API takes a
//! consistent snapshot of a live database and is the only supported way to do
//! this while connections are open.
//!
//! Restoring writes *into* the existing connection instead of swapping files,
//! so every handle the app already holds stays valid and there is no window
//! where the database is missing from disk.

use std::fs::File;
use std::io::Read as _;
use std::path::Path;

use rusqlite::backup::Backup;
use rusqlite::Connection;
use sha2::{Digest as _, Sha256};

use crate::domain::backup::BackupContents;
use crate::error::{AppError, AppResult, BackupError, DatabaseError, FileSystemError};

use super::{connection::configure, migrations, Database};

impl Database {
    /// Writes a consistent copy of the live database to `destination`.
    ///
    /// # Errors
    /// Fails when the destination cannot be created or written.
    pub fn snapshot_to(&self, destination: &Path) -> AppResult<()> {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(FileSystemError::Io)?;
        }
        // A leftover from an interrupted export would otherwise be backed into,
        // and SQLite would happily reuse its pages.
        if destination.exists() {
            std::fs::remove_file(destination).map_err(FileSystemError::Io)?;
        }

        self.with_connection(|source| {
            let mut target = Connection::open(destination)
                .map_err(|_| AppError::Backup(BackupError::WriteFailed))?;
            let backup = Backup::new(source, &mut target)
                .map_err(|_| AppError::Backup(BackupError::WriteFailed))?;
            backup
                .run_to_completion(PAGES_PER_STEP, std::time::Duration::from_millis(0), None)
                .map_err(|_| AppError::Backup(BackupError::WriteFailed))?;
            Ok(())
        })
    }

    /// Replaces the contents of the live database with those of `source`.
    ///
    /// Migrations run afterwards, which is what lets a backup taken by an older
    /// build be restored into a newer one.
    ///
    /// # Errors
    /// Fails when `source` is not a readable database or when the copy or the
    /// subsequent migration fails. The live database is left untouched when the
    /// failure happens before the copy starts.
    pub fn restore_from(&self, source: &Path, now_millis: i64) -> AppResult<()> {
        let origin = open_readonly(source)?;

        let mut guard = self.lock_connection();
        let backup = Backup::new(&origin, &mut guard)
            .map_err(|source| AppError::Database(DatabaseError::Open(source)))?;
        backup
            .run_to_completion(PAGES_PER_STEP, std::time::Duration::from_millis(0), None)
            .map_err(|_| AppError::Backup(BackupError::Corrupt))?;
        drop(backup);

        // The restored file brings its own settings for anything stored in the
        // database header, and `foreign_keys` is per-connection state that the
        // copy does not carry, so both are re-asserted before anything reads.
        configure(&guard)?;
        migrations::apply(&mut guard, now_millis)?;
        Ok(())
    }
}

/// How many pages to copy per step of the backup.
///
/// At the default page size this is a couple of megabytes per step: large
/// enough that a database of any realistic size finishes in a handful of them,
/// small enough that SQLite gets to notice a writer waiting in between.
const PAGES_PER_STEP: std::os::raw::c_int = 512;

/// Reads a candidate file without touching it or the live database.
///
/// # Errors
/// Returns [`BackupError::Corrupt`] when the file is not a database of ours,
/// and a filesystem error when it cannot be read at all.
pub fn inspect(path: &Path) -> AppResult<BackupContents> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::FileSystem(FileSystemError::NotFound)
        } else {
            AppError::FileSystem(FileSystemError::Io(error))
        }
    })?;

    let connection = open_readonly(path)?;

    // Asking for our own tables is the check: SQLite will open anything with the
    // right magic bytes, including some other application's database, and
    // restoring one of those would empty the app in a way nothing warns about.
    let schema_version: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|_| AppError::Backup(BackupError::Corrupt))?;
    let note_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .map_err(|_| AppError::Backup(BackupError::Corrupt))?;
    let reminder_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM reminders", [], |row| row.get(0))
        .map_err(|_| AppError::Backup(BackupError::Corrupt))?;

    Ok(BackupContents {
        schema_version,
        note_count,
        reminder_count,
        size_bytes: metadata.len(),
        sha256: digest_of(path)?,
    })
}

/// SHA-256 of a file, read in chunks so a large backup never lands in memory.
///
/// # Errors
/// Fails when the file cannot be read.
pub fn digest_of(path: &Path) -> AppResult<String> {
    let mut file = File::open(path).map_err(FileSystemError::Io)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(FileSystemError::Io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn open_readonly(path: &Path) -> AppResult<Connection> {
    Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|_| AppError::Backup(BackupError::Corrupt))
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use crate::domain::backup::ensure_restorable;
    use crate::error::AppError;

    use super::*;

    fn tag(database: &Database, name: &str) {
        database
            .in_transaction(|transaction| {
                transaction
                    .execute(
                        "INSERT INTO tags (id, name, created_at, updated_at) VALUES (?1, ?2, 0, 0)",
                        params![uuid::Uuid::now_v7().to_string(), name],
                    )
                    .map_err(AppError::from)?;
                Ok(())
            })
            .expect("insert succeeds");
    }

    fn tag_names(database: &Database) -> Vec<String> {
        database
            .with_connection(|connection| {
                let mut statement = connection
                    .prepare("SELECT name FROM tags ORDER BY name")
                    .map_err(AppError::from)?;
                let names = statement
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(AppError::from)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(AppError::from)?;
                Ok(names)
            })
            .expect("read succeeds")
    }

    #[test]
    fn a_snapshot_can_be_restored_into_another_database() {
        let directory = tempfile::tempdir().expect("temp dir");
        let live = Database::open(&directory.path().join("live.sqlite"), 0).expect("opens");
        tag(&live, "работа");

        let backup_path = directory.path().join("backup.sqlite");
        live.snapshot_to(&backup_path).expect("snapshot succeeds");

        let other = Database::open(&directory.path().join("other.sqlite"), 0).expect("opens");
        tag(&other, "выброшенный");
        other.restore_from(&backup_path, 1).expect("restore");

        assert_eq!(
            tag_names(&other),
            ["работа"],
            "restoring replaces the contents rather than merging them"
        );
    }

    #[test]
    fn a_snapshot_taken_with_unflushed_writes_is_still_complete() {
        let directory = tempfile::tempdir().expect("temp dir");
        let live = Database::open(&directory.path().join("live.sqlite"), 0).expect("opens");
        // In WAL mode this commit lives in the log, not the main file, so a
        // plain file copy would lose it.
        tag(&live, "в журнале");

        let backup_path = directory.path().join("backup.sqlite");
        live.snapshot_to(&backup_path).expect("snapshot succeeds");

        let contents = inspect(&backup_path).expect("inspect succeeds");
        assert_eq!(contents.schema_version, migrations::latest_version());
        assert!(contents.size_bytes > 0);
        ensure_restorable(&contents, migrations::latest_version()).expect("restorable");
    }

    #[test]
    fn the_live_database_keeps_working_after_a_restore() {
        let directory = tempfile::tempdir().expect("temp dir");
        let live = Database::open(&directory.path().join("live.sqlite"), 0).expect("opens");
        tag(&live, "до");
        let backup_path = directory.path().join("backup.sqlite");
        live.snapshot_to(&backup_path).expect("snapshot succeeds");

        live.restore_from(&backup_path, 1).expect("restore");

        // The handles the app is holding must survive, which is the whole
        // reason restoring copies into the connection instead of swapping files.
        tag(&live, "после");
        assert_eq!(tag_names(&live), ["до", "после"]);
        live.verify_integrity().expect("still consistent");
    }

    #[test]
    fn a_file_that_is_not_a_database_is_refused() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("photo.jpg");
        std::fs::write(&path, b"not a database at all").expect("write");

        let error = inspect(&path).expect_err("must refuse");
        assert_eq!(error.code(), "backup_corrupt");
    }

    #[test]
    fn a_database_belonging_to_another_application_is_refused() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("someone-else.sqlite");
        let connection = Connection::open(&path).expect("opens");
        connection
            .execute_batch("CREATE TABLE messages (id INTEGER PRIMARY KEY);")
            .expect("creates");
        drop(connection);

        let error = inspect(&path).expect_err("must refuse");
        assert_eq!(error.code(), "backup_corrupt");
    }

    #[test]
    fn a_missing_file_reports_itself_as_missing_rather_than_corrupt() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = inspect(&directory.path().join("nothing.sqlite")).expect_err("must fail");
        assert_eq!(error.code(), "file_not_found");
    }

    #[test]
    fn the_digest_changes_when_the_contents_change() {
        let directory = tempfile::tempdir().expect("temp dir");
        let live = Database::open(&directory.path().join("live.sqlite"), 0).expect("opens");
        let first = directory.path().join("first.sqlite");
        live.snapshot_to(&first).expect("snapshot");

        tag(&live, "новое");
        let second = directory.path().join("second.sqlite");
        live.snapshot_to(&second).expect("snapshot");

        assert_ne!(
            digest_of(&first).expect("digest"),
            digest_of(&second).expect("digest")
        );
    }
}
