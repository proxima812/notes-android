//! Things to do, attached to a note.
//!
//! A checklist item in the editor is formatting; a task is a row. The
//! difference is what you can do afterwards — count them, filter by them, tell
//! at a glance how much of a note is still open — none of which works when the
//! only record is a tick painted in rich text.
//!
//! The schema carries far more than this: priorities, dependencies, estimates,
//! recurrence. Only what the interface can show is modelled, so nothing here
//! is a promise the app does not keep.

use crate::domain::clock::Timestamp;
use crate::domain::ids::{NoteId, TaskId};
use crate::error::{AppError, AppResult, ValidationError};

/// Longest a task title may be, matching what a single line can show.
pub const MAX_TITLE: usize = 200;

/// The two states the interface offers.
///
/// The column allows six; the others are written by nothing and read as open,
/// so a database from a later build still opens in this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Open,
    Completed,
}

impl TaskStatus {
    #[must_use]
    pub const fn stored(self) -> &'static str {
        match self {
            Self::Open => "inbox",
            Self::Completed => "completed",
        }
    }

    #[must_use]
    pub fn from_stored(raw: &str) -> Self {
        if raw == "completed" {
            Self::Completed
        } else {
            Self::Open
        }
    }

    #[must_use]
    pub const fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub note_id: Option<NoteId>,
    pub title: String,
    pub status: TaskStatus,
    /// Order the user arranged by hand.
    pub position: i64,
    pub completed_at: Option<Timestamp>,
}

/// How much of a note's checklist is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskProgress {
    pub total: i64,
    pub completed: i64,
}

/// # Errors
/// Returns a validation error for an empty title or one over [`MAX_TITLE`].
pub fn validate_title(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(ValidationError::Required {
            field: "title",
        }));
    }
    if trimmed.chars().count() > MAX_TITLE {
        return Err(AppError::Validation(ValidationError::TooLong {
            field: "title",
            max: MAX_TITLE,
        }));
    }
    Ok(trimmed.to_owned())
}

pub trait TaskRepository: Send + Sync {
    /// Every task on a note, in the order the user arranged.
    ///
    /// # Errors
    /// Fails on a database error.
    fn list_for_note(&self, note_id: NoteId) -> AppResult<Vec<Task>>;

    /// # Errors
    /// Fails on validation or a database error.
    fn create_for_note(&self, note_id: NoteId, title: &str, now: Timestamp) -> AppResult<Task>;

    /// Ticks or unticks a task.
    ///
    /// # Errors
    /// Fails when the task is missing or on a database error.
    fn set_completed(&self, id: TaskId, completed: bool, now: Timestamp) -> AppResult<Task>;

    /// # Errors
    /// Fails on a database error.
    fn delete(&self, id: TaskId, now: Timestamp) -> AppResult<()>;

    /// Takes the whole checklist off a note, answering how many rows went.
    ///
    /// One call rather than a delete per row: emptying a checklist someone
    /// opened by accident is a single act, and doing it row by row would leave
    /// half a list behind if the app were closed part-way.
    ///
    /// # Errors
    /// Fails on a database error.
    fn delete_for_note(&self, note_id: NoteId, now: Timestamp) -> AppResult<u32>;

    /// Totals for a note, for showing progress without loading every row.
    ///
    /// # Errors
    /// Fails on a database error.
    fn progress_for_note(&self, note_id: NoteId) -> AppResult<TaskProgress>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_trimmed() {
        assert_eq!(
            validate_title("  купить хлеб  ").expect("valid"),
            "купить хлеб"
        );
    }

    #[test]
    fn a_title_of_only_spaces_is_empty() {
        let error = validate_title("  ").expect_err("must refuse");
        assert_eq!(error.code(), "validation_required");
    }

    #[test]
    fn the_limit_counts_characters_rather_than_bytes() {
        validate_title(&"я".repeat(MAX_TITLE)).expect("fits");
        assert!(validate_title(&"я".repeat(MAX_TITLE + 1)).is_err());
    }

    #[test]
    fn the_two_states_survive_a_round_trip_through_the_column() {
        for status in [TaskStatus::Open, TaskStatus::Completed] {
            assert_eq!(TaskStatus::from_stored(status.stored()), status);
        }
    }

    #[test]
    fn a_state_from_a_later_build_reads_as_open_rather_than_failing() {
        // The column allows six; a row written by a build that offers more must
        // still be shown rather than making the whole list unreadable.
        assert_eq!(TaskStatus::from_stored("waiting"), TaskStatus::Open);
        assert_eq!(TaskStatus::from_stored("in_progress"), TaskStatus::Open);
    }
}
