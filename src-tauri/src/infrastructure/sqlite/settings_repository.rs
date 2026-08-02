//! The key-value corner of the database.

use std::sync::Arc;

use rusqlite::{params, OptionalExtension as _};

use crate::domain::clock::SharedClock;
use crate::domain::settings::SettingsRepository;
use crate::error::{AppError, AppResult};

use super::Database;

pub struct SqliteSettingsRepository {
    database: Arc<Database>,
    clock: SharedClock,
}

impl SqliteSettingsRepository {
    #[must_use]
    pub fn new(database: Arc<Database>, clock: SharedClock) -> Self {
        Self { database, clock }
    }
}

impl SettingsRepository for SqliteSettingsRepository {
    fn read(&self, key: &str) -> AppResult<Option<String>> {
        self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    [key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(AppError::from)
        })
    }

    fn write(&self, key: &str, value: &str) -> AppResult<()> {
        let now = self.clock.now();
        self.database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO app_settings (key, value, updated_at)
                          VALUES (?1, ?2, ?3)
                     ON CONFLICT (key) DO UPDATE
                            SET value = excluded.value,
                                updated_at = excluded.updated_at",
                    params![key, value, now.as_millis()],
                )
                .map(|_| ())
                .map_err(AppError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::clock::{FixedClock, Timestamp};

    use super::*;

    fn fixture() -> SqliteSettingsRepository {
        let database = Arc::new(Database::open_in_memory(0).expect("opens"));
        let clock: SharedClock = Arc::new(FixedClock::new(Timestamp::from_millis(1_000)));
        SqliteSettingsRepository::new(database, clock)
    }

    #[test]
    fn a_setting_that_was_never_written_reads_as_absent() {
        assert_eq!(fixture().read("appearance.app_icon").expect("reads"), None);
    }

    #[test]
    fn a_setting_survives_being_written_and_read_back() {
        let repository = fixture();
        repository
            .write("appearance.app_icon", "neon")
            .expect("writes");
        assert_eq!(
            repository.read("appearance.app_icon").expect("reads"),
            Some("neon".to_owned())
        );
    }

    #[test]
    fn writing_again_replaces_rather_than_failing_on_the_key() {
        let repository = fixture();
        repository
            .write("appearance.app_icon", "neon")
            .expect("writes");
        repository
            .write("appearance.app_icon", "paper")
            .expect("writes");
        assert_eq!(
            repository.read("appearance.app_icon").expect("reads"),
            Some("paper".to_owned())
        );
    }
}
