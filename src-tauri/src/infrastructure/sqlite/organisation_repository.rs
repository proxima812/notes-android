//! Folders, tags, and which notes wear them.

use std::sync::Arc;

use rusqlite::{params, Row};

use crate::domain::clock::{SharedClock, Timestamp};
use crate::domain::ids::{FolderId, NoteId, TagId};
use crate::domain::organisation::{Folder, LabelName, OrganisationRepository, Tag};
use crate::error::{AppError, AppResult, DatabaseError};

use super::Database;

pub struct SqliteOrganisationRepository {
    database: Arc<Database>,
    clock: SharedClock,
}

impl SqliteOrganisationRepository {
    #[must_use]
    pub fn new(database: Arc<Database>, clock: SharedClock) -> Self {
        Self { database, clock }
    }
}

fn map_tag(row: &Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get(0)?,
        name: row.get(1)?,
        usage_count: row.get(2)?,
    })
}

fn map_folder(row: &Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get(0)?,
        name: row.get(1)?,
        note_count: row.get(2)?,
    })
}

/// Counted from the join table rather than from `tags.usage_count`: a stored
/// counter is one more thing that can drift, and the join is cheap at this size.
const SELECT_TAGS: &str = "SELECT t.id, t.name,
            (SELECT COUNT(*) FROM note_tags nt
               JOIN notes n ON n.id = nt.note_id
              WHERE nt.tag_id = t.id AND n.deleted_at IS NULL)
       FROM tags t";

const SELECT_FOLDERS: &str = "SELECT f.id, f.name,
            (SELECT COUNT(*) FROM note_folders nf
               JOIN notes n ON n.id = nf.note_id
              WHERE nf.folder_id = f.id AND n.deleted_at IS NULL)
       FROM folders f";

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

    fn folders(&self) -> AppResult<Vec<Folder>> {
        self.database.with_connection(|connection| {
            let mut statement = connection
                .prepare(&format!(
                    "{SELECT_FOLDERS}
                     WHERE f.deleted_at IS NULL
                     ORDER BY f.name COLLATE NOCASE"
                ))
                .map_err(AppError::from)?;
            let rows = statement
                .query_map([], map_folder)
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;
            Ok(rows)
        })
    }

    fn create_folder(&self, name: &LabelName, now: Timestamp) -> AppResult<Folder> {
        self.database.in_transaction(|transaction| {
            let id = FolderId::new();
            transaction
                .execute(
                    "INSERT INTO folders (id, name, created_at, updated_at)
                          VALUES (?1, ?2, ?3, ?3)",
                    params![id, name.display(), now.as_millis()],
                )
                .map_err(AppError::from)?;
            transaction
                .query_row(
                    &format!("{SELECT_FOLDERS} WHERE f.id = ?1"),
                    [id],
                    map_folder,
                )
                .map_err(AppError::from)
        })
    }

    fn delete_folder(&self, id: FolderId) -> AppResult<()> {
        let now = self.clock.now();
        self.database.with_connection(|connection| {
            // Soft delete, like notes: the folder disappears from the list and
            // the notes that were in it stay exactly where they were.
            let deleted = connection
                .execute(
                    "UPDATE folders SET deleted_at = ?1, updated_at = ?1
                      WHERE id = ?2 AND deleted_at IS NULL",
                    params![now.as_millis(), id],
                )
                .map_err(AppError::from)?;
            if deleted == 0 {
                return Err(not_found("folder", &id));
            }
            Ok(())
        })
    }

    fn folders_of_note(&self, note_id: NoteId) -> AppResult<Vec<Folder>> {
        self.database.with_connection(|connection| {
            let mut statement = connection
                .prepare(&format!(
                    "{SELECT_FOLDERS}
                      JOIN note_folders nf ON nf.folder_id = f.id
                     WHERE nf.note_id = ?1 AND f.deleted_at IS NULL
                     ORDER BY f.name COLLATE NOCASE"
                ))
                .map_err(AppError::from)?;
            let rows = statement
                .query_map([note_id], map_folder)
                .map_err(AppError::from)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;
            Ok(rows)
        })
    }

    fn set_note_folders(
        &self,
        note_id: NoteId,
        folders: &[FolderId],
        now: Timestamp,
    ) -> AppResult<()> {
        self.database.in_transaction(|transaction| {
            // A soft-deleted folder is gone as far as every list is concerned,
            // so filing into one is a miss rather than a silent success.
            let mut exists = transaction
                .prepare("SELECT 1 FROM folders WHERE id = ?1 AND deleted_at IS NULL")
                .map_err(AppError::from)?;
            for folder in folders {
                if !exists.exists([folder]).map_err(AppError::from)? {
                    return Err(not_found("folder", folder));
                }
            }
            drop(exists);

            transaction
                .execute("DELETE FROM note_folders WHERE note_id = ?1", [note_id])
                .map_err(AppError::from)?;
            for folder in folders {
                // `OR IGNORE` for the same reason as in `set_note_tags`.
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO note_folders (note_id, folder_id, created_at)
                              VALUES (?1, ?2, ?3)",
                        params![note_id, folder, now.as_millis()],
                    )
                    .map_err(AppError::from)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::clock::FixedClock;
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
            SqliteOrganisationRepository::new(database, Arc::clone(&clock)),
            note.id,
            clock,
        )
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
    fn a_deleted_folder_leaves_the_list_without_taking_its_notes() {
        let (repository, note, clock) = fixture();
        let folder = repository
            .create_folder(&label("Проекты"), clock.now())
            .expect("creates");
        repository
            .set_note_folders(note, &[folder.id], clock.now())
            .expect("files");

        repository.delete_folder(folder.id).expect("deletes");

        assert!(repository.folders().expect("reads").is_empty());
        assert!(repository.folders_of_note(note).expect("reads").is_empty());
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
    fn the_same_folder_named_twice_in_a_set_is_one_link() {
        let (repository, note, clock) = fixture();
        let projects = repository
            .create_folder(&label("Проекты"), clock.now())
            .expect("creates");

        repository
            .set_note_folders(note, &[projects.id, projects.id], clock.now())
            .expect("a repeat is not an error");

        let on_note = repository.folders_of_note(note).expect("reads");
        assert_eq!(on_note.len(), 1);
        assert_eq!(on_note[0].id, projects.id);
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
    fn deleting_a_folder_twice_succeeds_only_once() {
        let (repository, _note, clock) = fixture();
        let folder = repository
            .create_folder(&label("Проекты"), clock.now())
            .expect("creates");

        repository.delete_folder(folder.id).expect("deletes");
        let error = repository
            .delete_folder(folder.id)
            .expect_err("already gone");

        assert_eq!(
            error.code(),
            "not_found",
            "a folder the user can no longer see is not there to delete again"
        );
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
    fn filing_a_note_into_a_folder_that_does_not_exist_says_so() {
        let (repository, note, clock) = fixture();
        let absent = FolderId::new();

        let error = repository
            .set_note_folders(note, &[absent], clock.now())
            .expect_err("must refuse");

        assert_eq!(error.code(), "not_found");
        assert!(error.to_string().contains(&absent.to_string()));
    }

    #[test]
    fn filing_a_note_into_a_deleted_folder_says_so() {
        let (repository, note, clock) = fixture();
        let folder = repository
            .create_folder(&label("Проекты"), clock.now())
            .expect("creates");
        repository.delete_folder(folder.id).expect("deletes");

        let error = repository
            .set_note_folders(note, &[folder.id], clock.now())
            .expect_err("must refuse");

        assert_eq!(
            error.code(),
            "not_found",
            "a soft-deleted folder is gone from every list, so filing into it is a miss"
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
