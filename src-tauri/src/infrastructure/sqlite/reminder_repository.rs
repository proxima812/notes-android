//! SQLite persistence for one-shot note reminders.

use std::sync::Arc;

use rusqlite::{params, OptionalExtension, Row, Transaction};

use crate::domain::clock::{SharedClock, Timestamp};
use crate::domain::ids::{NoteId, ReminderId, ReminderOccurrenceId};
use crate::domain::reminders::{
    Reminder, ReminderDraft, ReminderOccurrence, ReminderRepository, ScheduledReminder,
    DEFAULT_SOUND_SETTING_KEY,
};
use crate::error::{AppError, AppResult};

use super::Database;

pub struct SqliteReminderRepository {
    database: Arc<Database>,
    clock: SharedClock,
}

impl SqliteReminderRepository {
    #[must_use]
    pub fn new(database: Arc<Database>, clock: SharedClock) -> Self {
        Self { database, clock }
    }
}

const SELECT_SCHEDULED: &str =
    "SELECT r.id, r.note_id, r.title, r.body, r.scheduled_at, r.timezone,
            r.sound, r.is_enabled, o.id, o.reminder_id, o.occurrence_at,
            o.alarm_request_code, o.is_exact
       FROM reminders r
       JOIN reminder_occurrences o ON o.reminder_id = r.id";

fn map_scheduled(row: &Row<'_>) -> rusqlite::Result<ScheduledReminder> {
    Ok(ScheduledReminder {
        reminder: Reminder {
            id: row.get(0)?,
            note_id: row.get(1)?,
            title: row.get(2)?,
            body: row.get(3)?,
            scheduled_at: Timestamp::from_millis(row.get(4)?),
            timezone: row.get(5)?,
            sound: row.get(6)?,
            is_enabled: row.get::<_, i64>(7)? != 0,
        },
        occurrence: ReminderOccurrence {
            id: row.get(8)?,
            reminder_id: row.get(9)?,
            occurrence_at: Timestamp::from_millis(row.get(10)?),
            alarm_request_code: row.get(11)?,
            is_exact: row.get::<_, i64>(12)? != 0,
        },
    })
}

fn fetch_active(
    transaction: &Transaction<'_>,
    note_id: NoteId,
    now: Timestamp,
) -> AppResult<Option<ScheduledReminder>> {
    transaction
        .query_row(
            &format!(
                "{SELECT_SCHEDULED}
                 WHERE r.note_id = ?1
                   AND r.deleted_at IS NULL
                   AND r.is_enabled = 1
                   AND o.state IN ('scheduled', 'snoozed')
                   AND o.occurrence_at > ?2
                 ORDER BY o.occurrence_at
                 LIMIT 1"
            ),
            params![note_id, now.as_millis()],
            map_scheduled,
        )
        .optional()
        .map_err(AppError::from)
}

fn fetch_current(
    transaction: &Transaction<'_>,
    reminder_id: ReminderId,
) -> AppResult<ScheduledReminder> {
    transaction
        .query_row(
            &format!(
                "{SELECT_SCHEDULED}
                 WHERE r.id = ?1
                 ORDER BY o.occurrence_at DESC
                 LIMIT 1"
            ),
            [reminder_id],
            map_scheduled,
        )
        .map_err(AppError::from)
}

fn raw_request_code(id: ReminderOccurrenceId) -> i32 {
    let bytes = id.as_uuid().as_bytes();
    i32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) & i32::MAX
}

fn allocate_occurrence(transaction: &Transaction<'_>) -> AppResult<(ReminderOccurrenceId, i32)> {
    loop {
        let id = ReminderOccurrenceId::new();
        let code = raw_request_code(id);
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM reminder_occurrences WHERE alarm_request_code = ?1
                )",
                [code],
                |row| row.get(0),
            )
            .map_err(AppError::from)?;
        if !exists {
            return Ok((id, code));
        }
    }
}

fn write_next(
    transaction: &Transaction<'_>,
    previous: Option<&ScheduledReminder>,
    draft: &ReminderDraft,
    now: Timestamp,
) -> AppResult<ReminderId> {
    if let Some(stored) = previous {
        transaction
            .execute(
                "UPDATE reminders
                    SET title = ?1, body = ?2, scheduled_at = ?3, timezone = ?4,
                        sound = ?5, exactness = 'exact', is_enabled = 1,
                        deleted_at = NULL, updated_at = ?6
                  WHERE id = ?7",
                params![
                    draft.title,
                    draft.body,
                    draft.scheduled_at.as_millis(),
                    draft.timezone,
                    draft.sound,
                    now.as_millis(),
                    stored.reminder.id,
                ],
            )
            .map_err(AppError::from)?;
        transaction
            .execute(
                "UPDATE reminder_occurrences
                    SET occurrence_at = ?1, state = 'scheduled', is_exact = 1,
                        fired_at = NULL, handled_at = NULL, updated_at = ?2
                  WHERE id = ?3",
                params![
                    draft.scheduled_at.as_millis(),
                    now.as_millis(),
                    stored.occurrence.id,
                ],
            )
            .map_err(AppError::from)?;
        return Ok(stored.reminder.id);
    }

    let reminder_id = ReminderId::new();
    let (occurrence_id, request_code) = allocate_occurrence(transaction)?;
    transaction
        .execute(
            "INSERT INTO reminders (
                id, note_id, title, body, scheduled_at, timezone, sound,
                exactness, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'exact', ?8, ?8)",
            params![
                reminder_id,
                draft.note_id,
                draft.title,
                draft.body,
                draft.scheduled_at.as_millis(),
                draft.timezone,
                draft.sound,
                now.as_millis(),
            ],
        )
        .map_err(AppError::from)?;
    transaction
        .execute(
            "INSERT INTO reminder_occurrences (
                id, reminder_id, occurrence_at, state, alarm_request_code,
                is_exact, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'scheduled', ?4, 1, ?5, ?5)",
            params![
                occurrence_id,
                reminder_id,
                draft.scheduled_at.as_millis(),
                request_code,
                now.as_millis(),
            ],
        )
        .map_err(AppError::from)?;
    Ok(reminder_id)
}

impl ReminderRepository for SqliteReminderRepository {
    fn find_active_for_note(
        &self,
        note_id: NoteId,
        now: Timestamp,
    ) -> AppResult<Option<ScheduledReminder>> {
        self.database
            .in_transaction(|transaction| fetch_active(transaction, note_id, now))
    }

    fn upsert_for_note(
        &self,
        draft: ReminderDraft,
        schedule: &mut dyn FnMut(Option<&ScheduledReminder>, &ScheduledReminder) -> AppResult<bool>,
    ) -> AppResult<ScheduledReminder> {
        let now = self.clock.now();
        self.database.in_transaction(|transaction| {
            let previous = fetch_active(transaction, draft.note_id, now)?;
            let reminder_id = write_next(transaction, previous.as_ref(), &draft, now)?;
            let mut next = fetch_current(transaction, reminder_id)?;
            let is_exact = schedule(previous.as_ref(), &next)?;
            transaction
                .execute(
                    "UPDATE reminder_occurrences
                        SET is_exact = ?1, updated_at = ?2
                      WHERE id = ?3",
                    params![is_exact, now.as_millis(), next.occurrence.id],
                )
                .map_err(AppError::from)?;
            next.occurrence.is_exact = is_exact;
            Ok(next)
        })
    }

    fn delete_for_note(
        &self,
        note_id: NoteId,
        cancel: &mut dyn FnMut(&ScheduledReminder) -> AppResult<()>,
    ) -> AppResult<Option<ScheduledReminder>> {
        let now = self.clock.now();
        self.database.in_transaction(|transaction| {
            let Some(current) = fetch_active(transaction, note_id, now)? else {
                return Ok(None);
            };
            cancel(&current)?;
            transaction
                .execute(
                    "UPDATE reminders
                        SET is_enabled = 0, deleted_at = ?1, updated_at = ?1
                      WHERE id = ?2",
                    params![now.as_millis(), current.reminder.id],
                )
                .map_err(AppError::from)?;
            transaction
                .execute(
                    "UPDATE reminder_occurrences
                        SET state = 'cancelled', updated_at = ?1
                      WHERE id = ?2",
                    params![now.as_millis(), current.occurrence.id],
                )
                .map_err(AppError::from)?;
            Ok(Some(current))
        })
    }

    fn default_sound_id(&self) -> AppResult<Option<String>> {
        self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    [DEFAULT_SOUND_SETTING_KEY],
                    |row| row.get(0),
                )
                .optional()
                .map_err(AppError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::domain::clock::{FixedClock, SharedClock, Timestamp};
    use crate::domain::ids::NoteId;
    use crate::domain::notes::{NoteDraft, NoteRepository};
    use crate::domain::reminders::{ReminderDraft, ReminderRepository};
    use crate::error::{AppError, NotificationError};
    use crate::infrastructure::sqlite::{Database, SqliteNoteRepository};

    use super::SqliteReminderRepository;

    fn fixture() -> (SqliteReminderRepository, NoteId) {
        let clock: SharedClock = Arc::new(FixedClock::new(Timestamp::from_millis(1_000)));
        let database = Arc::new(Database::open_in_memory(1_000).expect("opens"));
        let note = SqliteNoteRepository::new(Arc::clone(&database), Arc::clone(&clock))
            .create(NoteDraft {
                title: Some("Заметка".into()),
                ..NoteDraft::default()
            })
            .expect("creates note");
        (SqliteReminderRepository::new(database, clock), note.id)
    }

    fn draft(note_id: NoteId, at: i64) -> ReminderDraft {
        ReminderDraft {
            note_id,
            title: "Проверить".into(),
            body: "Текст".into(),
            scheduled_at: Timestamp::from_millis(at),
            timezone: "Asia/Almaty".into(),
            sound: "default".into(),
        }
    }

    #[test]
    fn upsert_keeps_one_active_row_and_reuses_the_request_code() {
        let (repository, note_id) = fixture();
        let first = repository
            .upsert_for_note(draft(note_id, 2_000), &mut |_, _| Ok(true))
            .expect("creates");
        let second = repository
            .upsert_for_note(draft(note_id, 3_000), &mut |previous, _| {
                assert!(previous.is_some());
                Ok(false)
            })
            .expect("replaces");

        assert_eq!(
            first.occurrence.alarm_request_code,
            second.occurrence.alarm_request_code
        );
        assert_eq!(first.reminder.id, second.reminder.id);
        assert!(!second.occurrence.is_exact);
    }

    #[test]
    fn a_schedule_failure_rolls_the_sql_transaction_back() {
        let (repository, note_id) = fixture();
        let result = repository.upsert_for_note(draft(note_id, 2_000), &mut |_, _| {
            Err(AppError::Notification(NotificationError::ScheduleFailed {
                reason: "test".into(),
            }))
        });

        assert!(result.is_err());
        assert!(repository
            .find_active_for_note(note_id, Timestamp::from_millis(1_000))
            .expect("reads")
            .is_none());
    }

    #[test]
    fn delete_cancels_inside_the_same_transaction() {
        let (repository, note_id) = fixture();
        let stored = repository
            .upsert_for_note(draft(note_id, 2_000), &mut |_, _| Ok(true))
            .expect("creates");
        let mut cancelled = None;

        repository
            .delete_for_note(note_id, &mut |current| {
                cancelled = Some(current.occurrence.alarm_request_code);
                Ok(())
            })
            .expect("deletes");

        assert_eq!(cancelled, Some(stored.occurrence.alarm_request_code));
        assert!(repository
            .find_active_for_note(note_id, Timestamp::from_millis(1_000))
            .expect("reads")
            .is_none());
    }
}
