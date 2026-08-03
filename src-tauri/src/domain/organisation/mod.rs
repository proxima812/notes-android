//! Folders and tags: the two ways a note can be put somewhere.
//!
//! Both are flat here. The schema allows nested folders, and the column stays,
//! but a hierarchy the interface cannot show would be a feature only the
//! database knows about — so nesting waits until there is a screen for it.
//!
//! Filtering the library by either is already the note repository's job; this
//! module is about the labels themselves and which notes wear them.

use crate::domain::clock::Timestamp;
use crate::domain::ids::{FolderId, NoteId, TagId};
use crate::error::{AppError, AppResult, ValidationError};

/// Longest a label may be. Long enough for a sentence fragment, short enough to
/// stay readable as a chip.
pub const MAX_LABEL: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
    /// How many notes wear it, for showing the useful ones first.
    pub usage_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub id: FolderId,
    pub name: String,
    pub note_count: i64,
}

/// Trims a label and refuses the ones that cannot be shown.
///
/// # Errors
/// Returns a validation error for an empty label or one over [`MAX_LABEL`].
pub fn validate_label(raw: &str, field: &'static str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(ValidationError::Required { field }));
    }
    if trimmed.chars().count() > MAX_LABEL {
        return Err(AppError::Validation(ValidationError::TooLong {
            field,
            max: MAX_LABEL,
        }));
    }
    Ok(trimmed.to_owned())
}

pub trait OrganisationRepository: Send + Sync {
    /// Every tag, most used first.
    ///
    /// # Errors
    /// Fails on a database error.
    fn tags(&self) -> AppResult<Vec<Tag>>;

    /// Finds a tag by name or creates it.
    ///
    /// Names are matched case-insensitively, so `#Работа` and `#работа` are one
    /// tag rather than two that look identical in a list.
    ///
    /// # Errors
    /// Fails on validation or a database error.
    fn ensure_tag(&self, name: &str, now: Timestamp) -> AppResult<Tag>;

    /// Deletes a tag, taking it off every note that wore it.
    ///
    /// # Errors
    /// Fails if no such tag exists, or on a database error.
    fn delete_tag(&self, id: TagId) -> AppResult<()>;

    /// # Errors
    /// Fails on a database error.
    fn tags_of_note(&self, note_id: NoteId) -> AppResult<Vec<Tag>>;

    /// Replaces the whole set of tags on a note.
    ///
    /// Every tag in the set must already exist; the set is written whole or not
    /// at all.
    ///
    /// # Errors
    /// Fails if any of the tags does not exist, or on a database error.
    fn set_note_tags(&self, note_id: NoteId, tags: &[TagId], now: Timestamp) -> AppResult<()>;

    /// Every folder, by name.
    ///
    /// # Errors
    /// Fails on a database error.
    fn folders(&self) -> AppResult<Vec<Folder>>;

    /// # Errors
    /// Fails on validation or a database error.
    fn create_folder(&self, name: &str, now: Timestamp) -> AppResult<Folder>;

    /// Deletes a folder, leaving the notes that were in it where they are.
    ///
    /// # Errors
    /// Fails if no such folder exists, or on a database error.
    fn delete_folder(&self, id: FolderId) -> AppResult<()>;

    /// # Errors
    /// Fails on a database error.
    fn folders_of_note(&self, note_id: NoteId) -> AppResult<Vec<Folder>>;

    /// Replaces the whole set of folders a note is filed under.
    ///
    /// Every folder in the set must already exist; the set is written whole or
    /// not at all.
    ///
    /// # Errors
    /// Fails if any of the folders does not exist, or on a database error.
    fn set_note_folders(
        &self,
        note_id: NoteId,
        folders: &[FolderId],
        now: Timestamp,
    ) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_is_trimmed_rather_than_stored_with_its_spaces() {
        assert_eq!(
            validate_label("  работа  ", "name").expect("valid"),
            "работа"
        );
    }

    #[test]
    fn a_label_of_only_spaces_is_empty() {
        let error = validate_label("   ", "name").expect_err("must refuse");
        assert_eq!(error.code(), "validation_required");
    }

    #[test]
    fn a_label_longer_than_a_chip_can_show_is_refused() {
        let error = validate_label(&"я".repeat(MAX_LABEL + 1), "name").expect_err("must refuse");
        assert_eq!(error.code(), "validation_too_long");
    }

    #[test]
    fn the_limit_counts_characters_rather_than_bytes() {
        // Cyrillic is two bytes a letter; a byte limit would cut these in half.
        validate_label(&"я".repeat(MAX_LABEL), "name").expect("fits");
    }
}
