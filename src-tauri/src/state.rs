//! Application state.
//!
//! Built once at startup and handed to commands by Tauri. Everything inside is
//! shared behind `Arc` and internally synchronised; there is no global mutable
//! state and no `static mut` anywhere in the core.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::application::backup::BackupUseCases;
use crate::application::use_cases::{NoteUseCases, ReminderUseCases, SearchUseCases};
use crate::domain::clock::{SharedClock, SystemClock};
use crate::error::AppResult;
use crate::infrastructure::sqlite::{
    Database, SqliteBackupArchive, SqliteBackupRepository, SqliteNoteRepository,
    SqliteReminderRepository, SqliteSearchRepository,
};
use crate::platform::{AlarmClock, DocumentStore};

/// File name of the database inside the app's private directory.
pub const DATABASE_FILE: &str = "organizer.sqlite";

pub struct AppState {
    pub notes: Arc<NoteUseCases>,
    pub reminders: Arc<ReminderUseCases>,
    pub search: Arc<SearchUseCases>,
    pub backup: Arc<BackupUseCases>,
    pub database: Arc<Database>,
    pub clock: SharedClock,
}

impl AppState {
    /// Opens the database under `data_dir` and wires the object graph.
    ///
    /// # Errors
    /// Fails when the database cannot be opened or migrated.
    pub fn bootstrap(
        data_dir: &Path,
        staging_dir: PathBuf,
        alarms: Arc<dyn AlarmClock>,
        documents: Arc<dyn DocumentStore>,
    ) -> AppResult<Self> {
        let clock: SharedClock = Arc::new(SystemClock);
        Self::with_services(data_dir, staging_dir, clock, alarms, documents)
    }

    /// Same as [`Self::bootstrap`] but with injected platform services, for tests.
    ///
    /// # Errors
    /// Fails when the database cannot be opened or migrated.
    pub fn with_services(
        data_dir: &Path,
        staging_dir: PathBuf,
        clock: SharedClock,
        alarms: Arc<dyn AlarmClock>,
        documents: Arc<dyn DocumentStore>,
    ) -> AppResult<Self> {
        let path = data_dir.join(DATABASE_FILE);
        tracing::info!("opening the local database");

        let database = Arc::new(Database::open(&path, clock.now().as_millis())?);

        let note_repository = Arc::new(SqliteNoteRepository::new(
            Arc::clone(&database),
            Arc::clone(&clock),
        ));
        let search_repository = Arc::new(SqliteSearchRepository::new(
            Arc::clone(&database),
            Arc::clone(&clock),
        ));
        let reminder_repository = Arc::new(SqliteReminderRepository::new(
            Arc::clone(&database),
            Arc::clone(&clock),
        ));

        let backup = Arc::new(BackupUseCases::new(
            Arc::new(SqliteBackupArchive::new(
                Arc::clone(&database),
                Arc::clone(&clock),
            )),
            Arc::new(SqliteBackupRepository::new(Arc::clone(&database))),
            Arc::clone(&reminder_repository)
                as Arc<dyn crate::domain::reminders::ReminderRepository>,
            Arc::clone(&alarms),
            documents,
            Arc::clone(&clock),
            staging_dir,
        ));

        Ok(Self {
            backup,
            notes: Arc::new(NoteUseCases::new(note_repository)),
            reminders: Arc::new(ReminderUseCases::new(
                reminder_repository,
                alarms,
                Arc::clone(&clock),
            )),
            search: Arc::new(SearchUseCases::new(search_repository)),
            database,
            clock,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::dto::ListNotesRequest;
    use crate::domain::clock::{FixedClock, Timestamp};
    use crate::domain::notes::NoteDraft;
    use crate::platform::{Alarm, AlarmClock, AlarmPermissions, DocumentStore, PickedDocument};

    struct FakeDocumentStore;

    impl DocumentStore for FakeDocumentStore {
        fn export(&self, _source: &str, _name: &str, _mime: &str) -> AppResult<PickedDocument> {
            Ok(PickedDocument::default())
        }

        fn import(&self, _destination: &str, _mime: &str) -> AppResult<PickedDocument> {
            Ok(PickedDocument::default())
        }
    }

    fn fake_documents() -> Arc<dyn DocumentStore> {
        Arc::new(FakeDocumentStore)
    }

    struct FakeAlarmClock;

    impl AlarmClock for FakeAlarmClock {
        fn schedule(&self, _alarm: &Alarm) -> AppResult<bool> {
            Ok(true)
        }

        fn cancel(&self, _request_code: i32) -> AppResult<()> {
            Ok(())
        }

        fn cancel_all(&self) -> AppResult<()> {
            Ok(())
        }

        fn take_launch_target(&self) -> AppResult<Option<String>> {
            Ok(None)
        }

        fn permissions(&self) -> AppResult<AlarmPermissions> {
            Ok(AlarmPermissions {
                notifications_granted: true,
                exact_allowed: true,
            })
        }

        fn request_notification_permission(&self) -> AppResult<bool> {
            Ok(true)
        }
    }

    fn fake_alarms() -> Arc<dyn AlarmClock> {
        Arc::new(FakeAlarmClock)
    }

    #[test]
    fn bootstrapping_wires_the_reminder_catalog() {
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock = Arc::new(FixedClock::new(Timestamp::from_millis(1_000)));

        let state = AppState::with_services(
            directory.path(),
            directory.path().join("staging"),
            clock,
            fake_alarms(),
            fake_documents(),
        )
        .expect("bootstraps reminders");

        let catalog = state.reminders.sound_catalog().expect("reads catalog");
        assert_eq!(catalog.default_sound_id, "death_and_rebirth");
    }

    #[test]
    fn bootstrapping_creates_a_usable_database_in_the_given_directory() {
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock =
            Arc::new(FixedClock::new(Timestamp::from_millis(1_700_000_000_000)));

        let state = AppState::with_services(
            directory.path(),
            directory.path().join("staging"),
            clock,
            fake_alarms(),
            fake_documents(),
        )
        .expect("bootstraps");
        assert!(directory.path().join(DATABASE_FILE).exists());

        state
            .notes
            .create(NoteDraft {
                title: Some("Первая".to_owned()),
                ..NoteDraft::default()
            })
            .expect("creates");
        assert_eq!(
            state
                .notes
                .count(&ListNotesRequest::default())
                .expect("counts"),
            1
        );
    }

    #[test]
    fn data_survives_a_restart_of_the_process() {
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock =
            Arc::new(FixedClock::new(Timestamp::from_millis(1_700_000_000_000)));

        {
            let state = AppState::with_services(
                directory.path(),
                directory.path().join("staging"),
                Arc::clone(&clock),
                fake_alarms(),
                fake_documents(),
            )
            .expect("bootstraps");
            state
                .notes
                .create(NoteDraft {
                    title: Some("Переживёт перезапуск".to_owned()),
                    ..NoteDraft::default()
                })
                .expect("creates");
        }

        let restarted = AppState::with_services(
            directory.path(),
            directory.path().join("staging"),
            clock,
            fake_alarms(),
            fake_documents(),
        )
        .expect("bootstraps again");
        let page = restarted
            .notes
            .list(&ListNotesRequest::default())
            .expect("lists");
        assert_eq!(page.total, 1);
        assert_eq!(
            page.items.first().map(|note| note.title.as_str()),
            Some("Переживёт перезапуск")
        );
    }
}
