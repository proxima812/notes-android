//! SQLite-backed [`NoteRepository`].
//!
//! All SQL for notes lives here. Statements are parameterised, multi-table
//! writes run inside a transaction, and the FTS index is maintained by triggers
//! in the schema rather than by hand at each call site.

use rusqlite::{types::Value, Connection, OptionalExtension as _, Row, ToSql};
use std::sync::Arc;

use crate::domain::clock::{SharedClock, Timestamp};
use crate::domain::ids::{NoteBlockId, NoteId};
use crate::domain::notes::{
    derive_title, measure, validate_content, validate_title, Note, NoteDraft, NoteFilter,
    NotePatch, NoteRepository, NoteScope, NoteSort, NoteType, Page, PageRequest,
};
use crate::error::{AppError, AppResult, DatabaseError, ValidationError};

use super::Database;

const NOTE_COLUMNS: &str = "id, note_type, title, content_text, content_json, color, background, \
     is_pinned, is_favorite, is_archived, is_readonly, position, word_count, char_count, \
     revision, created_at, updated_at, deleted_at";

pub struct SqliteNoteRepository {
    database: Arc<Database>,
    clock: SharedClock,
}

impl SqliteNoteRepository {
    #[must_use]
    pub fn new(database: Arc<Database>, clock: SharedClock) -> Self {
        Self { database, clock }
    }
}

fn map_note(row: &Row<'_>) -> rusqlite::Result<Note> {
    let note_type: String = row.get("note_type")?;
    Ok(Note {
        id: row.get("id")?,
        // A row with an unknown discriminator would be unreadable; falling back
        // to Text keeps the note openable instead of hiding it from the user.
        note_type: NoteType::parse(&note_type).unwrap_or(NoteType::Text),
        title: row.get("title")?,
        content_text: row.get("content_text")?,
        content_json: row.get("content_json")?,
        color: row.get("color")?,
        background: row.get("background")?,
        is_pinned: row.get::<_, i64>("is_pinned")? != 0,
        is_favorite: row.get::<_, i64>("is_favorite")? != 0,
        is_archived: row.get::<_, i64>("is_archived")? != 0,
        is_readonly: row.get::<_, i64>("is_readonly")? != 0,
        position: row.get("position")?,
        word_count: row.get("word_count")?,
        char_count: row.get("char_count")?,
        revision: row.get("revision")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn fetch(connection: &Connection, id: NoteId) -> AppResult<Option<Note>> {
    let sql = format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?1");
    connection
        .query_row(&sql, [&id], map_note)
        .optional()
        .map_err(AppError::from)
}

fn require(connection: &Connection, id: NoteId) -> AppResult<Note> {
    fetch(connection, id)?.ok_or_else(|| {
        AppError::Database(DatabaseError::NotFound {
            entity: "note",
            id: id.to_string(),
        })
    })
}

/// Builds the `WHERE` fragment for a filter along with its bound parameters.
fn where_clause(filter: &NoteFilter) -> (String, Vec<Value>) {
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();

    match filter.scope {
        NoteScope::Active => {
            conditions.push("notes.deleted_at IS NULL AND notes.is_archived = 0".to_owned());
        }
        NoteScope::Archived => {
            conditions.push("notes.deleted_at IS NULL AND notes.is_archived = 1".to_owned());
        }
        NoteScope::Trash => conditions.push("notes.deleted_at IS NOT NULL".to_owned()),
        NoteScope::Favorites => {
            conditions.push("notes.deleted_at IS NULL AND notes.is_favorite = 1".to_owned());
        }
        NoteScope::AllExceptTrash => conditions.push("notes.deleted_at IS NULL".to_owned()),
    }

    if filter.pinned_only {
        conditions.push("notes.is_pinned = 1".to_owned());
    }

    if let Some(tag_id) = filter.tag_id {
        conditions.push(
            "EXISTS (SELECT 1 FROM note_tags nt \
             WHERE nt.note_id = notes.id AND nt.tag_id = ?)"
                .to_owned(),
        );
        params.push(Value::Text(tag_id.to_string()));
    }

    if let Some(note_type) = filter.note_type {
        conditions.push("notes.note_type = ?".to_owned());
        params.push(Value::Text(note_type.as_str().to_owned()));
    }

    if let Some(after) = filter.updated_after {
        conditions.push("notes.updated_at >= ?".to_owned());
        params.push(Value::Integer(after.as_millis()));
    }

    if let Some(before) = filter.updated_before {
        conditions.push("notes.updated_at <= ?".to_owned());
        params.push(Value::Integer(before.as_millis()));
    }

    (conditions.join(" AND "), params)
}

fn order_clause(sort: NoteSort) -> &'static str {
    match sort {
        NoteSort::PinnedThenUpdated => "notes.is_pinned DESC, notes.updated_at DESC, notes.id DESC",
        NoteSort::UpdatedDesc => "notes.updated_at DESC, notes.id DESC",
        NoteSort::UpdatedAsc => "notes.updated_at ASC, notes.id ASC",
        NoteSort::CreatedDesc => "notes.created_at DESC, notes.id DESC",
        NoteSort::CreatedAsc => "notes.created_at ASC, notes.id ASC",
        NoteSort::TitleAsc => "notes.title COLLATE NOCASE ASC, notes.id ASC",
        NoteSort::TitleDesc => "notes.title COLLATE NOCASE DESC, notes.id DESC",
        NoteSort::Manual => "notes.position ASC, notes.updated_at DESC, notes.id DESC",
        // SQLite has no NULLS LAST, so the boolean sorts absent reminders after
        // present ones instead.
        NoteSort::NextReminder => {
            "next_reminder_at IS NULL, next_reminder_at ASC, notes.updated_at DESC, notes.id DESC"
        }
    }
}

impl NoteRepository for SqliteNoteRepository {
    fn create(&self, draft: NoteDraft) -> AppResult<Note> {
        let note_type = draft.note_type.unwrap_or(NoteType::Text);
        let raw_title = draft.title.unwrap_or_default();
        let content_text = draft.content_text.unwrap_or_default();

        validate_title(&raw_title)?;
        validate_content(&content_text)?;

        let title = derive_title(&raw_title, &content_text);
        let (word_count, char_count) = measure(&content_text);
        let now = self.clock.now();
        let id = NoteId::new();

        self.database.in_transaction(|transaction| {
            transaction
                .execute(
                    "INSERT INTO notes (
                         id, note_type, title, content_text, content_json, color, background,
                         is_pinned, is_favorite, is_archived, is_readonly, position,
                         word_count, char_count, revision, created_at, updated_at, deleted_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, 0, 0, 0, ?8, ?9, 1, ?10, ?11, NULL)",
                    rusqlite::params![
                        id,
                        note_type.as_str(),
                        title,
                        content_text,
                        draft.content_json,
                        draft.color,
                        draft.background,
                        word_count,
                        char_count,
                        now,
                        now,
                    ],
                )
                .map_err(AppError::from)?;

            require(transaction, id)
        })
    }

    fn find(&self, id: NoteId) -> AppResult<Option<Note>> {
        self.database
            .with_connection(|connection| fetch(connection, id))
    }

    fn update(&self, id: NoteId, patch: NotePatch) -> AppResult<Note> {
        if let Some(title) = patch.title.as_deref() {
            validate_title(title)?;
        }
        if let Some(content) = patch.content_text.as_deref() {
            validate_content(content)?;
        }

        let now = self.clock.now();

        self.database.in_transaction(|transaction| {
            let existing = require(transaction, id)?;

            if existing.is_readonly && patch.is_readonly != Some(false) {
                // Read-only mode exists to stop accidental edits; only the flag
                // itself may be changed while it is on.
                let touches_content = patch.title.is_some()
                    || patch.content_text.is_some()
                    || patch.content_json.is_some();
                if touches_content {
                    return Err(AppError::Validation(ValidationError::Invalid {
                        field: "is_readonly",
                    }));
                }
            }

            if patch.is_empty() {
                return Ok(existing);
            }

            let mut assignments: Vec<String> = Vec::new();
            let mut params: Vec<Value> = Vec::new();

            if let Some(note_type) = patch.note_type {
                assignments.push("note_type = ?".to_owned());
                params.push(Value::Text(note_type.as_str().to_owned()));
            }

            // Title and body interact: clearing the title re-derives it from the
            // body, so they are resolved together against the post-patch state.
            let next_content = patch
                .content_text
                .clone()
                .unwrap_or_else(|| existing.content_text.clone());

            if let Some(title) = patch.title.clone() {
                assignments.push("title = ?".to_owned());
                params.push(Value::Text(derive_title(&title, &next_content)));
            }

            if let Some(content_text) = patch.content_text {
                let (word_count, char_count) = measure(&content_text);
                assignments.push("content_text = ?".to_owned());
                params.push(Value::Text(content_text.clone()));
                assignments.push("word_count = ?".to_owned());
                params.push(Value::Integer(word_count));
                assignments.push("char_count = ?".to_owned());
                params.push(Value::Integer(char_count));

                // An untitled note follows its body as the user types.
                if patch.title.is_none() && existing.title.is_empty() {
                    assignments.push("title = ?".to_owned());
                    params.push(Value::Text(derive_title("", &content_text)));
                }
            }

            if let Some(content_json) = patch.content_json {
                assignments.push("content_json = ?".to_owned());
                params.push(content_json.map_or(Value::Null, Value::Text));
            }

            if let Some(color) = patch.color {
                assignments.push("color = ?".to_owned());
                params.push(color.map_or(Value::Null, Value::Text));
            }

            if let Some(background) = patch.background {
                assignments.push("background = ?".to_owned());
                params.push(background.map_or(Value::Null, Value::Text));
            }

            for (column, flag) in [
                ("is_pinned", patch.is_pinned),
                ("is_favorite", patch.is_favorite),
                ("is_archived", patch.is_archived),
                ("is_readonly", patch.is_readonly),
            ] {
                if let Some(value) = flag {
                    assignments.push(format!("{column} = ?"));
                    params.push(Value::Integer(i64::from(value)));
                }
            }

            if let Some(position) = patch.position {
                assignments.push("position = ?".to_owned());
                params.push(Value::Integer(position));
            }

            assignments.push("revision = revision + 1".to_owned());
            assignments.push("updated_at = ?".to_owned());
            params.push(Value::Integer(now.as_millis()));
            params.push(Value::Text(id.to_string()));

            let sql = format!("UPDATE notes SET {} WHERE id = ?", assignments.join(", "));
            let bound: Vec<&dyn ToSql> = params.iter().map(|value| value as &dyn ToSql).collect();
            transaction
                .execute(&sql, bound.as_slice())
                .map_err(AppError::from)?;

            require(transaction, id)
        })
    }

    fn soft_delete(&self, id: NoteId) -> AppResult<()> {
        let now = self.clock.now();
        self.database.in_transaction(|transaction| {
            let affected = transaction
                .execute(
                    "UPDATE notes SET deleted_at = ?1, updated_at = ?1 \
                     WHERE id = ?2 AND deleted_at IS NULL",
                    rusqlite::params![now, id],
                )
                .map_err(AppError::from)?;

            if affected == 0 && fetch(transaction, id)?.is_none() {
                return Err(AppError::Database(DatabaseError::NotFound {
                    entity: "note",
                    id: id.to_string(),
                }));
            }
            Ok(())
        })
    }

    fn restore(&self, id: NoteId) -> AppResult<Note> {
        let now = self.clock.now();
        self.database.in_transaction(|transaction| {
            transaction
                .execute(
                    "UPDATE notes SET deleted_at = NULL, updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![now, id],
                )
                .map_err(AppError::from)?;
            require(transaction, id)
        })
    }

    fn purge(&self, id: NoteId) -> AppResult<()> {
        self.database.in_transaction(|transaction| {
            let affected = transaction
                .execute("DELETE FROM notes WHERE id = ?1", [&id])
                .map_err(AppError::from)?;
            if affected == 0 {
                return Err(AppError::Database(DatabaseError::NotFound {
                    entity: "note",
                    id: id.to_string(),
                }));
            }
            Ok(())
        })
    }

    fn purge_trash(&self, older_than: Option<Timestamp>) -> AppResult<u32> {
        self.database.in_transaction(|transaction| {
            let affected = match older_than {
                Some(cutoff) => transaction.execute(
                    "DELETE FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
                    [&cutoff],
                ),
                None => transaction.execute("DELETE FROM notes WHERE deleted_at IS NOT NULL", []),
            }
            .map_err(AppError::from)?;
            Ok(u32::try_from(affected).unwrap_or(u32::MAX))
        })
    }

    fn list(
        &self,
        filter: &NoteFilter,
        sort: NoteSort,
        page: PageRequest,
    ) -> AppResult<Page<Note>> {
        let (conditions, params) = where_clause(filter);

        self.database.with_connection(|connection| {
            let total: i64 = {
                let sql = format!("SELECT COUNT(*) FROM notes WHERE {conditions}");
                let bound: Vec<&dyn ToSql> =
                    params.iter().map(|value| value as &dyn ToSql).collect();
                connection
                    .query_row(&sql, bound.as_slice(), |row| row.get(0))
                    .map_err(AppError::from)?
            };

            // The correlated subquery is only computed when actually sorting by
            // it; SQLite skips unreferenced result columns in the other cases.
            let sql = format!(
                "SELECT {NOTE_COLUMNS},
                        (SELECT MIN(r.scheduled_at) FROM reminders r
                          WHERE r.note_id = notes.id
                            AND r.deleted_at IS NULL
                            AND r.is_enabled = 1) AS next_reminder_at
                 FROM notes
                 WHERE {conditions}
                 ORDER BY {}
                 LIMIT ?{} OFFSET ?{}",
                order_clause(sort),
                params.len() + 1,
                params.len() + 2,
            );

            let mut all_params = params.clone();
            all_params.push(Value::Integer(i64::from(page.limit)));
            all_params.push(Value::Integer(i64::from(page.offset)));
            let bound: Vec<&dyn ToSql> =
                all_params.iter().map(|value| value as &dyn ToSql).collect();

            let mut statement = connection.prepare(&sql).map_err(AppError::from)?;
            let items = statement
                .query_map(bound.as_slice(), map_note)
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<Note>>>()
                .map_err(AppError::from)?;

            Ok(Page {
                items,
                total: u32::try_from(total).unwrap_or(u32::MAX),
                limit: page.limit,
                offset: page.offset,
            })
        })
    }

    fn duplicate(&self, id: NoteId) -> AppResult<Note> {
        let now = self.clock.now();
        let new_id = NoteId::new();

        self.database.in_transaction(|transaction| {
            let source = require(transaction, id)?;

            let copied_title = if source.title.is_empty() {
                String::new()
            } else {
                format!("{} (копия)", source.title)
            };

            transaction
                .execute(
                    "INSERT INTO notes (
                         id, note_type, title, content_text, content_json, color, background,
                         is_pinned, is_favorite, is_archived, is_readonly, position,
                         word_count, char_count, revision, created_at, updated_at, deleted_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8, ?9, ?10, ?11, ?12, 1, ?13, ?13, NULL)",
                    rusqlite::params![
                        new_id,
                        source.note_type.as_str(),
                        copied_title,
                        source.content_text,
                        source.content_json,
                        source.color,
                        source.background,
                        i64::from(source.is_archived),
                        i64::from(source.is_readonly),
                        source.position,
                        source.word_count,
                        source.char_count,
                        now,
                    ],
                )
                .map_err(AppError::from)?;

            // Blocks are copied with fresh ids, preserving nesting by mapping
            // old parent ids to new ones in insertion order (parents first).
            let mut statement = transaction
                .prepare(
                    "SELECT id, parent_block_id, block_type, text, is_checked, data_json, position
                     FROM note_blocks WHERE note_id = ?1
                     ORDER BY (parent_block_id IS NOT NULL), position",
                )
                .map_err(AppError::from)?;

            let rows = statement
                .query_map([&id], |row| {
                    Ok((
                        row.get::<_, NoteBlockId>("id")?,
                        row.get::<_, Option<NoteBlockId>>("parent_block_id")?,
                        row.get::<_, String>("block_type")?,
                        row.get::<_, String>("text")?,
                        row.get::<_, i64>("is_checked")?,
                        row.get::<_, Option<String>>("data_json")?,
                        row.get::<_, i64>("position")?,
                    ))
                })
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;

            let mut remapped: std::collections::HashMap<NoteBlockId, NoteBlockId> =
                std::collections::HashMap::new();

            for (old_id, parent, block_type, text, is_checked, data_json, position) in rows {
                let fresh = NoteBlockId::new();
                remapped.insert(old_id, fresh);
                let new_parent = parent.and_then(|value| remapped.get(&value).copied());

                transaction
                    .execute(
                        "INSERT INTO note_blocks (
                             id, note_id, parent_block_id, block_type, text, is_checked,
                             data_json, position, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
                        rusqlite::params![
                            fresh,
                            new_id,
                            new_parent,
                            block_type,
                            text,
                            is_checked,
                            data_json,
                            position,
                            now,
                        ],
                    )
                    .map_err(AppError::from)?;
            }

            transaction
                .execute(
                    "INSERT INTO note_tags (note_id, tag_id, created_at)
                     SELECT ?1, tag_id, ?2 FROM note_tags WHERE note_id = ?3",
                    rusqlite::params![new_id, now, id],
                )
                .map_err(AppError::from)?;

            require(transaction, new_id)
        })
    }

    fn count(&self, filter: &NoteFilter) -> AppResult<u32> {
        let (conditions, params) = where_clause(filter);
        self.database.with_connection(|connection| {
            let sql = format!("SELECT COUNT(*) FROM notes WHERE {conditions}");
            let bound: Vec<&dyn ToSql> = params.iter().map(|value| value as &dyn ToSql).collect();
            let total: i64 = connection
                .query_row(&sql, bound.as_slice(), |row| row.get(0))
                .map_err(AppError::from)?;
            Ok(u32::try_from(total).unwrap_or(u32::MAX))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clock::{Clock, FixedClock};

    struct Fixture {
        repository: SqliteNoteRepository,
        database: Arc<Database>,
        clock: Arc<FixedClock>,
    }

    fn fixture() -> Fixture {
        let clock = Arc::new(FixedClock::new(Timestamp::from_millis(1_700_000_000_000)));
        let database = Arc::new(Database::open_in_memory(clock.now().as_millis()).expect("opens"));
        let repository = SqliteNoteRepository::new(Arc::clone(&database), clock.clone());
        Fixture {
            repository,
            database,
            clock,
        }
    }

    fn draft(title: &str, body: &str) -> NoteDraft {
        NoteDraft {
            title: Some(title.to_owned()),
            content_text: Some(body.to_owned()),
            ..NoteDraft::default()
        }
    }

    fn fts_hits(database: &Database, query: &str) -> i64 {
        database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH ?1",
                        [query],
                        |row| row.get(0),
                    )
                    .map_err(AppError::from)
            })
            .expect("fts query runs")
    }

    #[test]
    fn a_created_note_is_readable_with_its_counts() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Покупки", "молоко и хлеб"))
            .expect("creates");

        assert_eq!(note.title, "Покупки");
        assert_eq!(note.content_text, "молоко и хлеб");
        assert_eq!(note.word_count, 3);
        assert_eq!(note.char_count, 13);
        assert_eq!(note.revision, 1);
        assert!(!note.is_in_trash());
        assert_eq!(note.created_at, f.clock.now());

        let found = f.repository.find(note.id).expect("query runs");
        assert_eq!(found.as_ref(), Some(&note));
    }

    #[test]
    fn a_note_without_a_title_borrows_the_first_line_of_the_body() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("", "  первая строка\nвторая"))
            .expect("creates");
        assert_eq!(note.title, "первая строка");
    }

    #[test]
    fn an_overlong_title_is_refused_before_any_write() {
        let f = fixture();
        let long: String = "я".repeat(crate::domain::notes::MAX_TITLE_LEN + 1);
        let error = f
            .repository
            .create(draft(&long, "тело"))
            .expect_err("must be rejected");
        assert_eq!(error.code(), "validation_too_long");

        let filter = NoteFilter::default();
        assert_eq!(f.repository.count(&filter).expect("counts"), 0);
    }

    #[test]
    fn updating_bumps_the_revision_and_recounts() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Заметка", "раз"))
            .expect("creates");

        let updated = f
            .repository
            .update(
                note.id,
                NotePatch {
                    content_text: Some("раз два три".to_owned()),
                    ..NotePatch::default()
                },
            )
            .expect("updates");

        assert_eq!(updated.revision, 2);
        assert_eq!(updated.word_count, 3);
        assert_eq!(
            updated.title, "Заметка",
            "an explicit title is not overwritten"
        );
    }

    #[test]
    fn an_empty_patch_changes_nothing() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Заметка", "текст"))
            .expect("creates");
        let updated = f
            .repository
            .update(note.id, NotePatch::default())
            .expect("updates");
        assert_eq!(
            updated.revision, 1,
            "a no-op must not create a new revision"
        );
    }

    #[test]
    fn an_untitled_note_keeps_following_its_body() {
        let f = fixture();
        let note = f.repository.create(draft("", "")).expect("creates");
        assert_eq!(note.title, "");

        let updated = f
            .repository
            .update(
                note.id,
                NotePatch {
                    content_text: Some("новая первая строка".to_owned()),
                    ..NotePatch::default()
                },
            )
            .expect("updates");
        assert_eq!(updated.title, "новая первая строка");
    }

    #[test]
    fn a_read_only_note_refuses_content_edits_but_allows_unlocking() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Архивное", "текст"))
            .expect("creates");
        f.repository
            .update(
                note.id,
                NotePatch {
                    is_readonly: Some(true),
                    ..NotePatch::default()
                },
            )
            .expect("locks");

        let error = f
            .repository
            .update(
                note.id,
                NotePatch {
                    content_text: Some("правка".to_owned()),
                    ..NotePatch::default()
                },
            )
            .expect_err("edits are refused while locked");
        assert_eq!(error.code(), "validation_invalid");

        f.repository
            .update(
                note.id,
                NotePatch {
                    is_readonly: Some(false),
                    ..NotePatch::default()
                },
            )
            .expect("unlocking is always allowed");
    }

    #[test]
    fn updating_a_missing_note_reports_not_found() {
        let f = fixture();
        let error = f
            .repository
            .update(
                NoteId::new(),
                NotePatch {
                    position: Some(1),
                    ..NotePatch::default()
                },
            )
            .expect_err("nothing to update");
        assert_eq!(error.code(), "not_found");
    }

    #[test]
    fn a_deleted_note_leaves_the_active_list_and_can_come_back() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Черновик", "текст"))
            .expect("creates");

        f.repository.soft_delete(note.id).expect("deletes");

        let active = NoteFilter::default();
        assert_eq!(f.repository.count(&active).expect("counts"), 0);

        let trash = NoteFilter {
            scope: NoteScope::Trash,
            ..NoteFilter::default()
        };
        assert_eq!(f.repository.count(&trash).expect("counts"), 1);

        let restored = f.repository.restore(note.id).expect("restores");
        assert!(!restored.is_in_trash());
        assert_eq!(f.repository.count(&active).expect("counts"), 1);
    }

    #[test]
    fn deleting_a_missing_note_reports_not_found() {
        let f = fixture();
        let error = f
            .repository
            .soft_delete(NoteId::new())
            .expect_err("nothing to delete");
        assert_eq!(error.code(), "not_found");
    }

    #[test]
    fn deleting_twice_is_not_an_error() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Черновик", "текст"))
            .expect("creates");
        f.repository.soft_delete(note.id).expect("deletes");
        f.repository
            .soft_delete(note.id)
            .expect("a second delete is a no-op, not a failure");
    }

    #[test]
    fn purging_the_trash_only_removes_old_enough_notes() {
        let f = fixture();
        let old = f
            .repository
            .create(draft("Старая", "текст"))
            .expect("creates");
        let recent = f
            .repository
            .create(draft("Свежая", "текст"))
            .expect("creates");

        f.repository.soft_delete(old.id).expect("deletes");
        f.repository.soft_delete(recent.id).expect("deletes");

        // Both were deleted at the same fixed instant; a cutoff before it keeps them.
        let removed = f
            .repository
            .purge_trash(Some(f.clock.now()))
            .expect("purges");
        assert_eq!(removed, 0, "the cutoff is exclusive");

        let removed = f
            .repository
            .purge_trash(Some(f.clock.now().saturating_add_minutes(1)))
            .expect("purges");
        assert_eq!(removed, 2);
        assert_eq!(f.repository.find(old.id).expect("query runs"), None);
    }

    #[test]
    fn a_purged_note_is_gone_from_the_search_index_too() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Покупки", "молоко"))
            .expect("creates");
        assert_eq!(fts_hits(&f.database, "молоко"), 1);

        f.repository.purge(note.id).expect("purges");
        assert_eq!(
            fts_hits(&f.database, "молоко"),
            0,
            "a hard delete must not leave the text searchable"
        );
    }

    #[test]
    fn a_trashed_note_disappears_from_search_and_returns_on_restore() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Покупки", "молоко"))
            .expect("creates");

        f.repository.soft_delete(note.id).expect("deletes");
        assert_eq!(fts_hits(&f.database, "молоко"), 0);

        f.repository.restore(note.id).expect("restores");
        assert_eq!(fts_hits(&f.database, "молоко"), 1);
    }

    #[test]
    fn editing_a_note_updates_what_is_searchable() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Покупки", "молоко"))
            .expect("creates");

        f.repository
            .update(
                note.id,
                NotePatch {
                    content_text: Some("хлеб".to_owned()),
                    ..NotePatch::default()
                },
            )
            .expect("updates");

        assert_eq!(
            fts_hits(&f.database, "молоко"),
            0,
            "stale text must not linger"
        );
        assert_eq!(fts_hits(&f.database, "хлеб"), 1);
    }

    #[test]
    fn the_active_list_puts_pinned_notes_first() {
        let f = fixture();
        let first = f.repository.create(draft("Первая", "a")).expect("creates");
        let second = f.repository.create(draft("Вторая", "b")).expect("creates");
        let third = f.repository.create(draft("Третья", "c")).expect("creates");

        f.repository
            .update(
                first.id,
                NotePatch {
                    is_pinned: Some(true),
                    ..NotePatch::default()
                },
            )
            .expect("pins");

        let page = f
            .repository
            .list(
                &NoteFilter::default(),
                NoteSort::PinnedThenUpdated,
                PageRequest::default(),
            )
            .expect("lists");

        // All three share an `updated_at` under the fixed clock, so the tie is
        // broken by id, which for UUIDv7 means newest first.
        let order: Vec<NoteId> = page.items.iter().map(|note| note.id).collect();
        assert_eq!(page.total, 3);
        assert_eq!(order, vec![first.id, third.id, second.id]);
    }

    #[test]
    fn archived_notes_are_their_own_scope() {
        let f = fixture();
        let note = f
            .repository
            .create(draft("Старое", "текст"))
            .expect("creates");
        f.repository
            .update(
                note.id,
                NotePatch {
                    is_archived: Some(true),
                    ..NotePatch::default()
                },
            )
            .expect("archives");

        assert_eq!(
            f.repository.count(&NoteFilter::default()).expect("counts"),
            0,
            "archived notes leave the main list"
        );
        assert_eq!(
            f.repository
                .count(&NoteFilter {
                    scope: NoteScope::Archived,
                    ..NoteFilter::default()
                })
                .expect("counts"),
            1
        );
    }

    #[test]
    fn sorting_by_title_is_case_insensitive() {
        let f = fixture();
        f.repository.create(draft("бета", "x")).expect("creates");
        f.repository.create(draft("Альфа", "x")).expect("creates");
        f.repository.create(draft("гамма", "x")).expect("creates");

        let page = f
            .repository
            .list(
                &NoteFilter::default(),
                NoteSort::TitleAsc,
                PageRequest::default(),
            )
            .expect("lists");

        let titles: Vec<&str> = page.items.iter().map(|note| note.title.as_str()).collect();
        assert_eq!(titles, vec!["Альфа", "бета", "гамма"]);
    }

    #[test]
    fn pagination_walks_the_whole_list_without_gaps_or_repeats() {
        let f = fixture();
        for index in 0..25 {
            f.repository
                .create(draft(&format!("Заметка {index:02}"), "текст"))
                .expect("creates");
        }

        let mut seen: Vec<NoteId> = Vec::new();
        for offset in (0..25).step_by(10) {
            let page = f
                .repository
                .list(
                    &NoteFilter::default(),
                    NoteSort::TitleAsc,
                    PageRequest::new(10, offset),
                )
                .expect("lists");
            assert_eq!(page.total, 25);
            seen.extend(page.items.iter().map(|note| note.id));
        }

        assert_eq!(seen.len(), 25);
        let unique: std::collections::HashSet<NoteId> = seen.iter().copied().collect();
        assert_eq!(unique.len(), 25, "pages must not overlap");
    }

    #[test]
    fn the_page_size_is_capped() {
        let page = PageRequest::new(10_000, 0);
        assert_eq!(page.limit, PageRequest::MAX_LIMIT);
        assert_eq!(PageRequest::new(0, 0).limit, 1);
    }

    #[test]
    fn a_duplicate_is_an_independent_copy() {
        let f = fixture();
        let source = f
            .repository
            .create(draft("Шаблон", "содержимое"))
            .expect("creates");

        let copy = f.repository.duplicate(source.id).expect("duplicates");

        assert_ne!(copy.id, source.id);
        assert_eq!(copy.title, "Шаблон (копия)");
        assert_eq!(copy.content_text, source.content_text);

        f.repository
            .update(
                copy.id,
                NotePatch {
                    content_text: Some("изменено".to_owned()),
                    ..NotePatch::default()
                },
            )
            .expect("updates the copy");

        let original = f
            .repository
            .find(source.id)
            .expect("query runs")
            .expect("still there");
        assert_eq!(
            original.content_text, "содержимое",
            "the original is untouched"
        );
    }

    #[test]
    fn duplicating_carries_over_nested_blocks_with_fresh_ids() {
        let f = fixture();
        let source = f.repository.create(draft("Список", "")).expect("creates");

        let parent = NoteBlockId::new();
        let child = NoteBlockId::new();
        f.database
            .in_transaction(|transaction| {
                transaction
                    .execute(
                        "INSERT INTO note_blocks (id, note_id, parent_block_id, block_type, text,
                             is_checked, position, created_at, updated_at)
                         VALUES (?1, ?2, NULL, 'checklist_item', 'купить', 0, 0, 0, 0)",
                        rusqlite::params![parent, source.id],
                    )
                    .map_err(AppError::from)?;
                transaction
                    .execute(
                        "INSERT INTO note_blocks (id, note_id, parent_block_id, block_type, text,
                             is_checked, position, created_at, updated_at)
                         VALUES (?1, ?2, ?3, 'checklist_item', 'молоко', 0, 0, 0, 0)",
                        rusqlite::params![child, source.id, parent],
                    )
                    .map_err(AppError::from)?;
                Ok(())
            })
            .expect("seeds blocks");

        let copy = f.repository.duplicate(source.id).expect("duplicates");

        let (count, orphans): (i64, i64) = f
            .database
            .with_connection(|connection| {
                let count = connection
                    .query_row(
                        "SELECT COUNT(*) FROM note_blocks WHERE note_id = ?1",
                        [&copy.id],
                        |row| row.get(0),
                    )
                    .map_err(AppError::from)?;
                // A child whose parent still points into the source note would
                // mean the remapping failed.
                let orphans = connection
                    .query_row(
                        "SELECT COUNT(*) FROM note_blocks child
                         WHERE child.note_id = ?1
                           AND child.parent_block_id IS NOT NULL
                           AND NOT EXISTS (SELECT 1 FROM note_blocks parent
                                            WHERE parent.id = child.parent_block_id
                                              AND parent.note_id = ?1)",
                        [&copy.id],
                        |row| row.get(0),
                    )
                    .map_err(AppError::from)?;
                Ok((count, orphans))
            })
            .expect("queries run");

        assert_eq!(count, 2, "both blocks are copied");
        assert_eq!(orphans, 0, "nesting is preserved inside the copy");
    }

    #[test]
    fn deleting_a_note_cascades_to_its_blocks() {
        let f = fixture();
        let note = f.repository.create(draft("Список", "")).expect("creates");
        f.database
            .in_transaction(|transaction| {
                transaction
                    .execute(
                        "INSERT INTO note_blocks (id, note_id, block_type, text, position,
                             created_at, updated_at)
                         VALUES (?1, ?2, 'checklist_item', 'молоко', 0, 0, 0)",
                        rusqlite::params![NoteBlockId::new(), note.id],
                    )
                    .map_err(AppError::from)?;
                Ok(())
            })
            .expect("seeds a block");

        f.repository.purge(note.id).expect("purges");

        let remaining: i64 = f
            .database
            .with_connection(|connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM note_blocks", [], |row| row.get(0))
                    .map_err(AppError::from)
            })
            .expect("counts");
        assert_eq!(remaining, 0, "orphaned blocks would leak storage forever");
    }

    #[test]
    fn filtering_by_type_narrows_the_list() {
        let f = fixture();
        f.repository
            .create(NoteDraft {
                note_type: Some(NoteType::Checklist),
                title: Some("Покупки".to_owned()),
                ..NoteDraft::default()
            })
            .expect("creates");
        f.repository
            .create(draft("Просто текст", "x"))
            .expect("creates");

        let page = f
            .repository
            .list(
                &NoteFilter {
                    note_type: Some(NoteType::Checklist),
                    ..NoteFilter::default()
                },
                NoteSort::default(),
                PageRequest::default(),
            )
            .expect("lists");

        assert_eq!(page.total, 1);
        assert_eq!(
            page.items.first().map(|note| note.title.as_str()),
            Some("Покупки")
        );
    }
}
