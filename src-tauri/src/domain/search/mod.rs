//! Search.
//!
//! User input never reaches FTS5 verbatim. A query like `хлеб"` or `AND` is
//! valid text to a person and a syntax error to FTS5, so [`build_match_expression`]
//! rewrites whatever was typed into a well-formed MATCH expression. The
//! alternative — letting the error surface — would make the search box feel
//! broken for perfectly reasonable input.

use serde::{Deserialize, Serialize};

use crate::domain::clock::Timestamp;
use crate::domain::ids::TagId;
use crate::domain::notes::NoteType;
use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEntity {
    Note,
    Task,
    Attachment,
}

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// Raw text as typed.
    pub text: String,
    /// Which indexes to consult. Empty means all of them.
    pub entities: Vec<SearchEntity>,
    pub tag_id: Option<TagId>,
    pub note_type: Option<NoteType>,
    pub created_after: Option<Timestamp>,
    pub created_before: Option<Timestamp>,
    /// Restrict to notes that carry at least one enabled reminder.
    pub has_reminder: bool,
    /// Restrict to notes that carry at least one attachment.
    pub has_attachment: bool,
    pub include_archived: bool,
    pub include_trashed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub entity: SearchEntity,
    pub id: String,
    pub title: String,
    /// Body excerpt with matches wrapped in the marker below.
    pub snippet: String,
    /// FTS5 rank; lower is a better match.
    pub rank: f64,
}

/// Markers wrapped around matches in a snippet. Chosen to be text the user
/// cannot type by accident, so the UI can split on them without escaping HTML.
pub const HIGHLIGHT_OPEN: &str = "\u{2062}[";
pub const HIGHLIGHT_CLOSE: &str = "]\u{2062}";

/// Rewrites free text into a safe FTS5 MATCH expression.
///
/// Each whitespace-separated term becomes a quoted string, and the final term
/// gains a `*` so results narrow as the user is still typing. Double quotes in
/// the input are doubled, which is how FTS5 escapes them inside a string.
///
/// Returns `None` when nothing searchable remains, which the caller treats as
/// an empty result rather than as "match everything".
#[must_use]
pub fn build_match_expression(text: &str) -> Option<String> {
    // A term surrounded by double quotes in the input is an explicit phrase
    // search and is kept as one unit.
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let explicit_phrase = trimmed.len() >= 2
        && trimmed.starts_with('"')
        && trimmed.ends_with('"')
        && trimmed[1..trimmed.len() - 1].contains(|character: char| character != '"');

    if explicit_phrase {
        let inner = &trimmed[1..trimmed.len() - 1];
        let escaped = inner.replace('"', "\"\"");
        return Some(format!("\"{escaped}\""));
    }

    let terms: Vec<String> = trimmed
        .split_whitespace()
        .map(|term| {
            term.chars()
                .filter(|character| character.is_alphanumeric() || *character == '_')
                .collect::<String>()
        })
        .filter(|term| !term.is_empty())
        .collect();

    if terms.is_empty() {
        return None;
    }

    let last = terms.len() - 1;
    let expression = terms
        .iter()
        .enumerate()
        .map(|(index, term)| {
            if index == last {
                // Prefix-match the final term so search narrows while typing.
                format!("\"{term}\"*")
            } else {
                format!("\"{term}\"")
            }
        })
        .collect::<Vec<String>>()
        .join(" AND ");

    Some(expression)
}

pub trait SearchRepository: Send + Sync {
    /// Runs a full-text search.
    ///
    /// # Errors
    /// Fails on a database error.
    fn search(
        &self,
        query: &SearchQuery,
        page: crate::domain::notes::PageRequest,
    ) -> AppResult<crate::domain::notes::Page<SearchHit>>;

    /// Records a query the user actually ran, for the recent-searches list.
    ///
    /// # Errors
    /// Fails on a database error.
    fn record_history(&self, text: &str, result_count: u32) -> AppResult<()>;

    /// Most recent distinct queries, newest first.
    ///
    /// # Errors
    /// Fails on a database error.
    fn recent_queries(&self, limit: u32) -> AppResult<Vec<String>>;

    /// Clears the search history.
    ///
    /// # Errors
    /// Fails on a database error.
    fn clear_history(&self) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_word_becomes_a_prefix_match() {
        assert_eq!(build_match_expression("мол").as_deref(), Some("\"мол\"*"));
    }

    #[test]
    fn several_words_are_all_required() {
        assert_eq!(
            build_match_expression("молоко хлеб").as_deref(),
            Some("\"молоко\" AND \"хлеб\"*")
        );
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        assert_eq!(
            build_match_expression("   молоко   ").as_deref(),
            Some("\"молоко\"*")
        );
    }

    #[test]
    fn empty_input_matches_nothing_rather_than_everything() {
        assert_eq!(build_match_expression(""), None);
        assert_eq!(build_match_expression("    "), None);
    }

    #[test]
    fn punctuation_only_input_matches_nothing() {
        // Typing `???` must not produce an FTS5 syntax error.
        assert_eq!(build_match_expression("???"), None);
        assert_eq!(build_match_expression("- * ^"), None);
    }

    #[test]
    fn fts5_operators_are_neutralised() {
        // `AND`/`OR`/`NOT` are FTS5 keywords; quoting turns them into words.
        let expression = build_match_expression("NOT молоко").expect("has terms");
        assert_eq!(expression, "\"NOT\" AND \"молоко\"*");
    }

    #[test]
    fn a_stray_quote_cannot_break_the_expression() {
        let expression = build_match_expression("хлеб\"").expect("has terms");
        assert_eq!(expression, "\"хлеб\"*");
    }

    #[test]
    fn an_explicit_phrase_is_kept_whole() {
        assert_eq!(
            build_match_expression("\"молоко и хлеб\"").as_deref(),
            Some("\"молоко и хлеб\"")
        );
    }

    #[test]
    fn an_empty_phrase_is_not_treated_as_a_phrase() {
        assert_eq!(build_match_expression("\"\""), None);
    }

    #[test]
    fn digits_and_underscores_survive() {
        assert_eq!(
            build_match_expression("отчёт_2026").as_deref(),
            Some("\"отчёт_2026\"*")
        );
    }
}
