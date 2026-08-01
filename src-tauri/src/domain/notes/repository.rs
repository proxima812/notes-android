//! Repository interface for notes.
//!
//! Use cases depend on this trait, never on `rusqlite`. That keeps SQL out of
//! the application layer and makes the use cases testable against an in-memory
//! double without a database.

use serde::{Deserialize, Serialize};

use crate::domain::ids::{FolderId, NoteId, TagId};
use crate::error::AppResult;

use super::{Note, NoteDraft, NotePatch};

/// Which slice of the library a query addresses. These are mutually exclusive
/// by design: a note in the trash never shows up in the archive list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteScope {
    /// Not deleted, not archived. The default list.
    #[default]
    Active,
    Archived,
    Trash,
    Favorites,
    /// Everything except the trash.
    AllExceptTrash,
}

#[derive(Debug, Clone, Default)]
pub struct NoteFilter {
    pub scope: NoteScope,
    pub folder_id: Option<FolderId>,
    pub tag_id: Option<TagId>,
    pub note_type: Option<super::NoteType>,
    pub pinned_only: bool,
    /// Inclusive lower bound on `updated_at`.
    pub updated_after: Option<crate::domain::clock::Timestamp>,
    /// Inclusive upper bound on `updated_at`.
    pub updated_before: Option<crate::domain::clock::Timestamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteSort {
    /// Pinned first, then most recently updated. What the main screen uses.
    #[default]
    PinnedThenUpdated,
    UpdatedDesc,
    UpdatedAsc,
    CreatedDesc,
    CreatedAsc,
    TitleAsc,
    TitleDesc,
    /// The order the user arranged by hand.
    Manual,
    /// Soonest upcoming reminder first; notes without one sort last.
    NextReminder,
}

/// Keyset-free offset pagination. Offsets are fine here because the UI only
/// ever walks forward through a virtualised list a page at a time.
#[derive(Debug, Clone, Copy)]
pub struct PageRequest {
    pub limit: u32,
    pub offset: u32,
}

impl PageRequest {
    /// Ceiling on how much a single command may return, so a bad caller cannot
    /// pull ten thousand notes through JSON at once.
    pub const MAX_LIMIT: u32 = 200;

    #[must_use]
    pub fn new(limit: u32, offset: u32) -> Self {
        Self {
            limit: limit.clamp(1, Self::MAX_LIMIT),
            offset,
        }
    }
}

impl Default for PageRequest {
    fn default() -> Self {
        Self::new(50, 0)
    }
}

#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Total matching rows, so the UI can size the scrollbar without fetching.
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}

impl<T> Page<T> {
    #[must_use]
    pub fn has_more(&self) -> bool {
        self.offset.saturating_add(self.limit) < self.total
    }
}

/// Persistence for notes.
pub trait NoteRepository: Send + Sync {
    /// Inserts a new note and returns it as stored.
    ///
    /// # Errors
    /// Fails on a database error or a validation failure.
    fn create(&self, draft: NoteDraft) -> AppResult<Note>;

    /// Returns a note whether or not it is in the trash.
    ///
    /// # Errors
    /// Fails on a database error.
    fn find(&self, id: NoteId) -> AppResult<Option<Note>>;

    /// Applies a partial update, bumping `revision` and `updated_at`.
    ///
    /// # Errors
    /// Fails when the note does not exist, is read-only, or on a database error.
    fn update(&self, id: NoteId, patch: NotePatch) -> AppResult<Note>;

    /// Moves a note to the trash. Reversible until the trash is emptied.
    ///
    /// # Errors
    /// Fails when the note does not exist, or on a database error.
    fn soft_delete(&self, id: NoteId) -> AppResult<()>;

    /// Brings a note back from the trash.
    ///
    /// # Errors
    /// Fails when the note does not exist, or on a database error.
    fn restore(&self, id: NoteId) -> AppResult<Note>;

    /// Deletes a note and its blocks for good.
    ///
    /// # Errors
    /// Fails when the note does not exist, or on a database error.
    fn purge(&self, id: NoteId) -> AppResult<()>;

    /// Empties the trash of everything deleted before `older_than`, returning
    /// how many notes went.
    ///
    /// # Errors
    /// Fails on a database error.
    fn purge_trash(&self, older_than: Option<crate::domain::clock::Timestamp>) -> AppResult<u32>;

    /// Lists notes matching `filter`, ordered by `sort`.
    ///
    /// # Errors
    /// Fails on a database error.
    fn list(&self, filter: &NoteFilter, sort: NoteSort, page: PageRequest)
        -> AppResult<Page<Note>>;

    /// Copies a note, including its blocks and tags, as a new untitled note.
    ///
    /// # Errors
    /// Fails when the source does not exist, or on a database error.
    fn duplicate(&self, id: NoteId) -> AppResult<Note>;

    /// Number of notes in the given scope.
    ///
    /// # Errors
    /// Fails on a database error.
    fn count(&self, filter: &NoteFilter) -> AppResult<u32>;
}
