//! Use cases.
//!
//! A use case owns one user-visible operation and coordinates the repositories
//! it needs. It holds its collaborators by constructor injection, so a test can
//! hand it a stub and the Tauri command layer stays free of logic.

use std::sync::Arc;

use crate::domain::clock::{SharedClock, Timestamp};
use crate::domain::ids::NoteId;
use crate::domain::notes::{
    validate_content, validate_title, Note, NoteRepository, Page, PageRequest,
};
use crate::domain::reminders::{
    resolve_sound, sound_presets, time_presets, ReminderDraft, ReminderRepository,
    ScheduledReminder, SoundPreset, FALLBACK_SOUND_ID,
};
use crate::domain::search::{SearchHit, SearchRepository};
use crate::error::{AppError, AppResult, NotificationError, ValidationError};
use crate::platform::{Alarm, AlarmClock};

use super::dto::{ListNotesRequest, SearchRequest, UpsertReminderRequest};

pub struct NoteUseCases {
    notes: Arc<dyn NoteRepository>,
}

impl NoteUseCases {
    #[must_use]
    pub fn new(notes: Arc<dyn NoteRepository>) -> Self {
        Self { notes }
    }

    /// # Errors
    /// Fails on validation or a database error.
    pub fn create(&self, draft: crate::domain::notes::NoteDraft) -> AppResult<Note> {
        self.notes.create(draft)
    }

    /// # Errors
    /// Fails when the identifier is malformed, or on a database error.
    pub fn get(&self, id: &str) -> AppResult<Option<Note>> {
        self.notes.find(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails when the note is missing or read-only, or on a database error.
    pub fn update(&self, id: &str, patch: crate::domain::notes::NotePatch) -> AppResult<Note> {
        self.notes.update(NoteId::parse(id)?, patch)
    }

    /// # Errors
    /// Fails when the note is missing, or on a database error.
    pub fn move_to_trash(&self, id: &str) -> AppResult<()> {
        self.notes.soft_delete(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails when the note is missing, or on a database error.
    pub fn restore(&self, id: &str) -> AppResult<Note> {
        self.notes.restore(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails when the note is missing, or on a database error.
    pub fn purge(&self, id: &str) -> AppResult<()> {
        self.notes.purge(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails on a database error.
    pub fn empty_trash(&self) -> AppResult<u32> {
        self.notes.purge_trash(None)
    }

    /// # Errors
    /// Fails when a request identifier is malformed, or on a database error.
    pub fn list(&self, request: &ListNotesRequest) -> AppResult<Page<Note>> {
        let filter = request.to_filter()?;
        self.notes.list(&filter, request.sort, request.to_page())
    }

    /// # Errors
    /// Fails when the source is missing, or on a database error.
    pub fn duplicate(&self, id: &str) -> AppResult<Note> {
        self.notes.duplicate(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails when a request identifier is malformed, or on a database error.
    pub fn count(&self, request: &ListNotesRequest) -> AppResult<u32> {
        self.notes.count(&request.to_filter()?)
    }
}

#[derive(Debug)]
pub struct ReminderView {
    pub scheduled: ScheduledReminder,
    pub effective_sound: SoundPreset,
}

#[derive(Debug)]
pub struct ReminderSoundCatalog {
    pub default_sound_id: String,
    pub items: Vec<SoundPreset>,
}

pub struct ReminderUseCases {
    reminders: Arc<dyn ReminderRepository>,
    alarms: Arc<dyn AlarmClock>,
    clock: SharedClock,
}

impl ReminderUseCases {
    #[must_use]
    pub fn new(
        reminders: Arc<dyn ReminderRepository>,
        alarms: Arc<dyn AlarmClock>,
        clock: SharedClock,
    ) -> Self {
        Self {
            reminders,
            alarms,
            clock,
        }
    }

    /// Returns the future active reminder for a note.
    ///
    /// # Errors
    /// Fails for a malformed note id, invalid sound setting, or storage errors.
    pub fn get_for_note(&self, note_id: &str) -> AppResult<Option<ReminderView>> {
        let note_id = NoteId::parse(note_id)?;
        let configured_default = self.reminders.default_sound_id()?;
        self.reminders
            .find_active_for_note(note_id, self.clock.now())?
            .map(|scheduled| {
                let effective_sound =
                    resolve_sound(&scheduled.reminder.sound, configured_default.as_deref())?;
                Ok(ReminderView {
                    scheduled,
                    effective_sound,
                })
            })
            .transpose()
    }

    /// Creates or replaces the note's one active reminder.
    ///
    /// # Errors
    /// Fails on validation, denied notifications, storage, or platform errors.
    pub fn upsert_for_note(&self, request: UpsertReminderRequest) -> AppResult<ReminderView> {
        let note_id = NoteId::parse(&request.note_id)?;
        let scheduled_at = Timestamp::from_millis(request.scheduled_at);
        if scheduled_at <= self.clock.now() {
            return Err(AppError::Validation(ValidationError::TimeInPast));
        }
        let title = request.title.trim();
        if title.is_empty() {
            return Err(AppError::Validation(ValidationError::Required {
                field: "title",
            }));
        }
        validate_title(title)?;
        validate_content(&request.body)?;
        if request.timezone.parse::<chrono_tz::Tz>().is_err() {
            return Err(AppError::Validation(ValidationError::UnknownTimeZone {
                value: request.timezone,
            }));
        }

        let configured_default = self.reminders.default_sound_id()?;
        let effective_sound = resolve_sound(&request.sound, configured_default.as_deref())?;
        let permissions = self.alarms.permissions()?;
        if !permissions.notifications_granted && !self.alarms.request_notification_permission()? {
            return Err(AppError::Notification(
                NotificationError::NotificationsDenied,
            ));
        }

        let draft = ReminderDraft {
            note_id,
            title: title.to_owned(),
            body: request.body,
            scheduled_at,
            timezone: request.timezone,
            sound: request.sound,
        };
        let alarms = Arc::clone(&self.alarms);
        let mut applied: Option<(Option<ScheduledReminder>, ScheduledReminder)> = None;
        let result = self
            .reminders
            .upsert_for_note(draft, &mut |previous, next| {
                applied = Some((previous.cloned(), next.clone()));
                alarms.schedule(&alarm_from(next, effective_sound))
            });

        match result {
            Ok(scheduled) => Ok(ReminderView {
                scheduled,
                effective_sound,
            }),
            Err(error) => {
                if let Some((previous, next)) = applied {
                    let _ = self.alarms.cancel(next.occurrence.alarm_request_code);
                    if let Some(old) = previous {
                        if let Ok(old_sound) =
                            resolve_sound(&old.reminder.sound, configured_default.as_deref())
                        {
                            let _ = self.alarms.schedule(&alarm_from(&old, old_sound));
                        }
                    }
                }
                Err(error)
            }
        }
    }

    /// Cancels and soft-deletes a note reminder.
    ///
    /// # Errors
    /// Fails for a malformed note id, storage errors, or platform errors.
    pub fn delete_for_note(&self, note_id: &str) -> AppResult<Option<ReminderView>> {
        let note_id = NoteId::parse(note_id)?;
        let configured_default = self.reminders.default_sound_id()?;
        let alarms = Arc::clone(&self.alarms);
        let mut cancelled: Option<ScheduledReminder> = None;
        let result = self.reminders.delete_for_note(note_id, &mut |current| {
            cancelled = Some(current.clone());
            alarms.cancel(current.occurrence.alarm_request_code)
        });

        match result {
            Ok(Some(scheduled)) => {
                let effective_sound =
                    resolve_sound(&scheduled.reminder.sound, configured_default.as_deref())?;
                Ok(Some(ReminderView {
                    scheduled,
                    effective_sound,
                }))
            }
            Ok(None) => Ok(None),
            Err(error) => {
                if let Some(previous) = cancelled {
                    if let Ok(sound) =
                        resolve_sound(&previous.reminder.sound, configured_default.as_deref())
                    {
                        let _ = self.alarms.schedule(&alarm_from(&previous, sound));
                    }
                }
                Err(error)
            }
        }
    }

    /// Returns the note a notification tap asked to open, once.
    ///
    /// The id comes back from an Android intent extra rather than from the
    /// database, so it is parsed instead of trusted. A malformed one counts as
    /// nothing pending rather than as an error: this is asked on every start,
    /// and failing it would break opening the app normally.
    ///
    /// # Errors
    /// Fails when the platform call itself fails.
    pub fn take_launch_target(&self) -> AppResult<Option<String>> {
        Ok(self
            .alarms
            .take_launch_target()?
            .and_then(|id| NoteId::parse(&id).ok())
            .map(|id| id.to_string()))
    }

    /// Returns sound choices and the concrete global default.
    ///
    /// # Errors
    /// Fails when the stored default is not in the bundled catalog.
    pub fn sound_catalog(&self) -> AppResult<ReminderSoundCatalog> {
        let configured = self.reminders.default_sound_id()?;
        let default = configured.as_deref().unwrap_or(FALLBACK_SOUND_ID);
        let preset = resolve_sound(default, None)?;
        Ok(ReminderSoundCatalog {
            default_sound_id: preset.id.to_owned(),
            items: sound_presets().to_vec(),
        })
    }

    /// The times offered for one-tap picking, as `HH:MM`.
    ///
    /// # Errors
    /// Fails when the stored set cannot be read or is not the form this build
    /// writes.
    pub fn time_presets(&self) -> AppResult<Vec<String>> {
        let stored = self.reminders.time_presets()?;
        Ok(labels(&time_presets::parse_stored(stored.as_deref())?))
    }

    /// Replaces the whole set with the one the user now wants.
    ///
    /// The whole list is written rather than a diff applied, because "delete
    /// one of the times we shipped" and "add one of my own" are the same edit
    /// from the core's side: this is the set, keep it.
    ///
    /// # Errors
    /// Fails when a value is not a real `HH:MM` time, when there are more of
    /// them than the cap allows, or on a database error.
    pub fn save_time_presets(&self, values: &[String]) -> AppResult<Vec<String>> {
        let presets = time_presets::normalise(values)?;
        self.reminders
            .set_time_presets(&time_presets::serialise(&presets)?)?;
        Ok(labels(&presets))
    }
}

fn labels(presets: &[time_presets::TimePreset]) -> Vec<String> {
    presets.iter().map(|preset| preset.label()).collect()
}

pub(super) fn alarm_from(scheduled: &ScheduledReminder, sound: SoundPreset) -> Alarm {
    Alarm {
        occurrence_id: scheduled.occurrence.id.to_string(),
        note_id: scheduled.reminder.note_id.to_string(),
        request_code: scheduled.occurrence.alarm_request_code,
        trigger_at: scheduled.occurrence.occurrence_at,
        title: scheduled.reminder.title.clone(),
        body: scheduled.reminder.body.clone(),
        exact: true,
        sound_id: sound.resource_name.to_owned(),
        sound_label: sound.label.to_owned(),
        vibrate: true,
    }
}

/// Moves a note to trash and cancels its active reminder as one user action.
///
/// # Errors
/// Restores the note when reminder cancellation or persistence fails.
pub fn move_note_to_trash(
    notes: &NoteUseCases,
    reminders: &ReminderUseCases,
    id: &str,
) -> AppResult<()> {
    notes.move_to_trash(id)?;
    if let Err(error) = reminders.delete_for_note(id) {
        let _ = notes.restore(id);
        return Err(error);
    }
    Ok(())
}

pub struct SearchUseCases {
    search: Arc<dyn SearchRepository>,
}

impl SearchUseCases {
    #[must_use]
    pub fn new(search: Arc<dyn SearchRepository>) -> Self {
        Self { search }
    }

    /// Runs a search and remembers the query.
    ///
    /// History is recorded here rather than in the repository so that an
    /// internal search (say, from a smart folder) can reuse the same query path
    /// without polluting the user's recent-searches list.
    ///
    /// # Errors
    /// Fails when a request identifier is malformed, or on a database error.
    pub fn search(&self, request: &SearchRequest) -> AppResult<Page<SearchHit>> {
        let query = request.to_query()?;
        let page = self.search.search(&query, request.to_page())?;

        // Only the first page of a query counts as "the user ran this search";
        // paging through results must not push it to the top again.
        if request.offset.unwrap_or(0) == 0 {
            self.search.record_history(&request.text, page.total)?;
        }

        Ok(page)
    }

    /// Runs a search without touching history.
    ///
    /// # Errors
    /// Fails when a request identifier is malformed, or on a database error.
    pub fn search_quietly(&self, request: &SearchRequest) -> AppResult<Page<SearchHit>> {
        self.search.search(&request.to_query()?, request.to_page())
    }

    /// # Errors
    /// Fails on a database error.
    pub fn recent_queries(&self, limit: Option<u32>) -> AppResult<Vec<String>> {
        self.search.recent_queries(limit.unwrap_or(10))
    }

    /// # Errors
    /// Fails on a database error.
    pub fn clear_history(&self) -> AppResult<()> {
        self.search.clear_history()
    }
}

/// Reads a page without the caller having to know the default size.
#[must_use]
pub fn default_page() -> PageRequest {
    PageRequest::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::dto::UpsertReminderRequest;
    use crate::domain::clock::{FixedClock, SharedClock, Timestamp};
    use crate::domain::notes::NoteDraft;
    use crate::infrastructure::sqlite::{
        Database, SqliteNoteRepository, SqliteReminderRepository, SqliteSearchRepository,
    };
    use crate::platform::{Alarm, AlarmClock, AlarmPermissions};

    struct Fixture {
        notes: NoteUseCases,
        search: SearchUseCases,
    }

    #[derive(Default)]
    struct FakeAlarmClock {
        scheduled: parking_lot::Mutex<Vec<Alarm>>,
        cancelled: parking_lot::Mutex<Vec<i32>>,
        cancelled_everything: parking_lot::Mutex<bool>,
        launch_target: parking_lot::Mutex<Option<String>>,
        notifications_granted: bool,
        exact: bool,
    }

    impl AlarmClock for FakeAlarmClock {
        fn schedule(&self, alarm: &Alarm) -> AppResult<bool> {
            self.scheduled.lock().push(alarm.clone());
            Ok(self.exact)
        }

        fn cancel(&self, request_code: i32) -> AppResult<()> {
            self.cancelled.lock().push(request_code);
            Ok(())
        }

        fn cancel_all(&self) -> AppResult<()> {
            *self.cancelled_everything.lock() = true;
            Ok(())
        }

        fn take_launch_target(&self) -> AppResult<Option<String>> {
            Ok(self.launch_target.lock().take())
        }

        fn permissions(&self) -> AppResult<AlarmPermissions> {
            Ok(AlarmPermissions {
                notifications_granted: self.notifications_granted,
                exact_allowed: self.exact,
            })
        }

        fn request_notification_permission(&self) -> AppResult<bool> {
            Ok(self.notifications_granted)
        }
    }

    struct ReminderFixture {
        notes: NoteUseCases,
        reminders: ReminderUseCases,
        note_id: NoteId,
        alarms: Arc<FakeAlarmClock>,
    }

    fn reminder_fixture(notifications_granted: bool, exact: bool) -> ReminderFixture {
        let clock: SharedClock = Arc::new(FixedClock::new(Timestamp::from_millis(1_000)));
        let database = Arc::new(Database::open_in_memory(1_000).expect("opens"));
        let note_repository = Arc::new(SqliteNoteRepository::new(
            Arc::clone(&database),
            Arc::clone(&clock),
        ));
        let note = note_repository
            .create(NoteDraft {
                title: Some("Заметка".into()),
                ..NoteDraft::default()
            })
            .expect("creates note");
        let alarms = Arc::new(FakeAlarmClock {
            notifications_granted,
            exact,
            ..FakeAlarmClock::default()
        });
        let reminder_repository =
            Arc::new(SqliteReminderRepository::new(database, Arc::clone(&clock)));
        ReminderFixture {
            notes: NoteUseCases::new(note_repository),
            reminders: ReminderUseCases::new(reminder_repository, alarms.clone(), clock),
            note_id: note.id,
            alarms,
        }
    }

    fn valid_reminder_request(note_id: NoteId) -> UpsertReminderRequest {
        UpsertReminderRequest {
            note_id: note_id.to_string(),
            title: "Проверить".into(),
            body: "Текст".into(),
            scheduled_at: 2_000,
            timezone: "Asia/Almaty".into(),
            sound: "default".into(),
        }
    }

    fn fixture() -> Fixture {
        let clock: SharedClock =
            Arc::new(FixedClock::new(Timestamp::from_millis(1_700_000_000_000)));
        let database = Arc::new(Database::open_in_memory(1_700_000_000_000).expect("opens"));
        Fixture {
            notes: NoteUseCases::new(Arc::new(SqliteNoteRepository::new(
                Arc::clone(&database),
                Arc::clone(&clock),
            ))),
            search: SearchUseCases::new(Arc::new(SqliteSearchRepository::new(database, clock))),
        }
    }

    #[test]
    fn a_malformed_identifier_never_reaches_the_database() {
        let f = fixture();
        let error = f.notes.get("not-a-uuid").expect_err("must be rejected");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn the_full_note_lifecycle_works_end_to_end() {
        let f = fixture();

        let created = f
            .notes
            .create(NoteDraft {
                title: Some("Покупки".to_owned()),
                content_text: Some("молоко".to_owned()),
                ..NoteDraft::default()
            })
            .expect("creates");
        let id = created.id.to_string();

        assert!(f.notes.get(&id).expect("reads").is_some());

        f.notes.move_to_trash(&id).expect("trashes");
        assert_eq!(
            f.notes.count(&ListNotesRequest::default()).expect("counts"),
            0
        );

        f.notes.restore(&id).expect("restores");
        assert_eq!(
            f.notes.count(&ListNotesRequest::default()).expect("counts"),
            1
        );

        f.notes.move_to_trash(&id).expect("trashes again");
        let removed = f.notes.empty_trash().expect("empties");
        assert_eq!(removed, 1);
        assert!(f.notes.get(&id).expect("reads").is_none());
    }

    #[test]
    fn searching_records_the_query_in_history() {
        let f = fixture();
        f.notes
            .create(NoteDraft {
                title: Some("Покупки".to_owned()),
                content_text: Some("молоко".to_owned()),
                ..NoteDraft::default()
            })
            .expect("creates");

        let page = f
            .search
            .search(&SearchRequest {
                text: "молоко".to_owned(),
                ..SearchRequest::default()
            })
            .expect("searches");

        assert_eq!(page.total, 1);
        assert_eq!(
            f.search.recent_queries(None).expect("reads"),
            vec!["молоко".to_owned()]
        );
    }

    #[test]
    fn paging_through_results_does_not_re_record_the_query() {
        let f = fixture();
        for index in 0..5 {
            f.notes
                .create(NoteDraft {
                    title: Some(format!("Заметка {index}")),
                    content_text: Some("молоко".to_owned()),
                    ..NoteDraft::default()
                })
                .expect("creates");
        }

        f.search
            .search(&SearchRequest {
                text: "молоко".to_owned(),
                limit: Some(2),
                ..SearchRequest::default()
            })
            .expect("first page");
        f.search
            .search(&SearchRequest {
                text: "хлеб".to_owned(),
                ..SearchRequest::default()
            })
            .expect("another query");
        f.search
            .search(&SearchRequest {
                text: "молоко".to_owned(),
                limit: Some(2),
                offset: Some(2),
                ..SearchRequest::default()
            })
            .expect("second page");

        // Had paging re-recorded it, "молоко" would have jumped back to the top.
        assert_eq!(
            f.search.recent_queries(None).expect("reads"),
            vec!["хлеб".to_owned(), "молоко".to_owned()]
        );
    }

    #[test]
    fn a_quiet_search_leaves_no_trace() {
        let f = fixture();
        f.search
            .search_quietly(&SearchRequest {
                text: "молоко".to_owned(),
                ..SearchRequest::default()
            })
            .expect("searches");
        assert!(f.search.recent_queries(None).expect("reads").is_empty());
    }

    #[test]
    fn reminder_upsert_rejects_past_time_before_touching_android() {
        let fixture = reminder_fixture(true, true);
        let mut request = valid_reminder_request(fixture.note_id);
        request.scheduled_at = 999;

        let error = fixture
            .reminders
            .upsert_for_note(request)
            .expect_err("past time fails");

        assert_eq!(error.code(), "validation_time_in_past");
        assert!(fixture.alarms.scheduled.lock().is_empty());
    }

    #[test]
    fn reminder_upsert_resolves_default_sound_and_records_inexact_fallback() {
        let fixture = reminder_fixture(true, false);

        let stored = fixture
            .reminders
            .upsert_for_note(valid_reminder_request(fixture.note_id))
            .expect("schedules");

        assert_eq!(stored.effective_sound.id, "death_and_rebirth");
        assert!(!stored.scheduled.occurrence.is_exact);
        assert_eq!(
            fixture.alarms.scheduled.lock()[0].sound_id,
            "death_and_rebirth"
        );
    }

    #[test]
    fn denied_notifications_do_not_create_a_database_row() {
        let fixture = reminder_fixture(false, true);

        let error = fixture
            .reminders
            .upsert_for_note(valid_reminder_request(fixture.note_id))
            .expect_err("denied");

        assert_eq!(error.code(), "notification_permission_denied");
        assert!(fixture
            .reminders
            .get_for_note(&fixture.note_id.to_string())
            .expect("reads")
            .is_none());
    }

    #[test]
    fn moving_a_note_to_trash_cancels_its_active_reminder() {
        let fixture = reminder_fixture(true, true);
        let scheduled = fixture
            .reminders
            .upsert_for_note(valid_reminder_request(fixture.note_id))
            .expect("schedules");

        move_note_to_trash(
            &fixture.notes,
            &fixture.reminders,
            &fixture.note_id.to_string(),
        )
        .expect("trashes");

        assert_eq!(
            fixture.alarms.cancelled.lock().as_slice(),
            &[scheduled.scheduled.occurrence.alarm_request_code]
        );
        assert_eq!(
            fixture
                .notes
                .count(&ListNotesRequest::default())
                .expect("counts"),
            0
        );
    }
}
