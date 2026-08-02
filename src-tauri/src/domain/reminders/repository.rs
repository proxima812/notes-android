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

    /// Every reminder that is still due, across all notes.
    ///
    /// Restoring a backup replaces the whole set, so re-arming has to start
    /// from what the database now says rather than from what was armed before.
    fn active_scheduled(&self, now: Timestamp) -> AppResult<Vec<ScheduledReminder>>;

    fn default_sound_id(&self) -> AppResult<Option<String>>;

    /// The stored preset times, still in the form they were written in.
    ///
    /// `None` means the user has never edited the set, which is what separates
    /// "use the shipped times" from "the user deleted every one of them".
    fn time_presets(&self) -> AppResult<Option<String>>;

    fn set_time_presets(&self, raw: &str) -> AppResult<()>;
}
