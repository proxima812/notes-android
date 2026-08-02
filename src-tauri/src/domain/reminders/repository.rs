//! Persistence boundary for reminders.

use crate::domain::clock::Timestamp;
use crate::domain::ids::NoteId;
use crate::error::AppResult;

use super::{Reminder, ReminderDraft, ScheduledReminder};

/// A repeating reminder whose armed window has run down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThinWindow {
    pub reminder: Reminder,
    /// Instants already armed, so the caller only adds what is missing.
    pub armed: Vec<Timestamp>,
}

pub trait ReminderRepository: Send + Sync {
    /// The nearest firing still ahead of `now`, if any.
    fn find_active_for_note(
        &self,
        note_id: NoteId,
        now: Timestamp,
    ) -> AppResult<Option<ScheduledReminder>>;

    /// Replaces the note's reminder and everything armed for it.
    ///
    /// `arm` is called for each new occurrence and reports whether the platform
    /// gave an exact alarm; `disarm` takes back the occurrences being replaced.
    /// Arming happens before anything is taken back, so a failure part-way
    /// leaves the reminder the user already had still armed.
    fn upsert_for_note(
        &self,
        draft: ReminderDraft,
        arm: &mut dyn FnMut(&ScheduledReminder) -> AppResult<bool>,
        disarm: &mut dyn FnMut(i32) -> AppResult<()>,
    ) -> AppResult<ScheduledReminder>;

    /// Cancels the note's reminder and every occurrence armed for it.
    fn delete_for_note(
        &self,
        note_id: NoteId,
        disarm: &mut dyn FnMut(i32) -> AppResult<()>,
    ) -> AppResult<Option<ScheduledReminder>>;

    /// Moves an occurrence to another instant, recording the zone it now means.
    ///
    /// Used when the device has changed zone: the wall-clock time the user
    /// asked for stays, and the instant that renders it moves.
    fn retime(
        &self,
        scheduled: &ScheduledReminder,
        at: Timestamp,
        zone: &str,
    ) -> AppResult<ScheduledReminder>;

    /// Every occurrence that is still due, across all notes.
    ///
    /// Restoring a backup replaces the whole set, so re-arming has to start
    /// from what the database now says rather than from what was armed before.
    fn active_scheduled(&self, now: Timestamp) -> AppResult<Vec<ScheduledReminder>>;

    /// Marks occurrences whose time has passed as fired.
    ///
    /// Nothing tells the core that an alarm went off — the notification is
    /// posted by a broadcast receiver that may run with the process dead — so
    /// elapsed occurrences are only recognised the next time anyone looks.
    fn mark_elapsed(&self, now: Timestamp) -> AppResult<u32>;

    /// Repeating reminders with fewer than `target` firings still armed.
    fn thin_windows(&self, now: Timestamp, target: usize) -> AppResult<Vec<ThinWindow>>;

    /// Arms the instants of `reminder` that are not armed already.
    fn extend_window(
        &self,
        reminder: &Reminder,
        instants: &[Timestamp],
        arm: &mut dyn FnMut(&ScheduledReminder) -> AppResult<bool>,
    ) -> AppResult<u32>;

    fn default_sound_id(&self) -> AppResult<Option<String>>;

    /// The stored preset times, still in the form they were written in.
    ///
    /// `None` means the user has never edited the set, which is what separates
    /// "use the shipped times" from "the user deleted every one of them".
    fn time_presets(&self) -> AppResult<Option<String>>;

    fn set_time_presets(&self, raw: &str) -> AppResult<()>;
}
