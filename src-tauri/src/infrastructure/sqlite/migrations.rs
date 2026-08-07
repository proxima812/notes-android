//! Versioned schema migrations.
//!
//! Migrations are embedded in the binary, applied in order inside a single
//! transaction each, and recorded in `schema_migrations` together with a
//! checksum. A checksum mismatch means an already-applied migration file was
//! edited after the fact, which would leave devices with silently divergent
//! schemas — so it is a hard error rather than a warning.

use rusqlite::{Connection, OptionalExtension as _};
use sha2::{Digest as _, Sha256};

use crate::error::{AppError, AppResult, DatabaseError};

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Every migration ever shipped, in ascending order. Append only; never edit an
/// entry that has already reached a device.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("../../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "drop_folders",
        sql: include_str!("../../../migrations/0002_drop_folders.sql"),
    },
];

/// Highest schema version this build understands.
#[must_use]
pub fn latest_version() -> i64 {
    MIGRATIONS.last().map_or(0, |migration| migration.version)
}

fn checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn ensure_bookkeeping_table(connection: &Connection) -> AppResult<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                 version    INTEGER PRIMARY KEY,
                 name       TEXT    NOT NULL,
                 checksum   TEXT    NOT NULL,
                 applied_at INTEGER NOT NULL
             );",
        )
        .map_err(|source| AppError::Database(DatabaseError::Migration { version: 0, source }))
}

/// Applies every migration the database has not seen yet.
///
/// # Errors
/// Fails when a migration statement fails, when a previously applied migration
/// no longer matches its recorded checksum, or when the database was created by
/// a newer build than this one.
pub fn apply(connection: &mut Connection, now_millis: i64) -> AppResult<i64> {
    ensure_bookkeeping_table(connection)?;

    let current: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(AppError::from)?;

    let supported = latest_version();
    if current > supported {
        return Err(AppError::Database(DatabaseError::SchemaTooNew {
            found: current,
            supported,
        }));
    }

    for migration in MIGRATIONS {
        let expected = checksum(migration.sql);

        if migration.version <= current {
            let recorded: Option<String> = connection
                .query_row(
                    "SELECT checksum FROM schema_migrations WHERE version = ?1",
                    [migration.version],
                    |row| row.get(0),
                )
                .optional()
                .map_err(AppError::from)?;

            if let Some(recorded) = recorded {
                if recorded != expected {
                    tracing::error!(
                        version = migration.version,
                        "migration file changed after it was applied"
                    );
                    return Err(AppError::Database(DatabaseError::Migration {
                        version: migration.version,
                        source: rusqlite::Error::InvalidQuery,
                    }));
                }
            }
            continue;
        }

        tracing::info!(
            version = migration.version,
            name = migration.name,
            "applying migration"
        );

        let transaction = connection.transaction().map_err(AppError::from)?;
        transaction.execute_batch(migration.sql).map_err(|source| {
            AppError::Database(DatabaseError::Migration {
                version: migration.version,
                source,
            })
        })?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![migration.version, migration.name, expected, now_millis],
            )
            .map_err(|source| {
                AppError::Database(DatabaseError::Migration {
                    version: migration.version,
                    source,
                })
            })?;
        transaction.commit().map_err(AppError::from)?;
    }

    Ok(latest_version())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::sqlite::connection::configure;

    fn fresh() -> Connection {
        let mut connection = Connection::open_in_memory().expect("in-memory database opens");
        configure(&connection).expect("pragmas apply");
        apply(&mut connection, 1_700_000_000_000).expect("migrations apply");
        connection
    }

    fn table_exists(connection: &Connection, name: &str) -> bool {
        connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE name = ?1",
                [name],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .expect("query runs")
            .is_some()
    }

    #[test]
    fn every_required_table_exists_after_migrating() {
        let connection = fresh();
        for name in [
            "notes",
            "note_blocks",
            "tasks",
            "reminders",
            "reminder_occurrences",
            "tags",
            "note_tags",
            "attachments",
            "note_links",
            "saved_searches",
            "templates",
            "routines",
            "notification_events",
            "activity_history",
            "app_settings",
            "backup_history",
            "schema_migrations",
            "notes_fts",
            "tasks_fts",
            "attachments_fts",
        ] {
            assert!(table_exists(&connection, name), "missing table: {name}");
        }
    }

    #[test]
    fn the_folder_tables_are_gone() {
        // Dropped by 0002. Asserted because the initial migration still creates
        // them: a database that never ran 0002 would keep working with folders
        // no code reads any more.
        let connection = fresh();
        for name in ["folders", "note_folders"] {
            assert!(!table_exists(&connection, name), "table still here: {name}");
        }

        let has_column = connection
            .prepare("SELECT folder_id FROM tasks")
            .map(drop)
            .is_ok();
        assert!(!has_column, "tasks.folder_id still here");
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let mut connection = fresh();
        let version = apply(&mut connection, 1_700_000_001_000).expect("second run succeeds");
        assert_eq!(version, latest_version());

        let applied: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("count runs");
        assert_eq!(applied, MIGRATIONS.len() as i64);
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused() {
        let mut connection = fresh();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (9999, 'from-the-future', 'x', 0)",
                [],
            )
            .expect("insert runs");

        let error = apply(&mut connection, 1_700_000_002_000)
            .expect_err("a newer schema must not be downgraded silently");
        assert_eq!(error.code(), "database_schema_too_new");
    }

    #[test]
    fn an_edited_migration_file_is_detected() {
        let mut connection = fresh();
        connection
            .execute(
                "UPDATE schema_migrations SET checksum = 'tampered' WHERE version = 1",
                [],
            )
            .expect("update runs");

        let error = apply(&mut connection, 1_700_000_003_000)
            .expect_err("a changed checksum must stop the app");
        assert_eq!(error.code(), "database_migration_failed");
    }

    #[test]
    fn fts5_is_available_in_the_bundled_sqlite() {
        // If the bundled build ever loses FTS5 the whole search feature is gone,
        // so this is worth asserting rather than discovering on a device.
        let connection = fresh();
        connection
            .execute(
                "INSERT INTO notes_fts (note_id, title, body, tags)
                 VALUES ('n', 'Покупки', 'молоко и хлеб', '')",
                [],
            )
            .expect("fts insert works");

        let found: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH 'молоко'",
                [],
                |row| row.get(0),
            )
            .expect("fts match works");
        assert_eq!(found, 1);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let connection = fresh();
        let result = connection.execute(
            "INSERT INTO note_tags (note_id, tag_id, created_at) VALUES ('missing', 'gone', 0)",
            [],
        );
        assert!(result.is_err(), "a dangling foreign key must be rejected");
    }
}
