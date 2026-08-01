//! Notes: the entity, its invariants, and the repository interface.
//!
//! The note type changes how the UI presents a note, never how it is stored:
//! one durable row shape serves all seventeen types, so switching a checklist
//! to a journal entry can never lose the text that was already written.

pub mod repository;

use serde::{Deserialize, Serialize};

use crate::domain::clock::Timestamp;
use crate::domain::ids::NoteId;
use crate::error::{AppError, AppResult, ValidationError};

pub use repository::{NoteFilter, NoteRepository, NoteScope, NoteSort, Page, PageRequest};

/// Longest accepted title. Generous for a heading, short enough that a runaway
/// paste cannot turn the note list into a wall of text.
pub const MAX_TITLE_LEN: usize = 500;

/// Longest accepted body, in characters. Roughly a 400-page book.
pub const MAX_CONTENT_LEN: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteType {
    Text,
    RichText,
    Checklist,
    TaskList,
    Journal,
    DailyNote,
    Meeting,
    Idea,
    Project,
    Contact,
    ShoppingList,
    Habit,
    PasswordHint,
    Bookmark,
    CodeSnippet,
    VoiceNote,
    Drawing,
}

impl NoteType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::RichText => "rich_text",
            Self::Checklist => "checklist",
            Self::TaskList => "task_list",
            Self::Journal => "journal",
            Self::DailyNote => "daily_note",
            Self::Meeting => "meeting",
            Self::Idea => "idea",
            Self::Project => "project",
            Self::Contact => "contact",
            Self::ShoppingList => "shopping_list",
            Self::Habit => "habit",
            Self::PasswordHint => "password_hint",
            Self::Bookmark => "bookmark",
            Self::CodeSnippet => "code_snippet",
            Self::VoiceNote => "voice_note",
            Self::Drawing => "drawing",
        }
    }

    /// Parses the stored discriminator.
    ///
    /// # Errors
    /// Returns [`ValidationError::Invalid`] for an unknown value, which means
    /// the row was written by a newer build or hand-edited.
    pub fn parse(value: &str) -> AppResult<Self> {
        let parsed = match value {
            "text" => Self::Text,
            "rich_text" => Self::RichText,
            "checklist" => Self::Checklist,
            "task_list" => Self::TaskList,
            "journal" => Self::Journal,
            "daily_note" => Self::DailyNote,
            "meeting" => Self::Meeting,
            "idea" => Self::Idea,
            "project" => Self::Project,
            "contact" => Self::Contact,
            "shopping_list" => Self::ShoppingList,
            "habit" => Self::Habit,
            "password_hint" => Self::PasswordHint,
            "bookmark" => Self::Bookmark,
            "code_snippet" => Self::CodeSnippet,
            "voice_note" => Self::VoiceNote,
            "drawing" => Self::Drawing,
            _ => {
                return Err(AppError::Validation(ValidationError::Invalid {
                    field: "note_type",
                }))
            }
        };
        Ok(parsed)
    }
}

/// A stored note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: NoteId,
    pub note_type: NoteType,
    pub title: String,
    /// Plain-text projection of the document: what FTS indexes and what
    /// Markdown export starts from.
    pub content_text: String,
    /// Rich document as JSON, when the type uses one.
    pub content_json: Option<String>,
    pub color: Option<String>,
    pub background: Option<String>,
    pub is_pinned: bool,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_readonly: bool,
    pub position: i64,
    pub word_count: i64,
    pub char_count: i64,
    pub revision: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deleted_at: Option<Timestamp>,
}

impl Note {
    #[must_use]
    pub const fn is_in_trash(&self) -> bool {
        self.deleted_at.is_some()
    }
}

/// Everything needed to create a note. Fields absent here get schema defaults.
#[derive(Debug, Clone, Default)]
pub struct NoteDraft {
    pub note_type: Option<NoteType>,
    pub title: Option<String>,
    pub content_text: Option<String>,
    pub content_json: Option<String>,
    pub color: Option<String>,
    pub background: Option<String>,
}

/// A partial update. `None` means "leave as is", which is what lets the editor
/// autosave just the body without touching pin or archive state.
#[derive(Debug, Clone, Default)]
pub struct NotePatch {
    pub note_type: Option<NoteType>,
    pub title: Option<String>,
    pub content_text: Option<String>,
    /// Outer `None` leaves the document alone; inner `None` clears it.
    pub content_json: Option<Option<String>>,
    pub color: Option<Option<String>>,
    pub background: Option<Option<String>>,
    pub is_pinned: Option<bool>,
    pub is_favorite: Option<bool>,
    pub is_archived: Option<bool>,
    pub is_readonly: Option<bool>,
    pub position: Option<i64>,
}

impl NotePatch {
    /// True when the patch would not change anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.note_type.is_none()
            && self.title.is_none()
            && self.content_text.is_none()
            && self.content_json.is_none()
            && self.color.is_none()
            && self.background.is_none()
            && self.is_pinned.is_none()
            && self.is_favorite.is_none()
            && self.is_archived.is_none()
            && self.is_readonly.is_none()
            && self.position.is_none()
    }
}

/// Counts words and characters the way the editor footer reports them.
///
/// Characters are counted in Unicode scalar values, not bytes, so Cyrillic text
/// does not read as twice its length.
#[must_use]
pub fn measure(text: &str) -> (i64, i64) {
    let words = text.split_whitespace().count();
    let chars = text.chars().count();
    (
        i64::try_from(words).unwrap_or(i64::MAX),
        i64::try_from(chars).unwrap_or(i64::MAX),
    )
}

/// Rejects a title that is too long.
///
/// # Errors
/// Returns [`ValidationError::TooLong`] past [`MAX_TITLE_LEN`].
pub fn validate_title(title: &str) -> AppResult<()> {
    if title.chars().count() > MAX_TITLE_LEN {
        return Err(AppError::Validation(ValidationError::TooLong {
            field: "title",
            max: MAX_TITLE_LEN,
        }));
    }
    Ok(())
}

/// Rejects a body that is too long.
///
/// # Errors
/// Returns [`ValidationError::TooLong`] past [`MAX_CONTENT_LEN`].
pub fn validate_content(content: &str) -> AppResult<()> {
    if content.chars().count() > MAX_CONTENT_LEN {
        return Err(AppError::Validation(ValidationError::TooLong {
            field: "content_text",
            max: MAX_CONTENT_LEN,
        }));
    }
    Ok(())
}

/// Derives a display title for a note the user never titled, using the first
/// non-empty line of the body. An untitled note in a list is useless.
#[must_use]
pub fn derive_title(title: &str, content_text: &str) -> String {
    if !title.trim().is_empty() {
        return title.trim().to_owned();
    }

    let first_line = content_text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");

    if first_line.is_empty() {
        return String::new();
    }

    // Keep it to something that fits a list row without ellipsising mid-word.
    let mut derived = String::new();
    for word in first_line.split_whitespace() {
        if derived.chars().count() + word.chars().count() + 1 > 80 {
            break;
        }
        if !derived.is_empty() {
            derived.push(' ');
        }
        derived.push_str(word);
    }

    if derived.is_empty() {
        first_line.chars().take(80).collect()
    } else {
        derived
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_note_type_round_trips_through_its_discriminator() {
        for note_type in [
            NoteType::Text,
            NoteType::RichText,
            NoteType::Checklist,
            NoteType::TaskList,
            NoteType::Journal,
            NoteType::DailyNote,
            NoteType::Meeting,
            NoteType::Idea,
            NoteType::Project,
            NoteType::Contact,
            NoteType::ShoppingList,
            NoteType::Habit,
            NoteType::PasswordHint,
            NoteType::Bookmark,
            NoteType::CodeSnippet,
            NoteType::VoiceNote,
            NoteType::Drawing,
        ] {
            let parsed = NoteType::parse(note_type.as_str()).expect("known discriminator");
            assert_eq!(parsed, note_type);
        }
    }

    #[test]
    fn an_unknown_note_type_is_rejected() {
        let error = NoteType::parse("hologram").expect_err("not a known type");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn characters_are_counted_in_scalars_not_bytes() {
        let (words, chars) = measure("привет мир");
        assert_eq!(words, 2);
        assert_eq!(chars, 10, "Cyrillic must not count double");
    }

    #[test]
    fn measuring_empty_text_yields_zero() {
        assert_eq!(measure(""), (0, 0));
        assert_eq!(measure("   \n\t  "), (0, 7));
    }

    #[test]
    fn an_overlong_title_is_rejected() {
        let title: String = "я".repeat(MAX_TITLE_LEN + 1);
        let error = validate_title(&title).expect_err("too long");
        assert_eq!(error.code(), "validation_too_long");
    }

    #[test]
    fn a_title_at_the_limit_is_accepted() {
        let title: String = "я".repeat(MAX_TITLE_LEN);
        validate_title(&title).expect("exactly at the limit is fine");
    }

    #[test]
    fn an_explicit_title_wins_over_the_body() {
        assert_eq!(derive_title("  Покупки  ", "молоко"), "Покупки");
    }

    #[test]
    fn a_missing_title_falls_back_to_the_first_non_empty_line() {
        assert_eq!(
            derive_title("", "\n\n  молоко и хлеб\nещё"),
            "молоко и хлеб"
        );
    }

    #[test]
    fn a_derived_title_does_not_cut_a_word_in_half() {
        let body = "слово ".repeat(40);
        let derived = derive_title("", &body);
        assert!(derived.chars().count() <= 80);
        assert!(
            !derived.ends_with("сло"),
            "words must stay whole: {derived}"
        );
    }

    #[test]
    fn an_empty_note_derives_an_empty_title() {
        assert_eq!(derive_title("", "   \n  "), "");
    }

    #[test]
    fn a_single_overlong_word_is_still_truncated() {
        let body = "я".repeat(200);
        let derived = derive_title("", &body);
        assert_eq!(derived.chars().count(), 80);
    }
}
