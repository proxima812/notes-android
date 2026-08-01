//! Persistence boundary for reminders.

use crate::domain::clock::Timestamp;
use crate::domain::ids::NoteId;
use crate::error::AppResult;

use super::{ReminderDraft, ScheduledReminder};

pub trait ReminderRepository: Send + Sync {
    fn find_active_for_note(
        &self,
        note_id: NoteId,
        now: Timestamp,
    ) -> AppResult<Option<ScheduledReminder>>;

    /// Writes the next state and calls `schedule` before committing it.
    fn upsert_for_note(
        &self,
        draft: ReminderDraft,
        schedule: &mut dyn FnMut(Option<&ScheduledReminder>, &ScheduledReminder) -> AppResult<bool>,
    ) -> AppResult<ScheduledReminder>;

    /// Cancels the platform alarm before committing the soft delete.
    fn delete_for_note(
        &self,
        note_id: NoteId,
        cancel: &mut dyn FnMut(&ScheduledReminder) -> AppResult<()>,
    ) -> AppResult<Option<ScheduledReminder>>;

    fn default_sound_id(&self) -> AppResult<Option<String>>;
}
