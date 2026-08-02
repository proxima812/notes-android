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
