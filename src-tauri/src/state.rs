//! Application state.
//!
//! Built once at startup and handed to commands by Tauri. Everything inside is
//! shared behind `Arc` and internally synchronised; there is no global mutable
//! state and no `static mut` anywhere in the core.

use std::path::Path;
use std::sync::Arc;

use crate::application::use_cases::{NoteUseCases, SearchUseCases};
use crate::domain::clock::{SharedClock, SystemClock};
use crate::error::AppResult;
use crate::infrastructure::sqlite::{Database, SqliteNoteRepository, SqliteSearchRepository};

/// File name of the database inside the app's private directory.
pub const DATABASE_FILE: &str = "organizer.sqlite";

pub struct AppState {
    pub notes: Arc<NoteUseCases>,
    pub search: Arc<SearchUseCases>,
    pub database: Arc<Database>,
    pub clock: SharedClock,
}

impl AppState {
    /// Opens the database under `data_dir` and wires the object graph.
    ///
    /// # Errors
    /// Fails when the database cannot be opened or migrated.
    pub fn bootstrap(data_dir: &Path) -> AppResult<Self> {
        let clock: SharedClock = Arc::new(SystemClock);
        Self::with_clock(data_dir, clock)
    }

    /// Same as [`Self::bootstrap`] but with an injected clock, for tests.
    ///
    /// # Errors
    /// Fails when the database cannot be opened or migrated.
    pub fn with_clock(data_dir: &Path, clock: SharedClock) -> AppResult<Self> {
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

        Ok(Self {
            notes: Arc::new(NoteUseCases::new(note_repository)),
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

    #[test]
    fn bootstrapping_creates_a_usable_database_in_the_given_directory() {
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock =
            Arc::new(FixedClock::new(Timestamp::from_millis(1_700_000_000_000)));

        let state = AppState::with_clock(directory.path(), clock).expect("bootstraps");
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
            let state =
                AppState::with_clock(directory.path(), Arc::clone(&clock)).expect("bootstraps");
            state
                .notes
                .create(NoteDraft {
                    title: Some("Переживёт перезапуск".to_owned()),
                    ..NoteDraft::default()
                })
                .expect("creates");
        }

        let restarted = AppState::with_clock(directory.path(), clock).expect("bootstraps again");
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
