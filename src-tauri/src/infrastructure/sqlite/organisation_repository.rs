//! Tags and which notes wear them.

use std::collections::HashMap;
use std::sync::Arc;

use rusqlite::{params, Row};

use crate::domain::clock::Timestamp;
use crate::domain::ids::{NoteId, TagId};
use crate::domain::organisation::{LabelName, OrganisationRepository, Tag};
use crate::error::{AppError, AppResult, DatabaseError};

use super::Database;

pub struct SqliteOrganisationRepository {
    database: Arc<Database>,
}

impl SqliteOrganisationRepository {
    #[must_use]
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }
}

fn map_tag(row: &Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get(0)?,
        name: row.get(1)?,
        usage_count: row.get(2)?,
    })
}

/// Counted from the join table rather than from `tags.usage_count`: a stored
/// counter is one more thing that can drift, and the join is cheap at this size.
const SELECT_TAGS: &str = "SELECT t.id, t.name,
            (SELECT COUNT(*) FROM note_tags nt
               JOIN notes n ON n.id = nt.note_id
              WHERE nt.tag_id = t.id AND n.deleted_at IS NULL)
       FROM tags t";

fn not_found(entity: &'static str, id: &impl ToString) -> AppError {
    AppError::Database(DatabaseError::NotFound {
        entity,
        id: id.to_string(),
    })
}

impl OrganisationRepository for SqliteOrganisationRepository {
    fn tags(&self) -> AppResult<Vec<Tag>> {
        self.database.with_connection(|connection| {
            let mut statement = connection
                .prepare(&format!(
                    "{SELECT_TAGS} ORDER BY 3 DESC, t.name COLLATE NOCASE"
                ))
                .map_err(AppError::from)?;
            let rows = statement
                .query_map([], map_tag)
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;
            Ok(rows)
        })
    }

    fn ensure_tag(&self, name: &LabelName, now: Timestamp) -> AppResult<Tag> {
        self.database.in_transaction(|transaction| {
            // Every stored name is folded in Rust rather than compared in SQL:
            // SQLite's `NOCASE` collation — and its `lower()` — only fold
            // ASCII, so an index alone would let `Работа` and `работа` become
            // two tags that look identical in every list the user ever sees.
            let mut statement = transaction
                .prepare("SELECT id, name FROM tags")
                .map_err(AppError::from)?;
            let existing = statement
                .query_map([], |row| {
                    Ok((row.get::<_, TagId>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?
                .into_iter()
                .find(|(_, stored)| stored.to_lowercase() == name.folded())
                .map(|(id, _)| id);
            drop(statement);

            let id = match existing {
                Some(id) => id,
                None => {
                    let id = TagId::new();
                    transaction
                        .execute(
                            "INSERT INTO tags (id, name, created_at, updated_at)
                                  VALUES (?1, ?2, ?3, ?3)",
                            params![id, name.display(), now.as_millis()],
                        )
                        .map_err(AppError::from)?;
                    id
                }
            };

            transaction
                .query_row(&format!("{SELECT_TAGS} WHERE t.id = ?1"), [id], map_tag)
                .map_err(AppError::from)
        })
    }

    fn delete_tag(&self, id: TagId) -> AppResult<()> {
        self.database.with_connection(|connection| {
            // `note_tags` cascades, so the notes keep everything but the label.
            let deleted = connection
                .execute("DELETE FROM tags WHERE id = ?1", [id])
                .map_err(AppError::from)?;
            if deleted == 0 {
                return Err(not_found("tag", &id));
            }
            Ok(())
        })
    }

    fn tags_of_note(&self, note_id: NoteId) -> AppResult<Vec<Tag>> {
        self.database.with_connection(|connection| {
            let mut statement = connection
                .prepare(&format!(
                    "{SELECT_TAGS}
                      JOIN note_tags nt ON nt.tag_id = t.id
                     WHERE nt.note_id = ?1
                     ORDER BY t.name COLLATE NOCASE"
                ))
                .map_err(AppError::from)?;
            let rows = statement
                .query_map([note_id], map_tag)
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;
            Ok(rows)
        })
    }

    fn tags_of_notes(&self, notes: &[NoteId]) -> AppResult<HashMap<NoteId, Vec<Tag>>> {
        if notes.is_empty() {
            return Ok(HashMap::new());
        }

        self.database.with_connection(|connection| {
            // The placeholder list is built from the slice length rather than
            // from anything the user typed, so there is nothing here to inject.
            let placeholders = vec!["?"; notes.len()].join(", ");
            // Spelled out rather than built from `SELECT_TAGS`, which ends at
            // its `FROM`: `map_tag` reads the first three columns, so the note
            // the row belongs to has to come after them.
            let mut statement = connection
                .prepare(&format!(
                    "SELECT t.id, t.name,
                            (SELECT COUNT(*) FROM note_tags c
                               JOIN notes n ON n.id = c.note_id
                              WHERE c.tag_id = t.id AND n.deleted_at IS NULL),
                            nt.note_id
                       FROM tags t
                       JOIN note_tags nt ON nt.tag_id = t.id
                      WHERE nt.note_id IN ({placeholders})
                      ORDER BY t.name COLLATE NOCASE"
                ))
                .map_err(AppError::from)?;

            let ids = notes.iter().map(ToString::to_string);
            let mut by_note: HashMap<NoteId, Vec<Tag>> = HashMap::new();
            let mut rows = statement
                .query(rusqlite::params_from_iter(ids))
                .map_err(AppError::from)?;
            while let Some(row) = rows.next().map_err(AppError::from)? {
                let note_id: NoteId = row.get(3).map_err(AppError::from)?;
                by_note
                    .entry(note_id)
                    .or_default()
                    .push(map_tag(row).map_err(AppError::from)?);
            }
            Ok(by_note)
        })
    }

    fn set_note_tags(&self, note_id: NoteId, tags: &[TagId], now: Timestamp) -> AppResult<()> {
        self.database.in_transaction(|transaction| {
            // Checked here rather than in the use cases, and in the same
            // transaction as the write: a tag deleted between the check and the
            // insert would otherwise come back as a foreign-key failure, which
            // says «ошибка базы» where it means «такого тега нет».
            let mut exists = transaction
                .prepare("SELECT 1 FROM tags WHERE id = ?1")
                .map_err(AppError::from)?;
            for tag in tags {
                if !exists.exists([tag]).map_err(AppError::from)? {
                    return Err(not_found("tag", tag));
                }
            }
            drop(exists);

            transaction
                .execute("DELETE FROM note_tags WHERE note_id = ?1", [note_id])
                .map_err(AppError::from)?;
            for tag in tags {
                // `OR IGNORE` keeps the trait's promise that a repeated id is
                // one link: the scenarios already hand over a set, and a list
                // that says the same thing twice is not worth losing a save to.
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO note_tags (note_id, tag_id, created_at)
                              VALUES (?1, ?2, ?3)",
                        params![note_id, tag, now.as_millis()],
                    )
                    .map_err(AppError::from)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::clock::{FixedClock, SharedClock};
    use crate::domain::notes::{NoteDraft, NoteRepository};
    use crate::infrastructure::sqlite::SqliteNoteRepository;

    use super::*;

    fn label(name: &str) -> LabelName {
        LabelName::new(name, "name").expect("valid")
    }

    fn fixture() -> (SqliteOrganisationRepository, NoteId, SharedClock) {
        let clock: SharedClock = Arc::new(FixedClock::new(Timestamp::from_millis(1_000)));
        let database = Arc::new(Database::open_in_memory(1_000).expect("opens"));
        let note = SqliteNoteRepository::new(Arc::clone(&database), Arc::clone(&clock))
            .create(NoteDraft {
                title: Some("Заметка".into()),
                ..NoteDraft::default()
            })
            .expect("creates note");
        (
            SqliteOrganisationRepository::new(database),
            note.id,
            clock,
        )
    }

    #[test]
    fn the_tags_of_a_page_of_notes_come_back_note_by_note() {
        // The library asks for a whole page at once; the answer has to keep
        // saying which note each tag belongs to.
        let clock: SharedClock = Arc::new(FixedClock::new(Timestamp::from_millis(1_000)));
        let database = Arc::new(Database::open_in_memory(1_000).expect("opens"));
        let notes = SqliteNoteRepository::new(Arc::clone(&database), Arc::clone(&clock));
        let mut created = Vec::new();
        for title in ["Первая", "Вторая", "Третья"] {
            created.push(
                notes
                    .create(NoteDraft {
                        title: Some(title.into()),
                        ..NoteDraft::default()
                    })
                    .expect("creates note")
                    .id,
            );
        }
        let repository = SqliteOrganisationRepository::new(database);
        let work = repository
            .ensure_tag(&label("работа"), clock.now())
            .expect("creates");
        let home = repository
            .ensure_tag(&label("дом"), clock.now())
            .expect("creates");
        repository
            .set_note_tags(created[0], &[work.id, home.id], clock.now())
            .expect("sets");
        repository
            .set_note_tags(created[1], &[work.id], clock.now())
            .expect("sets");

        let by_note = repository.tags_of_notes(&created).expect("reads");
        let names = |note: NoteId| -> Vec<String> {
            by_note[&note].iter().map(|tag| tag.name.clone()).collect()
        };

        assert_eq!(
            names(created[0]),
            vec![home.name, work.name.clone()],
            "sorted by name, as everywhere else"
        );
        assert_eq!(names(created[1]), vec![work.name]);
        assert!(
            !by_note.contains_key(&created[2]),
            "a note without tags is absent rather than empty"
        );
    }

    #[test]
    fn asking_for_the_tags_of_no_notes_reaches_no_database() {
        let (repository, _note, _clock) = fixture();

        assert!(repository.tags_of_notes(&[]).expect("reads").is_empty());
    }

    #[test]
    fn a_tag_typed_in_another_case_is_the_same_tag() {
        let (repository, _note, clock) = fixture();

        let first = repository
            .ensure_tag(&label("Работа"), clock.now())
            .expect("creates");
        let second = repository
            .ensure_tag(&label("работа"), clock.now())
            .expect("finds");

        assert_eq!(first.id, second.id, "one tag, not two that look alike");
        assert_eq!(repository.tags().expect("reads").len(), 1);
    }

    #[test]
    fn setting_the_tags_of_a_note_replaces_rather_than_adds() {
        let (repository, note, clock) = fixture();
        let work = repository
            .ensure_tag(&label("работа"), clock.now())
            .expect("creates");
        let home = repository
            .ensure_tag(&label("дом"), clock.now())
            .expect("creates");

        repository
            .set_note_tags(note, &[work.id, home.id], clock.now())
            .expect("sets");
        repository
            .set_note_tags(note, &[home.id], clock.now())
            .expect("replaces");

        let on_note = repository.tags_of_note(note).expect("reads");
        assert_eq!(on_note.len(), 1);
        assert_eq!(on_note[0].name, "дом");
    }

    #[test]
    fn a_tag_counts_only_the_notes_that_still_exist() {
        let (repository, note, clock) = fixture();
        let tag = repository
            .ensure_tag(&label("работа"), clock.now())
            .expect("creates");
        repository
            .set_note_tags(note, &[tag.id], clock.now())
            .expect("sets");

        assert_eq!(repository.tags().expect("reads")[0].usage_count, 1);
    }

    #[test]
    fn deleting_a_tag_leaves_the_notes_alone() {
        let (repository, note, clock) = fixture();
        let tag = repository
            .ensure_tag(&label("работа"), clock.now())
            .expect("creates");
        repository
            .set_note_tags(note, &[tag.id], clock.now())
            .expect("sets");

        repository.delete_tag(tag.id).expect("deletes");

        assert!(repository.tags().expect("reads").is_empty());
        assert!(
            repository.tags_of_note(note).expect("reads").is_empty(),
            "the label is gone from the note, but the note is not"
        );
    }

    #[test]
    fn the_same_tag_named_twice_in_a_set_is_one_link() {
        let (repository, note, clock) = fixture();
        let work = repository
            .ensure_tag(&label("работа"), clock.now())
            .expect("creates");

        repository
            .set_note_tags(note, &[work.id, work.id], clock.now())
            .expect("a repeat is not an error");

        let on_note = repository.tags_of_note(note).expect("reads");
        assert_eq!(on_note.len(), 1);
        assert_eq!(on_note[0].id, work.id);
    }

    #[test]
    fn deleting_a_tag_that_is_not_there_is_reported_rather_than_passed_over() {
        let (repository, _note, _clock) = fixture();
        let absent = TagId::new();

        let error = repository
            .delete_tag(absent)
            .expect_err("nothing to delete");

        assert_eq!(error.code(), "not_found");
        assert!(error.to_string().contains(&absent.to_string()));
    }

    #[test]
    fn tagging_a_note_with_a_tag_that_does_not_exist_says_so() {
        let (repository, note, clock) = fixture();
        let work = repository
            .ensure_tag(&label("работа"), clock.now())
            .expect("creates");
        repository
            .set_note_tags(note, &[work.id], clock.now())
            .expect("sets");
        let absent = TagId::new();

        let error = repository
            .set_note_tags(note, &[work.id, absent], clock.now())
            .expect_err("must refuse");

        assert_eq!(
            error.code(),
            "not_found",
            "the foreign key would have said «ошибка базы» instead"
        );
        assert!(error.to_string().contains(&absent.to_string()));
        let on_note = repository.tags_of_note(note).expect("reads");
        assert_eq!(
            on_note.iter().map(|tag| tag.id).collect::<Vec<_>>(),
            vec![work.id],
            "the refused set is rolled back whole"
        );
    }

    #[test]
    fn a_tag_keeps_the_case_it_was_first_typed_in() {
        let (repository, _note, clock) = fixture();

        repository
            .ensure_tag(&label("Работа"), clock.now())
            .expect("creates");
        repository
            .ensure_tag(&label("работа"), clock.now())
            .expect("finds");

        assert_eq!(repository.tags().expect("reads")[0].name, "Работа");
    }
}
