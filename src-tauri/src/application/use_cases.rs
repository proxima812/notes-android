//! Use cases.
//!
//! A use case owns one user-visible operation and coordinates the repositories
//! it needs. It holds its collaborators by constructor injection, so a test can
//! hand it a stub and the Tauri command layer stays free of logic.

use std::sync::Arc;

use crate::domain::ids::NoteId;
use crate::domain::notes::{Note, NoteRepository, Page, PageRequest};
use crate::domain::search::{SearchHit, SearchRepository};
use crate::error::AppResult;

use super::dto::{ListNotesRequest, SearchRequest};

pub struct NoteUseCases {
    notes: Arc<dyn NoteRepository>,
}

impl NoteUseCases {
    #[must_use]
    pub fn new(notes: Arc<dyn NoteRepository>) -> Self {
        Self { notes }
    }

    /// # Errors
    /// Fails on validation or a database error.
    pub fn create(&self, draft: crate::domain::notes::NoteDraft) -> AppResult<Note> {
        self.notes.create(draft)
    }

    /// # Errors
    /// Fails when the identifier is malformed, or on a database error.
    pub fn get(&self, id: &str) -> AppResult<Option<Note>> {
        self.notes.find(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails when the note is missing or read-only, or on a database error.
    pub fn update(&self, id: &str, patch: crate::domain::notes::NotePatch) -> AppResult<Note> {
        self.notes.update(NoteId::parse(id)?, patch)
    }

    /// # Errors
    /// Fails when the note is missing, or on a database error.
    pub fn move_to_trash(&self, id: &str) -> AppResult<()> {
        self.notes.soft_delete(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails when the note is missing, or on a database error.
    pub fn restore(&self, id: &str) -> AppResult<Note> {
        self.notes.restore(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails when the note is missing, or on a database error.
    pub fn purge(&self, id: &str) -> AppResult<()> {
        self.notes.purge(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails on a database error.
    pub fn empty_trash(&self) -> AppResult<u32> {
        self.notes.purge_trash(None)
    }

    /// # Errors
    /// Fails when a request identifier is malformed, or on a database error.
    pub fn list(&self, request: &ListNotesRequest) -> AppResult<Page<Note>> {
        let filter = request.to_filter()?;
        self.notes.list(&filter, request.sort, request.to_page())
    }

    /// # Errors
    /// Fails when the source is missing, or on a database error.
    pub fn duplicate(&self, id: &str) -> AppResult<Note> {
        self.notes.duplicate(NoteId::parse(id)?)
    }

    /// # Errors
    /// Fails when a request identifier is malformed, or on a database error.
    pub fn count(&self, request: &ListNotesRequest) -> AppResult<u32> {
        self.notes.count(&request.to_filter()?)
    }
}

pub struct SearchUseCases {
    search: Arc<dyn SearchRepository>,
}

impl SearchUseCases {
    #[must_use]
    pub fn new(search: Arc<dyn SearchRepository>) -> Self {
        Self { search }
    }

    /// Runs a search and remembers the query.
    ///
    /// History is recorded here rather than in the repository so that an
    /// internal search (say, from a smart folder) can reuse the same query path
    /// without polluting the user's recent-searches list.
    ///
    /// # Errors
    /// Fails when a request identifier is malformed, or on a database error.
    pub fn search(&self, request: &SearchRequest) -> AppResult<Page<SearchHit>> {
        let query = request.to_query()?;
        let page = self.search.search(&query, request.to_page())?;

        // Only the first page of a query counts as "the user ran this search";
        // paging through results must not push it to the top again.
        if request.offset.unwrap_or(0) == 0 {
            self.search.record_history(&request.text, page.total)?;
        }

        Ok(page)
    }

    /// Runs a search without touching history.
    ///
    /// # Errors
    /// Fails when a request identifier is malformed, or on a database error.
    pub fn search_quietly(&self, request: &SearchRequest) -> AppResult<Page<SearchHit>> {
        self.search.search(&request.to_query()?, request.to_page())
    }

    /// # Errors
    /// Fails on a database error.
    pub fn recent_queries(&self, limit: Option<u32>) -> AppResult<Vec<String>> {
        self.search.recent_queries(limit.unwrap_or(10))
    }

    /// # Errors
    /// Fails on a database error.
    pub fn clear_history(&self) -> AppResult<()> {
        self.search.clear_history()
    }
}

/// Reads a page without the caller having to know the default size.
#[must_use]
pub fn default_page() -> PageRequest {
    PageRequest::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clock::{FixedClock, SharedClock, Timestamp};
    use crate::domain::notes::NoteDraft;
    use crate::infrastructure::sqlite::{Database, SqliteNoteRepository, SqliteSearchRepository};

    struct Fixture {
        notes: NoteUseCases,
        search: SearchUseCases,
    }

    fn fixture() -> Fixture {
        let clock: SharedClock =
            Arc::new(FixedClock::new(Timestamp::from_millis(1_700_000_000_000)));
        let database = Arc::new(Database::open_in_memory(1_700_000_000_000).expect("opens"));
        Fixture {
            notes: NoteUseCases::new(Arc::new(SqliteNoteRepository::new(
                Arc::clone(&database),
                Arc::clone(&clock),
            ))),
            search: SearchUseCases::new(Arc::new(SqliteSearchRepository::new(database, clock))),
        }
    }

    #[test]
    fn a_malformed_identifier_never_reaches_the_database() {
        let f = fixture();
        let error = f.notes.get("not-a-uuid").expect_err("must be rejected");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn the_full_note_lifecycle_works_end_to_end() {
        let f = fixture();

        let created = f
            .notes
            .create(NoteDraft {
                title: Some("Покупки".to_owned()),
                content_text: Some("молоко".to_owned()),
                ..NoteDraft::default()
            })
            .expect("creates");
        let id = created.id.to_string();

        assert!(f.notes.get(&id).expect("reads").is_some());

        f.notes.move_to_trash(&id).expect("trashes");
        assert_eq!(
            f.notes.count(&ListNotesRequest::default()).expect("counts"),
            0
        );

        f.notes.restore(&id).expect("restores");
        assert_eq!(
            f.notes.count(&ListNotesRequest::default()).expect("counts"),
            1
        );

        f.notes.move_to_trash(&id).expect("trashes again");
        let removed = f.notes.empty_trash().expect("empties");
        assert_eq!(removed, 1);
        assert!(f.notes.get(&id).expect("reads").is_none());
    }

    #[test]
    fn searching_records_the_query_in_history() {
        let f = fixture();
        f.notes
            .create(NoteDraft {
                title: Some("Покупки".to_owned()),
                content_text: Some("молоко".to_owned()),
                ..NoteDraft::default()
            })
            .expect("creates");

        let page = f
            .search
            .search(&SearchRequest {
                text: "молоко".to_owned(),
                ..SearchRequest::default()
            })
            .expect("searches");

        assert_eq!(page.total, 1);
        assert_eq!(
            f.search.recent_queries(None).expect("reads"),
            vec!["молоко".to_owned()]
        );
    }

    #[test]
    fn paging_through_results_does_not_re_record_the_query() {
        let f = fixture();
        for index in 0..5 {
            f.notes
                .create(NoteDraft {
                    title: Some(format!("Заметка {index}")),
                    content_text: Some("молоко".to_owned()),
                    ..NoteDraft::default()
                })
                .expect("creates");
        }

        f.search
            .search(&SearchRequest {
                text: "молоко".to_owned(),
                limit: Some(2),
                ..SearchRequest::default()
            })
            .expect("first page");
        f.search
            .search(&SearchRequest {
                text: "хлеб".to_owned(),
                ..SearchRequest::default()
            })
            .expect("another query");
        f.search
            .search(&SearchRequest {
                text: "молоко".to_owned(),
                limit: Some(2),
                offset: Some(2),
                ..SearchRequest::default()
            })
            .expect("second page");

        // Had paging re-recorded it, "молоко" would have jumped back to the top.
        assert_eq!(
            f.search.recent_queries(None).expect("reads"),
            vec!["хлеб".to_owned(), "молоко".to_owned()]
        );
    }

    #[test]
    fn a_quiet_search_leaves_no_trace() {
        let f = fixture();
        f.search
            .search_quietly(&SearchRequest {
                text: "молоко".to_owned(),
                ..SearchRequest::default()
            })
            .expect("searches");
        assert!(f.search.recent_queries(None).expect("reads").is_empty());
    }
}
