//! SQLite/FTS5-backed [`SearchRepository`].

use rusqlite::{types::Value, ToSql};
use std::sync::Arc;

use crate::domain::clock::SharedClock;
use crate::domain::ids::SavedSearchId;
use crate::domain::notes::{Page, PageRequest};
use crate::domain::search::{
    build_match_expression, SearchEntity, SearchHit, SearchQuery, SearchRepository,
    HIGHLIGHT_CLOSE, HIGHLIGHT_OPEN,
};
use crate::error::{AppError, AppResult};

use super::Database;

pub struct SqliteSearchRepository {
    database: Arc<Database>,
    clock: SharedClock,
}

impl SqliteSearchRepository {
    #[must_use]
    pub fn new(database: Arc<Database>, clock: SharedClock) -> Self {
        Self { database, clock }
    }

    fn wants(query: &SearchQuery, entity: SearchEntity) -> bool {
        query.entities.is_empty() || query.entities.contains(&entity)
    }
}

/// Extra `AND` conditions applied to note hits, built from the structured part
/// of the query. These run against `notes`, not against the FTS table.
fn note_conditions(query: &SearchQuery) -> (String, Vec<Value>) {
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();

    if !query.include_trashed {
        conditions.push("n.deleted_at IS NULL".to_owned());
    }
    if !query.include_archived {
        conditions.push("n.is_archived = 0".to_owned());
    }
    if let Some(note_type) = query.note_type {
        conditions.push("n.note_type = ?".to_owned());
        params.push(Value::Text(note_type.as_str().to_owned()));
    }
    if let Some(tag_id) = query.tag_id {
        conditions.push(
            "EXISTS (SELECT 1 FROM note_tags nt WHERE nt.note_id = n.id AND nt.tag_id = ?)"
                .to_owned(),
        );
        params.push(Value::Text(tag_id.to_string()));
    }
    if let Some(after) = query.created_after {
        conditions.push("n.created_at >= ?".to_owned());
        params.push(Value::Integer(after.as_millis()));
    }
    if let Some(before) = query.created_before {
        conditions.push("n.created_at <= ?".to_owned());
        params.push(Value::Integer(before.as_millis()));
    }
    if query.has_reminder {
        conditions.push(
            "EXISTS (SELECT 1 FROM reminders r WHERE r.note_id = n.id \
             AND r.deleted_at IS NULL AND r.is_enabled = 1)"
                .to_owned(),
        );
    }
    if query.has_attachment {
        conditions.push(
            "EXISTS (SELECT 1 FROM attachments a WHERE a.note_id = n.id AND a.deleted_at IS NULL)"
                .to_owned(),
        );
    }

    if conditions.is_empty() {
        ("1 = 1".to_owned(), params)
    } else {
        (conditions.join(" AND "), params)
    }
}

impl SearchRepository for SqliteSearchRepository {
    fn search(&self, query: &SearchQuery, page: PageRequest) -> AppResult<Page<SearchHit>> {
        let Some(expression) = build_match_expression(&query.text) else {
            return Ok(Page {
                items: Vec::new(),
                total: 0,
                limit: page.limit,
                offset: page.offset,
            });
        };

        let (note_where, note_params) = note_conditions(query);

        // Each branch is a self-contained SELECT so a query restricted to one
        // entity never pays for the others.
        let mut branches: Vec<String> = Vec::new();
        let mut params: Vec<Value> = Vec::new();

        if Self::wants(query, SearchEntity::Note) {
            branches.push(format!(
                "SELECT 'note' AS entity, n.id AS id, n.title AS title,
                        snippet(notes_fts, 2, ?, ?, '…', 12) AS snippet,
                        bm25(notes_fts) AS rank
                 FROM notes_fts
                 JOIN notes n ON n.id = notes_fts.note_id
                 WHERE notes_fts MATCH ? AND {note_where}"
            ));
            params.push(Value::Text(HIGHLIGHT_OPEN.to_owned()));
            params.push(Value::Text(HIGHLIGHT_CLOSE.to_owned()));
            params.push(Value::Text(expression.clone()));
            params.extend(note_params.iter().cloned());
        }

        if Self::wants(query, SearchEntity::Task) {
            branches.push(
                "SELECT 'task' AS entity, t.id AS id, t.title AS title,
                        snippet(tasks_fts, 2, ?, ?, '…', 12) AS snippet,
                        bm25(tasks_fts) AS rank
                 FROM tasks_fts
                 JOIN tasks t ON t.id = tasks_fts.task_id
                 WHERE tasks_fts MATCH ? AND t.deleted_at IS NULL"
                    .to_owned(),
            );
            params.push(Value::Text(HIGHLIGHT_OPEN.to_owned()));
            params.push(Value::Text(HIGHLIGHT_CLOSE.to_owned()));
            params.push(Value::Text(expression.clone()));
        }

        if Self::wants(query, SearchEntity::Attachment) {
            branches.push(
                "SELECT 'attachment' AS entity, a.id AS id, a.name AS title,
                        snippet(attachments_fts, 2, ?, ?, '…', 12) AS snippet,
                        bm25(attachments_fts) AS rank
                 FROM attachments_fts
                 JOIN attachments a ON a.id = attachments_fts.attachment_id
                 WHERE attachments_fts MATCH ? AND a.deleted_at IS NULL"
                    .to_owned(),
            );
            params.push(Value::Text(HIGHLIGHT_OPEN.to_owned()));
            params.push(Value::Text(HIGHLIGHT_CLOSE.to_owned()));
            params.push(Value::Text(expression));
        }

        if branches.is_empty() {
            return Ok(Page {
                items: Vec::new(),
                total: 0,
                limit: page.limit,
                offset: page.offset,
            });
        }

        let union = branches.join(" UNION ALL ");

        self.database.with_connection(|connection| {
            let total: i64 = {
                let sql = format!("SELECT COUNT(*) FROM ({union})");
                let bound: Vec<&dyn ToSql> =
                    params.iter().map(|value| value as &dyn ToSql).collect();
                connection
                    .query_row(&sql, bound.as_slice(), |row| row.get(0))
                    .map_err(AppError::from)?
            };

            let sql = format!("SELECT * FROM ({union}) ORDER BY rank ASC LIMIT ? OFFSET ?");
            let mut all_params = params.clone();
            all_params.push(Value::Integer(i64::from(page.limit)));
            all_params.push(Value::Integer(i64::from(page.offset)));
            let bound: Vec<&dyn ToSql> =
                all_params.iter().map(|value| value as &dyn ToSql).collect();

            let mut statement = connection.prepare(&sql).map_err(AppError::from)?;
            let items = statement
                .query_map(bound.as_slice(), |row| {
                    let entity: String = row.get("entity")?;
                    Ok(SearchHit {
                        entity: match entity.as_str() {
                            "task" => SearchEntity::Task,
                            "attachment" => SearchEntity::Attachment,
                            _ => SearchEntity::Note,
                        },
                        id: row.get("id")?,
                        title: row.get("title")?,
                        snippet: row.get("snippet")?,
                        rank: row.get("rank")?,
                    })
                })
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<SearchHit>>>()
                .map_err(AppError::from)?;

            Ok(Page {
                items,
                total: u32::try_from(total).unwrap_or(u32::MAX),
                limit: page.limit,
                offset: page.offset,
            })
        })
    }

    fn record_history(&self, text: &str, result_count: u32) -> AppResult<()> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let now = self.clock.now();

        self.database.in_transaction(|transaction| {
            // Re-running a query moves it to the top rather than duplicating it.
            transaction
                .execute("DELETE FROM search_history WHERE query = ?1", [trimmed])
                .map_err(AppError::from)?;
            transaction
                .execute(
                    "INSERT INTO search_history (id, query, result_count, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![SavedSearchId::new(), trimmed, i64::from(result_count), now],
                )
                .map_err(AppError::from)?;
            // Keep the list short; nobody scrolls a thousand past queries.
            transaction
                .execute(
                    "DELETE FROM search_history WHERE id NOT IN
                         (SELECT id FROM search_history ORDER BY created_at DESC LIMIT 50)",
                    [],
                )
                .map_err(AppError::from)?;
            Ok(())
        })
    }

    fn recent_queries(&self, limit: u32) -> AppResult<Vec<String>> {
        self.database.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT query FROM search_history ORDER BY created_at DESC, rowid DESC LIMIT ?1",
                )
                .map_err(AppError::from)?;
            let rows = statement
                .query_map([i64::from(limit.clamp(1, 50))], |row| row.get::<_, String>(0))
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<String>>>()
                .map_err(AppError::from)?;
            Ok(rows)
        })
    }

    fn clear_history(&self) -> AppResult<()> {
        self.database.in_transaction(|transaction| {
            transaction
                .execute("DELETE FROM search_history", [])
                .map_err(AppError::from)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clock::{FixedClock, Timestamp};
    use crate::domain::notes::{NoteDraft, NotePatch, NoteRepository, NoteType};
    use crate::infrastructure::sqlite::SqliteNoteRepository;

    struct Fixture {
        notes: SqliteNoteRepository,
        search: SqliteSearchRepository,
    }

    fn fixture() -> Fixture {
        let clock: SharedClock =
            Arc::new(FixedClock::new(Timestamp::from_millis(1_700_000_000_000)));
        let database = Arc::new(Database::open_in_memory(1_700_000_000_000).expect("opens"));
        Fixture {
            notes: SqliteNoteRepository::new(Arc::clone(&database), Arc::clone(&clock)),
            search: SqliteSearchRepository::new(database, clock),
        }
    }

    fn seed(fixture: &Fixture, title: &str, body: &str) -> crate::domain::notes::Note {
        fixture
            .notes
            .create(NoteDraft {
                title: Some(title.to_owned()),
                content_text: Some(body.to_owned()),
                ..NoteDraft::default()
            })
            .expect("creates")
    }

    fn run(fixture: &Fixture, text: &str) -> Vec<SearchHit> {
        fixture
            .search
            .search(
                &SearchQuery {
                    text: text.to_owned(),
                    ..SearchQuery::default()
                },
                PageRequest::default(),
            )
            .expect("search runs")
            .items
    }

    #[test]
    fn a_whole_word_is_found() {
        let f = fixture();
        seed(&f, "Покупки", "молоко и хлеб");
        let hits = run(&f, "молоко");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Покупки");
        assert_eq!(hits[0].entity, SearchEntity::Note);
    }

    #[test]
    fn a_partial_word_is_found_while_typing() {
        let f = fixture();
        seed(&f, "Покупки", "молоко и хлеб");
        assert_eq!(run(&f, "мол").len(), 1);
        assert_eq!(run(&f, "моло").len(), 1);
    }

    #[test]
    fn the_title_is_searchable_too() {
        let f = fixture();
        seed(&f, "Совещание", "обсудили сроки");
        assert_eq!(run(&f, "совещание").len(), 1);
    }

    #[test]
    fn search_is_case_insensitive() {
        let f = fixture();
        seed(&f, "Покупки", "Молоко");
        assert_eq!(run(&f, "молоко").len(), 1);
        assert_eq!(run(&f, "МОЛОКО").len(), 1);
    }

    #[test]
    fn several_words_must_all_appear() {
        let f = fixture();
        seed(&f, "Первая", "молоко и хлеб");
        seed(&f, "Вторая", "молоко и сыр");

        assert_eq!(run(&f, "молоко хлеб").len(), 1);
        assert_eq!(run(&f, "молоко").len(), 2);
    }

    #[test]
    fn matches_come_back_wrapped_in_highlight_markers() {
        let f = fixture();
        seed(&f, "Покупки", "нужно купить молоко сегодня");
        let hits = run(&f, "молоко");
        assert!(
            hits[0].snippet.contains(HIGHLIGHT_OPEN),
            "snippet had no highlight: {}",
            hits[0].snippet
        );
    }

    #[test]
    fn a_trashed_note_is_not_searchable_by_default() {
        let f = fixture();
        let note = seed(&f, "Покупки", "молоко");
        f.notes.soft_delete(note.id).expect("deletes");
        assert_eq!(run(&f, "молоко").len(), 0);
    }

    #[test]
    fn an_archived_note_is_excluded_unless_asked_for() {
        let f = fixture();
        let note = seed(&f, "Покупки", "молоко");
        f.notes
            .update(
                note.id,
                NotePatch {
                    is_archived: Some(true),
                    ..NotePatch::default()
                },
            )
            .expect("archives");

        assert_eq!(run(&f, "молоко").len(), 0);

        let including = f
            .search
            .search(
                &SearchQuery {
                    text: "молоко".to_owned(),
                    include_archived: true,
                    ..SearchQuery::default()
                },
                PageRequest::default(),
            )
            .expect("search runs");
        assert_eq!(including.items.len(), 1);
    }

    #[test]
    fn filtering_by_note_type_narrows_results() {
        let f = fixture();
        f.notes
            .create(NoteDraft {
                note_type: Some(NoteType::Checklist),
                title: Some("Покупки".to_owned()),
                content_text: Some("молоко".to_owned()),
                ..NoteDraft::default()
            })
            .expect("creates");
        seed(&f, "Заметка", "молоко");

        let filtered = f
            .search
            .search(
                &SearchQuery {
                    text: "молоко".to_owned(),
                    note_type: Some(NoteType::Checklist),
                    ..SearchQuery::default()
                },
                PageRequest::default(),
            )
            .expect("search runs");
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.items[0].title, "Покупки");
    }

    #[test]
    fn nonsense_input_returns_nothing_instead_of_failing() {
        let f = fixture();
        seed(&f, "Покупки", "молоко");
        // Each of these is an FTS5 syntax error if passed through untouched.
        for input in ["", "   ", "???", "\"", "AND", "* *", "NEAR("] {
            let result = f.search.search(
                &SearchQuery {
                    text: input.to_owned(),
                    ..SearchQuery::default()
                },
                PageRequest::default(),
            );
            assert!(result.is_ok(), "input {input:?} must not error");
        }
    }

    #[test]
    fn results_are_paginated_with_a_total() {
        let f = fixture();
        for index in 0..15 {
            seed(&f, &format!("Заметка {index}"), "молоко");
        }
        let page = f
            .search
            .search(
                &SearchQuery {
                    text: "молоко".to_owned(),
                    ..SearchQuery::default()
                },
                PageRequest::new(5, 0),
            )
            .expect("search runs");

        assert_eq!(page.total, 15);
        assert_eq!(page.items.len(), 5);
        assert!(page.has_more());
    }

    #[test]
    fn history_keeps_the_latest_run_of_a_repeated_query() {
        let f = fixture();
        f.search.record_history("молоко", 3).expect("records");
        f.search.record_history("хлеб", 1).expect("records");
        f.search.record_history("молоко", 5).expect("records");

        let recent = f.search.recent_queries(10).expect("reads");
        assert_eq!(recent, vec!["молоко".to_owned(), "хлеб".to_owned()]);
    }

    #[test]
    fn blank_queries_are_not_recorded() {
        let f = fixture();
        f.search.record_history("   ", 0).expect("records nothing");
        assert!(f.search.recent_queries(10).expect("reads").is_empty());
    }

    #[test]
    fn history_can_be_cleared() {
        let f = fixture();
        f.search.record_history("молоко", 1).expect("records");
        f.search.clear_history().expect("clears");
        assert!(f.search.recent_queries(10).expect("reads").is_empty());
    }
}
