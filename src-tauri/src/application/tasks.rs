//! The checklist on a note.

use std::sync::Arc;

use crate::domain::clock::SharedClock;
use crate::domain::ids::{NoteId, TaskId};
use crate::domain::tasks::{Task, TaskProgress, TaskRepository};
use crate::error::AppResult;

pub struct TaskUseCases {
    tasks: Arc<dyn TaskRepository>,
    clock: SharedClock,
}

impl TaskUseCases {
    #[must_use]
    pub fn new(tasks: Arc<dyn TaskRepository>, clock: SharedClock) -> Self {
        Self { tasks, clock }
    }

    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn list_for_note(&self, note_id: &str) -> AppResult<Vec<Task>> {
        self.tasks.list_for_note(NoteId::parse(note_id)?)
    }

    /// # Errors
    /// Fails on validation, a malformed identifier, or a database error.
    pub fn create_for_note(&self, note_id: &str, title: &str) -> AppResult<Task> {
        self.tasks
            .create_for_note(NoteId::parse(note_id)?, title, self.clock.now())
    }

    /// # Errors
    /// Fails when the task is missing, for a malformed identifier, or on a
    /// database error.
    pub fn set_completed(&self, id: &str, completed: bool) -> AppResult<Task> {
        self.tasks
            .set_completed(TaskId::parse(id)?, completed, self.clock.now())
    }

    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn delete(&self, id: &str) -> AppResult<()> {
        self.tasks.delete(TaskId::parse(id)?, self.clock.now())
    }

    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn progress_for_note(&self, note_id: &str) -> AppResult<TaskProgress> {
        self.tasks.progress_for_note(NoteId::parse(note_id)?)
    }
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use crate::domain::clock::{FixedClock, Timestamp};
    use crate::domain::tasks::{validate_title, TaskStatus};

    use super::*;

    const NOW: i64 = 1_700_000_000_000;
    const NOTE: &str = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e6f";
    const TASK: &str = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e70";

    /// What the layer under test asked storage to do, in the order it asked.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        List(NoteId),
        Create {
            note_id: NoteId,
            title: String,
            now: Timestamp,
        },
        SetCompleted {
            id: TaskId,
            completed: bool,
        },
        Delete(TaskId),
        Progress(NoteId),
    }

    #[derive(Default)]
    struct FakeTasks {
        calls: Mutex<Vec<Call>>,
        progress: Mutex<TaskProgress>,
    }

    impl FakeTasks {
        fn calls(&self) -> Vec<Call> {
            self.calls.lock().clone()
        }
    }

    impl TaskRepository for FakeTasks {
        fn list_for_note(&self, note_id: NoteId) -> AppResult<Vec<Task>> {
            self.calls.lock().push(Call::List(note_id));
            Ok(Vec::new())
        }

        fn create_for_note(&self, note_id: NoteId, title: &str, now: Timestamp) -> AppResult<Task> {
            // The real repository validates here, not in the use case; the
            // fake keeps that arrangement so the tests can tell where a
            // rejected title is turned away.
            let title = validate_title(title)?;
            self.calls.lock().push(Call::Create {
                note_id,
                title: title.clone(),
                now,
            });
            Ok(Task {
                id: TaskId::new(),
                note_id: Some(note_id),
                title,
                status: TaskStatus::Open,
                position: 0,
                completed_at: None,
            })
        }

        fn set_completed(&self, id: TaskId, completed: bool, now: Timestamp) -> AppResult<Task> {
            self.calls.lock().push(Call::SetCompleted { id, completed });
            Ok(Task {
                id,
                note_id: None,
                title: "тест".to_owned(),
                status: if completed {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Open
                },
                position: 0,
                completed_at: completed.then_some(now),
            })
        }

        fn delete(&self, id: TaskId, _now: Timestamp) -> AppResult<()> {
            self.calls.lock().push(Call::Delete(id));
            Ok(())
        }

        fn progress_for_note(&self, note_id: NoteId) -> AppResult<TaskProgress> {
            self.calls.lock().push(Call::Progress(note_id));
            Ok(*self.progress.lock())
        }
    }

    fn fixture() -> (TaskUseCases, Arc<FakeTasks>) {
        let tasks = Arc::new(FakeTasks::default());
        let clock = Arc::new(FixedClock::new(Timestamp::from_millis(NOW)));
        (
            TaskUseCases::new(Arc::clone(&tasks) as Arc<dyn TaskRepository>, clock),
            tasks,
        )
    }

    #[test]
    fn a_note_identifier_that_is_not_an_id_never_reaches_storage() {
        let (use_cases, tasks) = fixture();

        for error in [
            use_cases.list_for_note("note-1").expect_err("must refuse"),
            use_cases
                .create_for_note("note-1", "купить хлеб")
                .expect_err("must refuse"),
            use_cases
                .progress_for_note("note-1")
                .expect_err("must refuse"),
        ] {
            assert_eq!(error.code(), "validation_invalid");
        }

        assert!(
            tasks.calls().is_empty(),
            "a malformed id must be turned away before any database work begins"
        );
    }

    #[test]
    fn a_task_identifier_that_is_not_an_id_never_reaches_storage() {
        let (use_cases, tasks) = fixture();

        for error in [
            use_cases.set_completed("", true).expect_err("must refuse"),
            use_cases.delete("42").expect_err("must refuse"),
        ] {
            assert_eq!(error.code(), "validation_invalid");
        }

        assert!(tasks.calls().is_empty());
    }

    #[test]
    fn ticking_a_task_reaches_storage_as_completed() {
        let (use_cases, tasks) = fixture();

        let task = use_cases.set_completed(TASK, true).expect("ticks");

        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(
            tasks.calls(),
            vec![Call::SetCompleted {
                id: TaskId::parse(TASK).expect("valid id"),
                completed: true,
            }]
        );
    }

    #[test]
    fn unticking_a_task_reaches_storage_as_open() {
        let (use_cases, tasks) = fixture();

        let task = use_cases.set_completed(TASK, false).expect("unticks");

        assert_eq!(task.status, TaskStatus::Open);
        assert_eq!(
            tasks.calls(),
            vec![Call::SetCompleted {
                id: TaskId::parse(TASK).expect("valid id"),
                completed: false,
            }],
            "changing one's mind must be passed on as plainly as ticking was"
        );
    }

    #[test]
    fn a_new_task_carries_the_title_and_the_current_instant() {
        let (use_cases, tasks) = fixture();

        use_cases
            .create_for_note(NOTE, "купить хлеб")
            .expect("creates");

        assert_eq!(
            tasks.calls(),
            vec![Call::Create {
                note_id: NoteId::parse(NOTE).expect("valid id"),
                title: "купить хлеб".to_owned(),
                now: Timestamp::from_millis(NOW),
            }]
        );
    }

    #[test]
    fn a_title_of_only_spaces_is_left_for_the_domain_to_refuse() {
        let (use_cases, tasks) = fixture();

        let error = use_cases
            .create_for_note(NOTE, "   ")
            .expect_err("must refuse");

        assert_eq!(error.code(), "validation_required");
        assert!(
            tasks.calls().is_empty(),
            "the use case passes the title on untouched, so the refusal is the domain's"
        );
    }

    #[test]
    fn deleting_a_task_reaches_storage() {
        let (use_cases, tasks) = fixture();

        use_cases.delete(TASK).expect("deletes");

        assert_eq!(
            tasks.calls(),
            vec![Call::Delete(TaskId::parse(TASK).expect("valid id"))]
        );
    }

    #[test]
    fn listing_asks_storage_for_the_note_that_was_named() {
        let (use_cases, tasks) = fixture();

        use_cases.list_for_note(NOTE).expect("lists");

        assert_eq!(
            tasks.calls(),
            vec![Call::List(NoteId::parse(NOTE).expect("valid id"))]
        );
    }

    #[test]
    fn progress_is_reported_as_storage_counted_it() {
        let (use_cases, tasks) = fixture();
        *tasks.progress.lock() = TaskProgress {
            total: 7,
            completed: 3,
        };

        let progress = use_cases.progress_for_note(NOTE).expect("counts");

        assert_eq!(progress.total, 7);
        assert_eq!(progress.completed, 3);
    }
}
