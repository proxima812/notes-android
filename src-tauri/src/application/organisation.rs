//! Filing notes: folders and tags.

use std::sync::Arc;

use crate::domain::clock::SharedClock;
use crate::domain::ids::{FolderId, NoteId, TagId};
use crate::domain::organisation::{Folder, OrganisationRepository, Tag};
use crate::error::AppResult;

pub struct OrganisationUseCases {
    organisation: Arc<dyn OrganisationRepository>,
    clock: SharedClock,
}

impl OrganisationUseCases {
    #[must_use]
    pub fn new(organisation: Arc<dyn OrganisationRepository>, clock: SharedClock) -> Self {
        Self {
            organisation,
            clock,
        }
    }

    /// # Errors
    /// Fails on a database error.
    pub fn tags(&self) -> AppResult<Vec<Tag>> {
        self.organisation.tags()
    }

    /// # Errors
    /// Fails on validation or a database error.
    pub fn ensure_tag(&self, name: &str) -> AppResult<Tag> {
        self.organisation.ensure_tag(name, self.clock.now())
    }

    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn delete_tag(&self, id: &str) -> AppResult<()> {
        self.organisation.delete_tag(TagId::parse(id)?)
    }

    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn tags_of_note(&self, note_id: &str) -> AppResult<Vec<Tag>> {
        self.organisation.tags_of_note(NoteId::parse(note_id)?)
    }

    /// Replaces the whole set of tags on a note.
    ///
    /// The set is sent rather than a change to it: adding and removing are the
    /// same edit from the core's side, and a list that arrives whole cannot go
    /// out of step with the one the user is looking at.
    ///
    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn set_note_tags(&self, note_id: &str, tags: &[String]) -> AppResult<Vec<Tag>> {
        let note_id = NoteId::parse(note_id)?;
        let ids = tags
            .iter()
            .map(|id| TagId::parse(id))
            .collect::<AppResult<Vec<_>>>()?;
        self.organisation
            .set_note_tags(note_id, &ids, self.clock.now())?;
        self.organisation.tags_of_note(note_id)
    }

    /// # Errors
    /// Fails on a database error.
    pub fn folders(&self) -> AppResult<Vec<Folder>> {
        self.organisation.folders()
    }

    /// # Errors
    /// Fails on validation or a database error.
    pub fn create_folder(&self, name: &str) -> AppResult<Folder> {
        self.organisation.create_folder(name, self.clock.now())
    }

    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn delete_folder(&self, id: &str) -> AppResult<()> {
        self.organisation.delete_folder(FolderId::parse(id)?)
    }

    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn folders_of_note(&self, note_id: &str) -> AppResult<Vec<Folder>> {
        self.organisation.folders_of_note(NoteId::parse(note_id)?)
    }

    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn set_note_folders(&self, note_id: &str, folders: &[String]) -> AppResult<Vec<Folder>> {
        let note_id = NoteId::parse(note_id)?;
        let ids = folders
            .iter()
            .map(|id| FolderId::parse(id))
            .collect::<AppResult<Vec<_>>>()?;
        self.organisation
            .set_note_folders(note_id, &ids, self.clock.now())?;
        self.organisation.folders_of_note(note_id)
    }
}
