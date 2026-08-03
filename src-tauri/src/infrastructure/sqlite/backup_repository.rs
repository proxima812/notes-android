//! The database as something that can be copied, and the log of copies made.

use std::path::Path;
use std::sync::Arc;

use rusqlite::{params, OptionalExtension as _, Row};

use crate::domain::backup::{BackupArchive, BackupContents, BackupRecord, BackupRepository};
use crate::domain::clock::{SharedClock, Timestamp};
use crate::domain::ids::BackupRecordId;
use crate::error::{AppError, AppResult};

use super::{backup, Database};

pub struct SqliteBackupArchive {
    database: Arc<Database>,
    clock: SharedClock,
}

impl SqliteBackupArchive {
    #[must_use]
    pub fn new(database: Arc<Database>, clock: SharedClock) -> Self {
        Self { database, clock }
    }
}

impl BackupArchive for SqliteBackupArchive {
    fn snapshot_to(&self, destination: &Path) -> AppResult<()> {
        self.database.snapshot_to(destination)
    }

    fn inspect(&self, path: &Path) -> AppResult<BackupContents> {
        backup::inspect(path)
    }

    fn restore_from(&self, source: &Path) -> AppResult<()> {
        self.database
            .restore_from(source, self.clock.now().as_millis())
    }
}

pub struct SqliteBackupRepository {
    database: Arc<Database>,
}

impl SqliteBackupRepository {
    #[must_use]
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }
}

fn map_record(row: &Row<'_>) -> rusqlite::Result<BackupRecord> {
    Ok(BackupRecord {
        location: row.get(0)?,
        file_name: row.get(1)?,
        size_bytes: row.get::<_, i64>(2)?.unsigned_abs(),
        sha256: row.get(3)?,
        note_count: row.get(4)?,
        created_at: Timestamp::from_millis(row.get(5)?),
    })
}

impl BackupRepository for SqliteBackupRepository {
    fn record(&self, entry: &BackupRecord) -> AppResult<()> {
        self.database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO backup_history (
                        id, location, file_name, size_bytes, sha256,
                        note_count, status, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'completed', ?7)",
                    params![
                        BackupRecordId::new(),
                        entry.location,
                        entry.file_name,
                        i64::try_from(entry.size_bytes).unwrap_or(i64::MAX),
                        entry.sha256,
                        entry.note_count,
                        entry.created_at.as_millis(),
                    ],
                )
                .map(|_| ())
                .map_err(AppError::from)
        })
    }

    fn latest(&self) -> AppResult<Option<BackupRecord>> {
        self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT location, file_name, size_bytes, sha256, note_count, created_at
                       FROM backup_history
                      WHERE status = 'completed'
                      ORDER BY created_at DESC
                      LIMIT 1",
                    [],
                    map_record,
                )
                .optional()
                .map_err(AppError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::clock::{FixedClock, SharedClock};

    use super::*;

    fn fixture() -> (SqliteBackupRepository, Arc<Database>) {
        let database = Arc::new(Database::open_in_memory(0).expect("opens"));
        (SqliteBackupRepository::new(Arc::clone(&database)), database)
    }

    fn record(name: &str, at: i64) -> BackupRecord {
        BackupRecord {
            location: "Загрузки".into(),
            file_name: name.into(),
            size_bytes: 4096,
            sha256: "abc".into(),
            note_count: 7,
            created_at: Timestamp::from_millis(at),
        }
    }

    #[test]
    fn there_is_no_latest_backup_before_one_is_made() {
        let (repository, _database) = fixture();
        assert_eq!(repository.latest().expect("reads"), None);
    }

    #[test]
    fn the_latest_backup_is_the_most_recent_one() {
        let (repository, _database) = fixture();
        repository
            .record(&record("старый.sqlite", 1_000))
            .expect("records");
        repository
            .record(&record("новый.sqlite", 2_000))
            .expect("records");

        let latest = repository.latest().expect("reads").expect("one exists");
        assert_eq!(latest.file_name, "новый.sqlite");
        assert_eq!(latest.note_count, 7);
        assert_eq!(latest.size_bytes, 4096);
    }

    #[test]
    fn the_archive_round_trips_through_the_domain_port() {
        let directory = tempfile::tempdir().expect("temp dir");
        let database =
            Arc::new(Database::open(&directory.path().join("live.sqlite"), 0).expect("opens"));
        let clock: SharedClock = Arc::new(FixedClock::new(Timestamp::from_millis(1_000)));
        let archive = SqliteBackupArchive::new(Arc::clone(&database), clock);

        let path = directory.path().join("copy.sqlite");
        archive.snapshot_to(&path).expect("snapshot");
        let contents = archive.inspect(&path).expect("inspect");
        assert!(contents.size_bytes > 0);

        archive.restore_from(&path).expect("restore");
        database.verify_integrity().expect("still consistent");
    }
}
