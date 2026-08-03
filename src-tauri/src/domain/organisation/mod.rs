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

/// A label the user typed, trimmed and accepted, alongside the key it is
/// matched by.
///
/// A repository is handed one of these rather than a `&str` so that neither the
/// validation nor the case folding can belong to a single implementation: there
/// is no way to reach the store with a name that skipped either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelName {
    display: String,
    folded: String,
}

impl LabelName {
    /// Trims a label and refuses the ones that cannot be shown.
    ///
    /// # Errors
    /// Returns a validation error for an empty label or one over [`MAX_LABEL`].
    pub fn new(raw: &str, field: &'static str) -> AppResult<Self> {
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
        Ok(Self {
            folded: trimmed.to_lowercase(),
            display: trimmed.to_owned(),
        })
    }

    /// The label as it is stored and shown, in the case it was typed in.
    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }

    /// The key two labels are compared by.
    ///
    /// Folded with Rust's Unicode-aware lowercasing rather than SQLite's
    /// `NOCASE`, which only folds ASCII and would let `Работа` and `работа`
    /// become two labels that look identical in every list the user sees.
    #[must_use]
    pub fn folded(&self) -> &str {
        &self.folded
    }
}

pub trait OrganisationRepository: Send + Sync {
    /// Every tag, most used first.
    ///
    /// # Errors
    /// Fails on a database error.
    fn tags(&self) -> AppResult<Vec<Tag>>;

    /// Finds a tag by name or creates it.
    ///
    /// Implementations must look the tag up by [`LabelName::folded`], so that
    /// `#Работа` and `#работа` are one tag rather than two that look identical
    /// in a list, and must store [`LabelName::display`] as its name.
    ///
    /// # Errors
    /// Fails on a database error.
    fn ensure_tag(&self, name: &LabelName, now: Timestamp) -> AppResult<Tag>;

    /// # Errors
    /// Fails on a database error.
    fn delete_tag(&self, id: TagId) -> AppResult<()>;

    /// # Errors
    /// Fails on a database error.
    fn tags_of_note(&self, note_id: NoteId) -> AppResult<Vec<Tag>>;

    /// Replaces the whole set of tags on a note.
    ///
    /// The slice is a set: callers hand over each id once, and an id repeated
    /// anyway means one link rather than an error, because a list that says the
    /// same thing twice still says something the user can be given.
    ///
    /// # Errors
    /// Fails on a database error.
    fn set_note_tags(&self, note_id: NoteId, tags: &[TagId], now: Timestamp) -> AppResult<()>;

    /// Every folder, by name.
    ///
    /// # Errors
    /// Fails on a database error.
    fn folders(&self) -> AppResult<Vec<Folder>>;

    /// Creates a folder under the given name.
    ///
    /// Unlike a tag, a folder is not looked up first: two drawers may fairly
    /// carry the same label, and it is the user who decides they are the same.
    ///
    /// # Errors
    /// Fails on a database error.
    fn create_folder(&self, name: &LabelName, now: Timestamp) -> AppResult<Folder>;

    /// # Errors
    /// Fails on a database error.
    fn delete_folder(&self, id: FolderId) -> AppResult<()>;

    /// # Errors
    /// Fails on a database error.
    fn folders_of_note(&self, note_id: NoteId) -> AppResult<Vec<Folder>>;

    /// Replaces the whole set of folders a note is filed under.
    ///
    /// The slice is a set, on the same terms as [`Self::set_note_tags`].
    ///
    /// # Errors
    /// Fails on a database error.
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
            LabelName::new("  работа  ", "name")
                .expect("valid")
                .display(),
            "работа"
        );
    }

    #[test]
    fn a_label_of_only_spaces_is_empty() {
        let error = LabelName::new("   ", "name").expect_err("must refuse");
        assert_eq!(error.code(), "validation_required");
    }

    #[test]
    fn a_label_longer_than_a_chip_can_show_is_refused() {
        let error = LabelName::new(&"я".repeat(MAX_LABEL + 1), "name").expect_err("must refuse");
        assert_eq!(error.code(), "validation_too_long");
    }

    #[test]
    fn the_limit_counts_characters_rather_than_bytes() {
        // Cyrillic is two bytes a letter; a byte limit would cut these in half.
        LabelName::new(&"я".repeat(MAX_LABEL), "name").expect("fits");
    }

    #[test]
    fn a_label_keeps_the_case_it_was_typed_in_and_matches_without_it() {
        let typed = LabelName::new("Работа", "name").expect("valid");

        assert_eq!(typed.display(), "Работа", "shown as the user wrote it");
        assert_eq!(
            typed.folded(),
            LabelName::new("работа", "name").expect("valid").folded(),
            "and matched as if they had written it either way"
        );
    }

    #[test]
    fn folding_reaches_beyond_ascii() {
        // The reason folding is not left to SQLite's NOCASE, which stops here.
        assert_eq!(LabelName::new("ЁЖ", "name").expect("valid").folded(), "ёж");
    }
}
