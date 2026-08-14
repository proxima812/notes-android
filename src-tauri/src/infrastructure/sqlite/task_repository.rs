//! SQLite persistence for the tasks on a note.

use std::sync::Arc;

use rusqlite::{params, Row};

use crate::domain::clock::Timestamp;
use crate::domain::ids::{NoteId, TaskId};
use crate::domain::tasks::{validate_title, Task, TaskProgress, TaskRepository, TaskStatus};
use crate::error::{AppError, AppResult, DatabaseError};

use super::Database;

pub struct SqliteTaskRepository {
    database: Arc<Database>,
}

impl SqliteTaskRepository {
    #[must_use]
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }
}

const SELECT_TASK: &str = "SELECT id, note_id, title, status, position, completed_at
       FROM tasks";

fn map_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        note_id: row.get(1)?,
        title: row.get(2)?,
        status: TaskStatus::from_stored(&row.get::<_, String>(3)?),
        position: row.get(4)?,
        completed_at: row.get::<_, Option<i64>>(5)?.map(Timestamp::from_millis),
    })
}

impl TaskRepository for SqliteTaskRepository {
    fn list_for_note(&self, note_id: NoteId) -> AppResult<Vec<Task>> {
        self.database.with_connection(|connection| {
            let mut statement = connection
                .prepare(&format!(
                    "{SELECT_TASK}
                      WHERE note_id = ?1 AND deleted_at IS NULL
                      ORDER BY position, created_at"
                ))
                .map_err(AppError::from)?;
            let rows = statement
                .query_map([note_id], map_task)
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;
            Ok(rows)
        })
    }

    fn create_for_note(&self, note_id: NoteId, title: &str, now: Timestamp) -> AppResult<Task> {
        let title = validate_title(title)?;
        self.database.in_transaction(|transaction| {
            // Appended rather than inserted at the top: a checklist is read in
            // the order it was written.
            let position: i64 = transaction
                .query_row(
                    "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE note_id = ?1",
                    [note_id],
                    |row| row.get(0),
                )
                .map_err(AppError::from)?;

            let id = TaskId::new();
            transaction
                .execute(
                    "INSERT INTO tasks (id, note_id, title, status, position, created_at, updated_at)
                          VALUES (?1, ?2, ?3, 'inbox', ?4, ?5, ?5)",
                    params![id, note_id, title, position, now.as_millis()],
                )
                .map_err(AppError::from)?;

            transaction
                .query_row(&format!("{SELECT_TASK} WHERE id = ?1"), [id], map_task)
                .map_err(AppError::from)
        })
    }

    fn set_completed(&self, id: TaskId, completed: bool, now: Timestamp) -> AppResult<Task> {
        self.database.in_transaction(|transaction| {
            // The schema insists a completed task records when and an open one
            // does not, so both columns move together or the CHECK refuses.
            let changed = transaction
                .execute(
                    "UPDATE tasks
                        SET status = ?1, completed_at = ?2, updated_at = ?3
                      WHERE id = ?4 AND deleted_at IS NULL",
                    params![
                        if completed { "completed" } else { "inbox" },
                        if completed {
                            Some(now.as_millis())
                        } else {
                            None
                        },
                        now.as_millis(),
                        id,
                    ],
                )
                .map_err(AppError::from)?;
            if changed == 0 {
                return Err(AppError::Database(DatabaseError::NotFound {
                    entity: "task",
                    id: id.to_string(),
                }));
            }

            transaction
                .query_row(&format!("{SELECT_TASK} WHERE id = ?1"), [id], map_task)
                .map_err(AppError::from)
        })
    }

    fn delete(&self, id: TaskId, now: Timestamp) -> AppResult<()> {
        self.database.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE tasks SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
                    params![now.as_millis(), id],
                )
                .map(|_| ())
                .map_err(AppError::from)
        })
    }

    fn delete_for_note(&self, note_id: NoteId, now: Timestamp) -> AppResult<u32> {
        self.database.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE tasks SET deleted_at = ?1, updated_at = ?1
                      WHERE note_id = ?2 AND deleted_at IS NULL",
                    params![now.as_millis(), note_id],
                )
                .map(|rows| u32::try_from(rows).unwrap_or(u32::MAX))
                .map_err(AppError::from)
        })
    }

    fn progress_for_note(&self, note_id: NoteId) -> AppResult<TaskProgress> {
        self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(status = 'completed'), 0)
                       FROM tasks
                      WHERE note_id = ?1 AND deleted_at IS NULL",
                    [note_id],
                    |row| {
                        Ok(TaskProgress {
                            total: row.get(0)?,
                            completed: row.get(1)?,
                        })
                    },
                )
                .map_err(AppError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::clock::FixedClock;
    use crate::domain::notes::{NoteDraft, NoteRepository};
    use crate::infrastructure::sqlite::SqliteNoteRepository;

    use super::*;

    fn fixture() -> (SqliteTaskRepository, NoteId, Timestamp) {
        let now = Timestamp::from_millis(1_000);
        let clock: crate::domain::clock::SharedClock = Arc::new(FixedClock::new(now));
        let database = Arc::new(Database::open_in_memory(1_000).expect("opens"));
        let note = SqliteNoteRepository::new(Arc::clone(&database), clock)
            .create(NoteDraft {
                title: Some("Заметка".into()),
                ..NoteDraft::default()
            })
            .expect("creates note");
        (SqliteTaskRepository::new(database), note.id, now)
    }

    #[test]
    fn tasks_come_back_in_the_order_they_were_written() {
        let (repository, note, now) = fixture();
        repository
            .create_for_note(note, "первая", now)
            .expect("creates");
        repository
            .create_for_note(note, "вторая", now)
            .expect("creates");

        let listed = repository.list_for_note(note).expect("reads");

        assert_eq!(
            listed
                .iter()
                .map(|task| task.title.as_str())
                .collect::<Vec<_>>(),
            ["первая", "вторая"]
        );
    }

    #[test]
    fn ticking_a_task_records_when_and_unticking_forgets_it() {
        let (repository, note, now) = fixture();
        let task = repository
            .create_for_note(note, "купить хлеб", now)
            .expect("creates");

        let done = repository.set_completed(task.id, true, now).expect("ticks");
        assert!(done.status.is_completed());
        assert_eq!(done.completed_at, Some(now));

        let open = repository
            .set_completed(task.id, false, now)
            .expect("unticks");
        assert!(!open.status.is_completed());
        assert_eq!(
            open.completed_at, None,
            "an open task must not claim it was finished"
        );
    }

    #[test]
    fn progress_counts_only_the_tasks_that_are_still_there() {
        let (repository, note, now) = fixture();
        let first = repository
            .create_for_note(note, "первая", now)
            .expect("creates");
        let second = repository
            .create_for_note(note, "вторая", now)
            .expect("creates");
        repository
            .create_for_note(note, "третья", now)
            .expect("creates");
        repository
            .set_completed(first.id, true, now)
            .expect("ticks");
        repository.delete(second.id, now).expect("deletes");

        let progress = repository.progress_for_note(note).expect("reads");

        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 1);
    }

    #[test]
    fn a_deleted_task_leaves_the_list() {
        let (repository, note, now) = fixture();
        let task = repository
            .create_for_note(note, "первая", now)
            .expect("creates");

        repository.delete(task.id, now).expect("deletes");

        assert!(repository.list_for_note(note).expect("reads").is_empty());
    }

    #[test]
    fn ticking_a_task_that_is_not_there_says_so() {
        let (repository, _note, now) = fixture();
        let error = repository
            .set_completed(TaskId::new(), true, now)
            .expect_err("must fail");
        assert_eq!(error.code(), "not_found");
    }

    #[test]
    fn a_task_without_a_title_is_refused() {
        let (repository, note, now) = fixture();
        let error = repository
            .create_for_note(note, "   ", now)
            .expect_err("must refuse");
        assert_eq!(error.code(), "validation_required");
    }
}
