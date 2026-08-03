//! Database handle and connection configuration.

use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::Path;

use crate::error::{AppError, AppResult, DatabaseError};

use super::migrations;

/// Applies the pragmas every connection needs.
///
/// `foreign_keys` is per-connection in SQLite and off by default, so forgetting
/// it here would silently disable every `ON DELETE CASCADE` in the schema.
///
/// # Errors
/// Fails when the database rejects a pragma, which in practice means the file
/// is not a database or the disk is read-only.
pub fn configure(connection: &Connection) -> AppResult<()> {
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA temp_store = MEMORY;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|source| AppError::Database(DatabaseError::Open(source)))
}

/// Owns the single write connection to the local database.
///
/// SQLite handles concurrent readers well but serialises writers anyway; one
/// mutex-guarded connection is simpler than a pool and removes a whole class of
/// "database is locked" failures. Long operations (backup, import) run on a
/// blocking task so the UI thread never waits on this lock.
pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    /// Opens the database at `path`, creating and migrating it if needed.
    ///
    /// # Errors
    /// Fails when the file cannot be opened or a migration fails.
    pub fn open(path: &Path, now_millis: i64) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut connection = Connection::open(path)
            .map_err(|source| AppError::Database(DatabaseError::Open(source)))?;
        configure(&connection)?;
        migrations::apply(&mut connection, now_millis)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Opens a private in-memory database. Used by tests.
    ///
    /// # Errors
    /// Fails when a migration fails.
    pub fn open_in_memory(now_millis: i64) -> AppResult<Self> {
        let mut connection = Connection::open_in_memory()
            .map_err(|source| AppError::Database(DatabaseError::Open(source)))?;
        configure(&connection)?;
        migrations::apply(&mut connection, now_millis)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Runs `body` with exclusive access to the connection.
    ///
    /// # Errors
    /// Propagates whatever `body` returns.
    pub fn with_connection<T, F>(&self, body: F) -> AppResult<T>
    where
        F: FnOnce(&Connection) -> AppResult<T>,
    {
        let guard = self.connection.lock();
        body(&guard)
    }

    /// Runs `body` inside a transaction, committing on `Ok` and rolling back on
    /// `Err`. Multi-statement writes go through here so a failure halfway
    /// cannot leave a note without its tags or an occurrence without its alarm.
    ///
    /// # Errors
    /// Propagates whatever `body` returns, and any failure to commit.
    pub fn in_transaction<T, F>(&self, body: F) -> AppResult<T>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> AppResult<T>,
    {
        let mut guard = self.connection.lock();
        let transaction = guard.transaction().map_err(AppError::from)?;
        let value = body(&transaction)?;
        transaction.commit().map_err(AppError::from)?;
        Ok(value)
    }

    /// Exclusive access to the connection itself.
    ///
    /// Only restoring a backup needs this: it writes through SQLite's backup API
    /// rather than through SQL, so it cannot borrow the connection the way
    /// [`Self::with_connection`] hands it out.
    pub(super) fn lock_connection(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.connection.lock()
    }

    /// Runs SQLite's own consistency check.
    ///
    /// # Errors
    /// Returns [`DatabaseError::Corrupt`] when the check does not return `ok`.
    pub fn verify_integrity(&self) -> AppResult<()> {
        self.with_connection(|connection| {
            let result: String = connection
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .map_err(AppError::from)?;
            if result == "ok" {
                Ok(())
            } else {
                tracing::error!("integrity check failed");
                Err(AppError::Database(DatabaseError::Corrupt))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_in_memory_database_is_migrated_and_consistent() {
        let database = Database::open_in_memory(0).expect("opens");
        database
            .verify_integrity()
            .expect("a fresh database is consistent");
    }

    #[test]
    fn foreign_keys_are_on_for_the_connection() {
        let database = Database::open_in_memory(0).expect("opens");
        let enabled: i64 = database
            .with_connection(|connection| {
                connection
                    .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
                    .map_err(AppError::from)
            })
            .expect("pragma readable");
        assert_eq!(enabled, 1, "cascades depend on this being on");
    }

    #[test]
    fn a_failed_transaction_leaves_nothing_behind() {
        let database = Database::open_in_memory(0).expect("opens");

        let outcome: AppResult<()> = database.in_transaction(|transaction| {
            transaction
                .execute(
                    "INSERT INTO tags (id, name, created_at, updated_at) VALUES (?1, ?2, 0, 0)",
                    rusqlite::params!["11111111-1111-7111-8111-111111111111", "работа"],
                )
                .map_err(AppError::from)?;
            Err(AppError::Database(DatabaseError::Busy))
        });
        assert!(outcome.is_err());

        let count: i64 = database
            .with_connection(|connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
                    .map_err(AppError::from)
            })
            .expect("count runs");
        assert_eq!(count, 0, "the rolled-back insert must not survive");
    }

    #[test]
    fn a_committed_transaction_persists() {
        let database = Database::open_in_memory(0).expect("opens");

        database
            .in_transaction(|transaction| {
                transaction
                    .execute(
                        "INSERT INTO tags (id, name, created_at, updated_at) VALUES (?1, ?2, 0, 0)",
                        rusqlite::params!["11111111-1111-7111-8111-111111111111", "работа"],
                    )
                    .map_err(AppError::from)?;
                Ok(())
            })
            .expect("commit succeeds");

        let count: i64 = database
            .with_connection(|connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
                    .map_err(AppError::from)
            })
            .expect("count runs");
        assert_eq!(count, 1);
    }

    #[test]
    fn a_file_backed_database_survives_reopening() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("nested").join("organizer.sqlite");

        {
            let database = Database::open(&path, 0).expect("creates the file and parent directory");
            database
                .in_transaction(|transaction| {
                    transaction
                        .execute(
                            "INSERT INTO tags (id, name, created_at, updated_at) VALUES (?1, ?2, 0, 0)",
                            rusqlite::params!["11111111-1111-7111-8111-111111111111", "работа"],
                        )
                        .map_err(AppError::from)?;
                    Ok(())
                })
                .expect("write succeeds");
        }

        let reopened = Database::open(&path, 1).expect("reopens without re-migrating");
        let count: i64 = reopened
            .with_connection(|connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
                    .map_err(AppError::from)
            })
            .expect("count runs");
        assert_eq!(count, 1, "data must outlive the process");
    }
}
