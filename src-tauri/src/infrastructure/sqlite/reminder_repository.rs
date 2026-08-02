//! SQLite persistence for one-shot note reminders.

use std::sync::Arc;

use rusqlite::{params, OptionalExtension, Row, Transaction};

use crate::domain::clock::{SharedClock, Timestamp};
use crate::domain::ids::{NoteId, ReminderId, ReminderOccurrenceId};
use crate::domain::reminders::recurrence::Recurrence;
use crate::domain::reminders::repository::ThinWindow;
use crate::domain::reminders::{
    Reminder, ReminderDraft, ReminderOccurrence, ReminderRepository, ScheduledReminder,
    DEFAULT_SOUND_SETTING_KEY, TIME_PRESETS_SETTING_KEY,
};
use crate::error::{AppError, AppResult, ReminderError};

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

    fn read_setting(&self, key: &str) -> AppResult<Option<String>> {
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
}

const SELECT_SCHEDULED: &str =
    "SELECT r.id, r.note_id, r.title, r.body, r.scheduled_at, r.timezone,
            r.sound, r.is_enabled, o.id, o.reminder_id, o.occurrence_at,
            o.alarm_request_code, o.is_exact, r.recurrence_rule
       FROM reminders r
       JOIN reminder_occurrences o ON o.reminder_id = r.id";

const SELECT_REMINDER: &str =
    "SELECT id, note_id, title, body, scheduled_at, timezone, sound, is_enabled,
            recurrence_rule
       FROM reminders";

/// A rule the database holds but this build does not understand is treated as
/// "does not repeat" rather than as a failure: the reminder still fires once,
/// which beats making the whole list unreadable after a downgrade.
fn read_recurrence(stored: Option<String>) -> Option<Recurrence> {
    stored.and_then(|rule| Recurrence::parse(&rule).ok())
}

fn map_reminder(row: &Row<'_>) -> rusqlite::Result<Reminder> {
    Ok(Reminder {
        id: row.get(0)?,
        note_id: row.get(1)?,
        title: row.get(2)?,
        body: row.get(3)?,
        scheduled_at: Timestamp::from_millis(row.get(4)?),
        timezone: row.get(5)?,
        sound: row.get(6)?,
        is_enabled: row.get::<_, i64>(7)? != 0,
        recurrence: read_recurrence(row.get(8)?),
    })
}

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
            recurrence: read_recurrence(row.get(13)?),
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

/// Writes the reminder row, keeping its identity when one already exists.
fn write_reminder(
    transaction: &Transaction<'_>,
    existing: Option<ReminderId>,
    draft: &ReminderDraft,
    now: Timestamp,
) -> AppResult<ReminderId> {
    let rule = draft.recurrence.map(Recurrence::rule);
    if let Some(id) = existing {
        transaction
            .execute(
                "UPDATE reminders
                    SET title = ?1, body = ?2, scheduled_at = ?3, timezone = ?4,
                        sound = ?5, recurrence_rule = ?6, exactness = 'exact',
                        is_enabled = 1, deleted_at = NULL, updated_at = ?7
                  WHERE id = ?8",
                params![
                    draft.title,
                    draft.body,
                    draft.scheduled_at.as_millis(),
                    draft.timezone,
                    draft.sound,
                    rule,
                    now.as_millis(),
                    id,
                ],
            )
            .map_err(AppError::from)?;
        return Ok(id);
    }

    let id = ReminderId::new();
    transaction
        .execute(
            "INSERT INTO reminders (
                id, note_id, title, body, scheduled_at, timezone, sound,
                recurrence_rule, exactness, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'exact', ?9, ?9)",
            params![
                id,
                draft.note_id,
                draft.title,
                draft.body,
                draft.scheduled_at.as_millis(),
                draft.timezone,
                draft.sound,
                rule,
                now.as_millis(),
            ],
        )
        .map_err(AppError::from)?;
    Ok(id)
}

/// The reminder attached to a note, whatever state its occurrences are in.
fn fetch_reminder(transaction: &Transaction<'_>, reminder_id: ReminderId) -> AppResult<Reminder> {
    transaction
        .query_row(
            &format!("{SELECT_REMINDER} WHERE id = ?1"),
            [reminder_id],
            map_reminder,
        )
        .map_err(AppError::from)
}

fn fetch_reminder_for_note(
    transaction: &Transaction<'_>,
    note_id: NoteId,
) -> AppResult<Option<Reminder>> {
    transaction
        .query_row(
            &format!("{SELECT_REMINDER} WHERE note_id = ?1 ORDER BY created_at DESC LIMIT 1"),
            [note_id],
            map_reminder,
        )
        .optional()
        .map_err(AppError::from)
}

/// Request codes of everything still armed for a reminder.
fn armed_codes(transaction: &Transaction<'_>, reminder_id: ReminderId) -> AppResult<Vec<i32>> {
    let mut statement = transaction
        .prepare(
            "SELECT alarm_request_code FROM reminder_occurrences
              WHERE reminder_id = ?1 AND state IN ('scheduled', 'snoozed')",
        )
        .map_err(AppError::from)?;
    let codes = statement
        .query_map([reminder_id], |row| row.get(0))
        .map_err(AppError::from)?
        .collect::<rusqlite::Result<Vec<i32>>>()
        .map_err(AppError::from)?;
    Ok(codes)
}

/// Inserts one occurrence and hands it to `arm`, recording what was granted.
fn arm_occurrence(
    transaction: &Transaction<'_>,
    reminder: &Reminder,
    at: Timestamp,
    now: Timestamp,
    arm: &mut dyn FnMut(&ScheduledReminder) -> AppResult<bool>,
) -> AppResult<ScheduledReminder> {
    let (occurrence_id, request_code) = allocate_occurrence(transaction)?;
    transaction
        .execute(
            "INSERT INTO reminder_occurrences (
                id, reminder_id, occurrence_at, state, alarm_request_code,
                is_exact, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'scheduled', ?4, 1, ?5, ?5)",
            params![
                occurrence_id,
                reminder.id,
                at.as_millis(),
                request_code,
                now.as_millis(),
            ],
        )
        .map_err(AppError::from)?;

    let mut scheduled = ScheduledReminder {
        reminder: reminder.clone(),
        occurrence: ReminderOccurrence {
            id: occurrence_id,
            reminder_id: reminder.id,
            occurrence_at: at,
            alarm_request_code: request_code,
            is_exact: true,
        },
    };
    let is_exact = arm(&scheduled)?;
    if !is_exact {
        transaction
            .execute(
                "UPDATE reminder_occurrences SET is_exact = 0, updated_at = ?1 WHERE id = ?2",
                params![now.as_millis(), occurrence_id],
            )
            .map_err(AppError::from)?;
        scheduled.occurrence.is_exact = false;
    }
    Ok(scheduled)
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
        arm: &mut dyn FnMut(&ScheduledReminder) -> AppResult<bool>,
        disarm: &mut dyn FnMut(i32) -> AppResult<()>,
    ) -> AppResult<ScheduledReminder> {
        let now = self.clock.now();
        self.database.in_transaction(|transaction| {
            if draft.occurrences.is_empty() {
                return Err(AppError::Reminder(ReminderError::NoFutureOccurrence));
            }

            let existing = fetch_reminder_for_note(transaction, draft.note_id)?;
            let stale = match existing.as_ref() {
                Some(reminder) => armed_codes(transaction, reminder.id)?,
                None => Vec::new(),
            };
            let reminder_id = write_reminder(transaction, existing.map(|r| r.id), &draft, now)?;
            let reminder = fetch_reminder(transaction, reminder_id)?;

            // Arm first, take back second. A failure here rolls the transaction
            // back with the reminder the user already had still armed, rather
            // than leaving them with neither.
            let mut armed = Vec::with_capacity(draft.occurrences.len());
            for at in &draft.occurrences {
                armed.push(arm_occurrence(transaction, &reminder, *at, now, arm)?);
            }

            for code in stale {
                disarm(code)?;
            }
            transaction
                .execute(
                    "UPDATE reminder_occurrences
                        SET state = 'cancelled', updated_at = ?1
                      WHERE reminder_id = ?2
                        AND state IN ('scheduled', 'snoozed')
                        AND occurrence_at NOT IN (SELECT occurrence_at FROM reminder_occurrences
                                                   WHERE reminder_id = ?2 AND created_at = ?1)",
                    params![now.as_millis(), reminder_id],
                )
                .map_err(AppError::from)?;

            armed
                .into_iter()
                .min_by_key(|scheduled| scheduled.occurrence.occurrence_at)
                .ok_or(AppError::Reminder(ReminderError::NoFutureOccurrence))
        })
    }

    fn delete_for_note(
        &self,
        note_id: NoteId,
        disarm: &mut dyn FnMut(i32) -> AppResult<()>,
    ) -> AppResult<Option<ScheduledReminder>> {
        let now = self.clock.now();
        self.database.in_transaction(|transaction| {
            let Some(current) = fetch_active(transaction, note_id, now)? else {
                return Ok(None);
            };
            for code in armed_codes(transaction, current.reminder.id)? {
                disarm(code)?;
            }
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
                      WHERE reminder_id = ?2 AND state IN ('scheduled', 'snoozed')",
                    params![now.as_millis(), current.reminder.id],
                )
                .map_err(AppError::from)?;
            Ok(Some(current))
        })
    }

    fn mark_elapsed(&self, now: Timestamp) -> AppResult<u32> {
        self.database.with_connection(|connection| {
            let changed = connection
                .execute(
                    "UPDATE reminder_occurrences
                        SET state = 'fired', fired_at = ?1, updated_at = ?1
                      WHERE state = 'scheduled' AND occurrence_at <= ?1",
                    params![now.as_millis()],
                )
                .map_err(AppError::from)?;
            Ok(u32::try_from(changed).unwrap_or(u32::MAX))
        })
    }

    fn thin_windows(&self, now: Timestamp, target: usize) -> AppResult<Vec<ThinWindow>> {
        self.database.with_connection(|connection| {
            let mut reminders = connection
                .prepare(&format!(
                    "{SELECT_REMINDER}
                      WHERE deleted_at IS NULL
                        AND is_enabled = 1
                        AND recurrence_rule IS NOT NULL"
                ))
                .map_err(AppError::from)?;
            let candidates = reminders
                .query_map([], map_reminder)
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;

            let mut thin = Vec::new();
            for reminder in candidates {
                let mut statement = connection
                    .prepare(
                        "SELECT occurrence_at FROM reminder_occurrences
                          WHERE reminder_id = ?1
                            AND state IN ('scheduled', 'snoozed')
                            AND occurrence_at > ?2
                          ORDER BY occurrence_at",
                    )
                    .map_err(AppError::from)?;
                let armed = statement
                    .query_map(params![reminder.id, now.as_millis()], |row| {
                        Ok(Timestamp::from_millis(row.get(0)?))
                    })
                    .map_err(AppError::from)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(AppError::from)?;
                if armed.len() < target {
                    thin.push(ThinWindow { reminder, armed });
                }
            }
            Ok(thin)
        })
    }

    fn extend_window(
        &self,
        reminder: &Reminder,
        instants: &[Timestamp],
        arm: &mut dyn FnMut(&ScheduledReminder) -> AppResult<bool>,
    ) -> AppResult<u32> {
        let now = self.clock.now();
        self.database.in_transaction(|transaction| {
            let mut added = 0;
            for at in instants {
                let exists: bool = transaction
                    .query_row(
                        "SELECT EXISTS(
                            SELECT 1 FROM reminder_occurrences
                             WHERE reminder_id = ?1
                               AND occurrence_at = ?2
                               AND state IN ('scheduled', 'snoozed')
                         )",
                        params![reminder.id, at.as_millis()],
                        |row| row.get(0),
                    )
                    .map_err(AppError::from)?;
                if exists {
                    continue;
                }
                arm_occurrence(transaction, reminder, *at, now, arm)?;
                added += 1;
            }
            Ok(added)
        })
    }

    fn retime(
        &self,
        scheduled: &ScheduledReminder,
        at: Timestamp,
        zone: &str,
    ) -> AppResult<ScheduledReminder> {
        let now = self.clock.now();
        self.database.in_transaction(|transaction| {
            transaction
                .execute(
                    "UPDATE reminders
                        SET scheduled_at = ?1, timezone = ?2, updated_at = ?3
                      WHERE id = ?4",
                    params![at.as_millis(), zone, now.as_millis(), scheduled.reminder.id,],
                )
                .map_err(AppError::from)?;
            transaction
                .execute(
                    "UPDATE reminder_occurrences
                        SET occurrence_at = ?1, updated_at = ?2
                      WHERE id = ?3",
                    params![at.as_millis(), now.as_millis(), scheduled.occurrence.id],
                )
                .map_err(AppError::from)?;
            fetch_current(transaction, scheduled.reminder.id)
        })
    }

    fn active_scheduled(&self, now: Timestamp) -> AppResult<Vec<ScheduledReminder>> {
        self.database.with_connection(|connection| {
            let mut statement = connection
                .prepare(&format!(
                    "{SELECT_SCHEDULED}
                     WHERE r.deleted_at IS NULL
                       AND r.is_enabled = 1
                       AND o.state IN ('scheduled', 'snoozed')
                       AND o.occurrence_at > ?1
                     ORDER BY o.occurrence_at"
                ))
                .map_err(AppError::from)?;
            let rows = statement
                .query_map(params![now.as_millis()], map_scheduled)
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;
            Ok(rows)
        })
    }

    fn default_sound_id(&self) -> AppResult<Option<String>> {
        self.read_setting(DEFAULT_SOUND_SETTING_KEY)
    }

    fn time_presets(&self) -> AppResult<Option<String>> {
        self.read_setting(TIME_PRESETS_SETTING_KEY)
    }

    fn set_time_presets(&self, raw: &str) -> AppResult<()> {
        let now = self.clock.now();
        self.database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO app_settings (key, value, updated_at)
                          VALUES (?1, ?2, ?3)
                     ON CONFLICT (key) DO UPDATE
                            SET value = excluded.value,
                                updated_at = excluded.updated_at",
                    params![TIME_PRESETS_SETTING_KEY, raw, now.as_millis()],
                )
                .map(|_| ())
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
    use crate::domain::reminders::{
        Recurrence, ReminderDraft, ReminderRepository, ScheduledReminder,
    };
    use crate::error::{AppError, AppResult, NotificationError};
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
            recurrence: None,
            occurrences: vec![Timestamp::from_millis(at)],
        }
    }

    fn repeating(note_id: NoteId, at: i64, count: i64) -> ReminderDraft {
        ReminderDraft {
            recurrence: Some(Recurrence::Daily),
            occurrences: (0..count)
                .map(|step| Timestamp::from_millis(at + step * 86_400_000))
                .collect(),
            ..draft(note_id, at)
        }
    }

    /// Grants an exact alarm and remembers nothing.
    fn granting(_scheduled: &ScheduledReminder) -> AppResult<bool> {
        Ok(true)
    }

    #[test]
    fn replacing_a_reminder_keeps_its_identity_and_takes_the_old_alarm_back() {
        let (repository, note_id) = fixture();
        let first = repository
            .upsert_for_note(draft(note_id, 2_000), &mut granting, &mut |_| Ok(()))
            .expect("creates");

        let mut taken_back = Vec::new();
        let second = repository
            .upsert_for_note(draft(note_id, 3_000), &mut |_| Ok(false), &mut |code| {
                taken_back.push(code);
                Ok(())
            })
            .expect("replaces");

        assert_eq!(
            first.reminder.id, second.reminder.id,
            "the reminder is edited, not replaced"
        );
        assert_eq!(
            taken_back,
            [first.occurrence.alarm_request_code],
            "the alarm for the time the user changed must not survive"
        );
        assert!(
            !second.occurrence.is_exact,
            "an inexact grant has to be recorded"
        );
    }

    #[test]
    fn a_repeating_reminder_arms_its_whole_window() {
        let (repository, note_id) = fixture();
        let mut armed = Vec::new();
        let first = repository
            .upsert_for_note(
                repeating(note_id, 2_000, 4),
                &mut |scheduled| {
                    armed.push(scheduled.occurrence.occurrence_at.as_millis());
                    Ok(true)
                },
                &mut |_| Ok(()),
            )
            .expect("creates");

        assert_eq!(armed.len(), 4, "every firing in the window has to be armed");
        assert_eq!(
            first.occurrence.occurrence_at.as_millis(),
            2_000,
            "the nearest firing is the one the caller gets back"
        );
        assert_eq!(first.reminder.recurrence, Some(Recurrence::Daily));
    }

    #[test]
    fn a_window_is_topped_up_without_arming_what_is_already_armed() {
        let (repository, note_id) = fixture();
        repository
            .upsert_for_note(repeating(note_id, 2_000, 2), &mut granting, &mut |_| Ok(()))
            .expect("creates");

        let thin = repository
            .thin_windows(Timestamp::from_millis(1_000), 4)
            .expect("reads");
        assert_eq!(thin.len(), 1);
        assert_eq!(thin[0].armed.len(), 2);

        let wanted: Vec<Timestamp> = (0..4)
            .map(|step| Timestamp::from_millis(2_000 + step * 86_400_000))
            .collect();
        let added = repository
            .extend_window(&thin[0].reminder, &wanted, &mut granting)
            .expect("extends");
        assert_eq!(added, 2, "only the two missing firings are armed");

        let still_thin = repository
            .thin_windows(Timestamp::from_millis(1_000), 4)
            .expect("reads");
        assert!(still_thin.is_empty(), "a full window is not thin any more");
    }

    #[test]
    fn a_firing_whose_time_has_passed_stops_counting_as_armed() {
        let (repository, note_id) = fixture();
        repository
            .upsert_for_note(repeating(note_id, 2_000, 2), &mut granting, &mut |_| Ok(()))
            .expect("creates");

        let elapsed = repository
            .mark_elapsed(Timestamp::from_millis(3_000))
            .expect("marks");

        assert_eq!(elapsed, 1);
        let thin = repository
            .thin_windows(Timestamp::from_millis(3_000), 4)
            .expect("reads");
        assert_eq!(
            thin[0].armed.len(),
            1,
            "the fired one is not armed any more"
        );
    }

    #[test]
    fn deleting_takes_back_every_firing_that_was_armed() {
        let (repository, note_id) = fixture();
        repository
            .upsert_for_note(repeating(note_id, 2_000, 3), &mut granting, &mut |_| Ok(()))
            .expect("creates");

        let mut taken_back = Vec::new();
        repository
            .delete_for_note(note_id, &mut |code| {
                taken_back.push(code);
                Ok(())
            })
            .expect("deletes");

        assert_eq!(
            taken_back.len(),
            3,
            "leaving even one armed would fire for a reminder that is gone"
        );
    }

    #[test]
    fn a_schedule_failure_rolls_the_sql_transaction_back() {
        let (repository, note_id) = fixture();
        let result = repository.upsert_for_note(
            draft(note_id, 2_000),
            &mut |_| {
                Err(AppError::Notification(NotificationError::ScheduleFailed {
                    reason: "test".into(),
                }))
            },
            &mut |_| Ok(()),
        );

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
            .upsert_for_note(draft(note_id, 2_000), &mut granting, &mut |_| Ok(()))
            .expect("creates");
        let mut cancelled = None;

        repository
            .delete_for_note(note_id, &mut |code| {
                cancelled = Some(code);
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
