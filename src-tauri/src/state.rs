//! Application state.
//!
//! Built once at startup and handed to commands by Tauri. Everything inside is
//! shared behind `Arc` and internally synchronised; there is no global mutable
//! state and no `static mut` anywhere in the core.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::application::app_icons::AppIconUseCases;
use crate::application::backup::BackupUseCases;
use crate::application::organisation::OrganisationUseCases;
use crate::application::quick_notes::QuickNoteUseCases;
use crate::application::tasks::TaskUseCases;
use crate::application::use_cases::{NoteUseCases, ReminderUseCases, SearchUseCases};
use crate::domain::clock::{SharedClock, SystemClock};
use crate::error::AppResult;
use crate::infrastructure::sqlite::{
    Database, SqliteBackupArchive, SqliteBackupRepository, SqliteNoteRepository,
    SqliteOrganisationRepository, SqliteReminderRepository, SqliteSearchRepository,
    SqliteSettingsRepository, SqliteTaskRepository,
};
use crate::platform::{AlarmClock, AppIconSwitch, DocumentStore};

/// File name of the database inside the app's private directory.
pub const DATABASE_FILE: &str = "organizer.sqlite";

pub struct AppState {
    pub notes: Arc<NoteUseCases>,
    pub reminders: Arc<ReminderUseCases>,
    pub search: Arc<SearchUseCases>,
    pub backup: Arc<BackupUseCases>,
    pub app_icons: Arc<AppIconUseCases>,
    pub organisation: Arc<OrganisationUseCases>,
    pub quick_notes: Arc<QuickNoteUseCases>,
    pub tasks: Arc<TaskUseCases>,
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
        icons: Arc<dyn AppIconSwitch>,
    ) -> AppResult<Self> {
        let clock: SharedClock = Arc::new(SystemClock);
        Self::with_services(data_dir, staging_dir, clock, alarms, documents, icons)
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
        icons: Arc<dyn AppIconSwitch>,
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

        let settings = Arc::new(SqliteSettingsRepository::new(
            Arc::clone(&database),
            Arc::clone(&clock),
        ));

        // Dictating a note leans on the two use cases above it rather than
        // reaching for the repositories itself: a quick note is an ordinary
        // note and an ordinary reminder, made in one press.
        let notes = Arc::new(NoteUseCases::new(note_repository, Arc::clone(&clock)));
        let reminders = Arc::new(ReminderUseCases::new(
            reminder_repository,
            alarms,
            Arc::clone(&clock),
        ));

        Ok(Self {
            quick_notes: Arc::new(QuickNoteUseCases::new(
                Arc::clone(&notes),
                Arc::clone(&reminders),
                Arc::clone(&settings) as Arc<dyn crate::domain::settings::SettingsRepository>,
                Arc::clone(&clock),
            )),
            tasks: Arc::new(TaskUseCases::new(
                Arc::new(SqliteTaskRepository::new(Arc::clone(&database))),
                Arc::clone(&clock),
            )),
            organisation: Arc::new(OrganisationUseCases::new(
                Arc::new(SqliteOrganisationRepository::new(Arc::clone(&database))),
                Arc::clone(&clock),
            )),
            app_icons: Arc::new(AppIconUseCases::new(icons, settings)),
            backup,
            notes,
            reminders,
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

    struct FakeIconSwitch;

    impl AppIconSwitch for FakeIconSwitch {
        fn select(&self, _alias: &str, _known: &[String], _fallback: &str) -> AppResult<()> {
            Ok(())
        }

        fn current(&self, _known: &[String], _fallback: &str) -> AppResult<Option<String>> {
            Ok(None)
        }
    }

    fn fake_icons() -> Arc<dyn AppIconSwitch> {
        Arc::new(FakeIconSwitch)
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
            fake_icons(),
        )
        .expect("bootstraps reminders");

        let catalog = state.reminders.sound_catalog().expect("reads catalog");
        assert_eq!(catalog.default_sound_id, "death_and_rebirth");
    }

    /// The three lines every dictation test repeats.
    fn dictating(hour: u32, minute: u32) -> (tempfile::TempDir, AppState) {
        let zone = chrono_tz::Europe::Moscow;
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock = Arc::new(FixedClock::at_local(zone, 2026, 8, 10, hour, minute));
        let state = AppState::with_services(
            directory.path(),
            directory.path().join("staging"),
            clock,
            fake_alarms(),
            fake_documents(),
            fake_icons(),
        )
        .expect("bootstraps");
        (directory, state)
    }

    /// When the alarm was actually set for, as the local clock reads it.
    fn armed_at(outcome: &crate::application::quick_notes::QuickNoteOutcome) -> String {
        outcome
            .reminder
            .as_ref()
            .expect("an alarm was armed")
            .scheduled
            .reminder
            .scheduled_at
            .to_zoned(chrono_tz::Europe::Moscow)
            .expect("representable")
            .format("%Y-%m-%d %H:%M")
            .to_string()
    }

    /// The lead is the whole point of the feature, so the arithmetic is pinned
    /// through the real object graph:
    /// «встреча. 15:00» said at noon becomes a note called «Встреча» and an
    /// alarm at 14:30, which is the shipped half-hour lead subtracted from the
    /// time that was said.
    #[test]
    fn a_dictated_phrase_becomes_a_note_and_an_alarm_before_the_time_it_named() {
        let zone = chrono_tz::Europe::Moscow;
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock = Arc::new(FixedClock::at_local(zone, 2026, 8, 10, 12, 0));

        let state = AppState::with_services(
            directory.path(),
            directory.path().join("staging"),
            clock,
            fake_alarms(),
            fake_documents(),
            fake_icons(),
        )
        .expect("bootstraps");

        let outcome = state
            .quick_notes
            .create("встреча. 15:00", "Europe/Moscow")
            .expect("creates");

        assert_eq!(outcome.note.title, "Встреча");
        let reminder = outcome.reminder.expect("an alarm was armed");
        assert_eq!(
            reminder
                .scheduled
                .reminder
                .scheduled_at
                .to_zoned(zone)
                .expect("representable")
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            "2026-08-10 14:30"
        );
    }

    /// A phrase with no time in it is a quick note all the same: it lands on
    /// the fallback hour, with nothing subtracted, because there is no named
    /// event to be early for.
    #[test]
    fn a_phrase_with_no_time_lands_on_the_fallback_hour() {
        let zone = chrono_tz::Europe::Moscow;
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock = Arc::new(FixedClock::at_local(zone, 2026, 8, 10, 12, 0));

        let state = AppState::with_services(
            directory.path(),
            directory.path().join("staging"),
            clock,
            fake_alarms(),
            fake_documents(),
            fake_icons(),
        )
        .expect("bootstraps");

        let outcome = state
            .quick_notes
            .create("купить молоко", "Europe/Moscow")
            .expect("creates");

        assert_eq!(outcome.note.title, "Купить молоко");
        let reminder = outcome.reminder.expect("an alarm was armed");
        assert_eq!(
            reminder
                .scheduled
                .reminder
                .scheduled_at
                .to_zoned(zone)
                .expect("representable")
                .format("%H:%M")
                .to_string(),
            "19:00"
        );
    }

    /// The day is the user's too: someone who dictates errands for the next
    /// morning rather than for tonight says so once, in Settings.
    #[test]
    fn a_saved_day_is_the_one_a_phrase_without_one_lands_on() {
        let zone = chrono_tz::Europe::Moscow;
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock = Arc::new(FixedClock::at_local(zone, 2026, 8, 10, 12, 0));

        let state = AppState::with_services(
            directory.path(),
            directory.path().join("staging"),
            clock,
            fake_alarms(),
            fake_documents(),
            fake_icons(),
        )
        .expect("bootstraps");

        state
            .quick_notes
            .save_settings(30, "19:00", 1)
            .expect("saves settings");

        let outcome = state
            .quick_notes
            .create("купить молоко", "Europe/Moscow")
            .expect("creates");

        let reminder = outcome.reminder.expect("an alarm was armed");
        assert_eq!(
            reminder
                .scheduled
                .reminder
                .scheduled_at
                .to_zoned(zone)
                .expect("representable")
                .format("%d %H:%M")
                .to_string(),
            "11 19:00",
            "tomorrow evening, not tonight, which is where the same hour would \
             have landed with the day left at today"
        );
    }

    /// Settings are the user's, so the lead the reminder uses is the one they
    /// last saved rather than the one the app shipped with.
    #[test]
    fn a_saved_lead_is_the_one_the_next_dictation_uses() {
        let zone = chrono_tz::Europe::Moscow;
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock = Arc::new(FixedClock::at_local(zone, 2026, 8, 10, 12, 0));

        let state = AppState::with_services(
            directory.path(),
            directory.path().join("staging"),
            clock,
            fake_alarms(),
            fake_documents(),
            fake_icons(),
        )
        .expect("bootstraps");

        state
            .quick_notes
            .save_settings(5, "08:00", 0)
            .expect("saves settings");

        let outcome = state
            .quick_notes
            .create("встреча. 15:00", "Europe/Moscow")
            .expect("creates");

        let reminder = outcome.reminder.expect("an alarm was armed");
        assert_eq!(
            reminder
                .scheduled
                .reminder
                .scheduled_at
                .to_zoned(zone)
                .expect("representable")
                .format("%H:%M")
                .to_string(),
            "14:55"
        );
    }

    /// The words survive even when the alarm does not: a time that has already
    /// gone leaves the note in the list with the failure reported beside it.
    #[test]
    fn a_time_that_has_gone_still_leaves_the_note_behind() {
        let zone = chrono_tz::Europe::Moscow;
        let directory = tempfile::tempdir().expect("temp dir");
        let clock: SharedClock = Arc::new(FixedClock::at_local(zone, 2026, 8, 10, 12, 0));

        let state = AppState::with_services(
            directory.path(),
            directory.path().join("staging"),
            clock,
            fake_alarms(),
            fake_documents(),
            fake_icons(),
        )
        .expect("bootstraps");

        let outcome = state
            .quick_notes
            .create("сегодня в 9:00 зарядка", "Europe/Moscow")
            .expect("the note is still made");

        assert_eq!(outcome.note.title, "Зарядка");
        assert!(outcome.reminder.is_none());
        assert_eq!(
            outcome
                .reminder_error
                .as_ref()
                .map(crate::error::AppError::code),
            Some("validation_time_in_past")
        );
    }

    /// The branch that decides whether a near-term dictation rings at all.
    ///
    /// «встреча в 15:00» said at 14:45 asks for an alarm at 14:15, which has
    /// gone. Refusing would be pedantic — the meeting is real and still ahead —
    /// so it rings as soon as the alarm layer will take it.
    #[test]
    fn a_lead_that_would_land_in_the_past_rings_as_soon_as_it_can() {
        let (_directory, state) = dictating(14, 45);

        let outcome = state
            .quick_notes
            .create("встреча в 15:00", "Europe/Moscow")
            .expect("creates");

        assert_eq!(armed_at(&outcome), "2026-08-10 14:46");
        assert_eq!(outcome.lead_minutes, 30, "the lead was asked for");
    }

    /// An hour the app chose is not the person's word, and must not be thrown
    /// back at them as an error.
    #[test]
    fn a_day_with_no_hour_still_rings_when_the_fallback_has_already_passed() {
        let (_directory, state) = dictating(20, 0);

        let outcome = state
            .quick_notes
            .create("сегодня забрать посылку", "Europe/Moscow")
            .expect("creates");

        // Saying «сегодня» asks for more urgency, not for no reminder at all.
        assert_eq!(armed_at(&outcome), "2026-08-10 20:01");
        assert!(outcome.reminder_error.is_none());
    }

    /// A timer is the moment the person asked to hear from the app, so nothing
    /// is taken off it.
    #[test]
    fn a_timer_rings_exactly_when_it_was_asked_to() {
        let (_directory, state) = dictating(12, 0);

        let outcome = state
            .quick_notes
            .create("через 20 минут снять с плиты", "Europe/Moscow")
            .expect("creates");

        assert_eq!(armed_at(&outcome), "2026-08-10 12:20");
        assert_eq!(outcome.lead_minutes, 0);
    }

    /// The screen has to be able to say «сказали 15:00, напомню в 14:30», and
    /// that needs both instants to survive the trip out of the core.
    #[test]
    fn the_outcome_carries_the_time_that_was_said_as_well_as_the_alarm() {
        let (_directory, state) = dictating(12, 0);

        let outcome = state
            .quick_notes
            .create("встреча. 15:00", "Europe/Moscow")
            .expect("creates");

        let spoken = outcome
            .spoken_at
            .expect("the phrase named a time")
            .to_zoned(chrono_tz::Europe::Moscow)
            .expect("representable")
            .format("%H:%M")
            .to_string();
        assert_eq!(spoken, "15:00");
        assert_eq!(armed_at(&outcome), "2026-08-10 14:30");
        assert_eq!(outcome.lead_minutes, 30);
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
            fake_icons(),
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
                fake_icons(),
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
            fake_icons(),
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
