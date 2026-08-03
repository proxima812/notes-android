//! Filing notes: folders and tags.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;

use crate::domain::clock::SharedClock;
use crate::domain::ids::{FolderId, NoteId, TagId};
use crate::domain::organisation::{Folder, LabelName, OrganisationRepository, Tag};
use crate::error::AppResult;

/// Keeps the first of each id and drops the repeats.
///
/// The frontend sends the list the user is looking at, and a list can name the
/// same label twice; making that a set here means every repository is handed
/// one, instead of each having to survive the repeat on its own.
fn as_set<T: Copy + Eq + Hash>(ids: Vec<T>) -> Vec<T> {
    let mut seen = HashSet::with_capacity(ids.len());
    let mut set = ids;
    set.retain(|id| seen.insert(*id));
    set
}

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

    /// Finds a tag by name or creates it.
    ///
    /// The name is checked and folded here rather than in the repository, so
    /// that what the user is promised — a trimmed label, a length a chip can
    /// show, `#Работа` and `#работа` as one tag — holds whoever stores it.
    ///
    /// # Errors
    /// Fails on validation or a database error.
    pub fn ensure_tag(&self, name: &str) -> AppResult<Tag> {
        let name = LabelName::new(name, "name")?;
        self.organisation.ensure_tag(&name, self.clock.now())
    }

    /// # Errors
    /// Fails for a malformed identifier, for a tag that does not exist, or on a
    /// database error.
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
    /// out of step with the one the user is looking at. A tag named twice in
    /// that list is one tag.
    ///
    /// # Errors
    /// Fails for a malformed identifier, for a tag that does not exist, or on a
    /// database error.
    pub fn set_note_tags(&self, note_id: &str, tags: &[String]) -> AppResult<Vec<Tag>> {
        let note_id = NoteId::parse(note_id)?;
        let ids = as_set(
            tags.iter()
                .map(|id| TagId::parse(id))
                .collect::<AppResult<Vec<_>>>()?,
        );
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
        let name = LabelName::new(name, "name")?;
        self.organisation.create_folder(&name, self.clock.now())
    }

    /// # Errors
    /// Fails for a malformed identifier, for a folder that does not exist, or
    /// on a database error.
    pub fn delete_folder(&self, id: &str) -> AppResult<()> {
        self.organisation.delete_folder(FolderId::parse(id)?)
    }

    /// # Errors
    /// Fails for a malformed identifier or on a database error.
    pub fn folders_of_note(&self, note_id: &str) -> AppResult<Vec<Folder>> {
        self.organisation.folders_of_note(NoteId::parse(note_id)?)
    }

    /// Replaces the whole set of folders a note is filed under, on the same
    /// terms as [`Self::set_note_tags`].
    ///
    /// # Errors
    /// Fails for a malformed identifier, for a folder that does not exist, or
    /// on a database error.
    pub fn set_note_folders(&self, note_id: &str, folders: &[String]) -> AppResult<Vec<Folder>> {
        let note_id = NoteId::parse(note_id)?;
        let ids = as_set(
            folders
                .iter()
                .map(|id| FolderId::parse(id))
                .collect::<AppResult<Vec<_>>>()?,
        );
        self.organisation
            .set_note_folders(note_id, &ids, self.clock.now())?;
        self.organisation.folders_of_note(note_id)
    }
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use crate::domain::clock::{FixedClock, Timestamp};
    use crate::domain::organisation::MAX_LABEL;
    use crate::error::{AppError, DatabaseError};

    use super::*;

    fn not_found(entity: &'static str, id: &impl ToString) -> AppError {
        AppError::Database(DatabaseError::NotFound {
            entity,
            id: id.to_string(),
        })
    }

    /// Stands in for the database, and keeps the same promises the trait makes:
    /// tags are matched by their folded name, a set that repeats an id makes
    /// one link, an id that names nothing is refused rather than written, a
    /// label that goes away takes its links with it, and the notes themselves
    /// are never written to from here.
    #[derive(Default)]
    struct FakeOrganisation {
        tags: Mutex<Vec<Tag>>,
        folders: Mutex<Vec<Folder>>,
        note_tags: Mutex<Vec<(NoteId, TagId)>>,
        note_folders: Mutex<Vec<(NoteId, FolderId)>>,
        /// A stand-in for the `notes` table, so a test can say what deleting a
        /// label did *not* do.
        notes: Mutex<Vec<(NoteId, String)>>,
        /// Every method the use cases reached, in order.
        calls: Mutex<Vec<&'static str>>,
    }

    impl FakeOrganisation {
        fn add_note(&self, title: &str) -> NoteId {
            let id = NoteId::new();
            self.notes.lock().push((id, title.to_owned()));
            id
        }
    }

    impl OrganisationRepository for FakeOrganisation {
        fn tags(&self) -> AppResult<Vec<Tag>> {
            self.calls.lock().push("tags");
            Ok(self.tags.lock().clone())
        }

        fn ensure_tag(&self, name: &LabelName, _now: Timestamp) -> AppResult<Tag> {
            self.calls.lock().push("ensure_tag");
            let mut tags = self.tags.lock();
            if let Some(existing) = tags
                .iter()
                .find(|tag| tag.name.to_lowercase() == name.folded())
            {
                return Ok(existing.clone());
            }
            let tag = Tag {
                id: TagId::new(),
                name: name.display().to_owned(),
                usage_count: 0,
            };
            tags.push(tag.clone());
            Ok(tag)
        }

        fn delete_tag(&self, id: TagId) -> AppResult<()> {
            self.calls.lock().push("delete_tag");
            let mut tags = self.tags.lock();
            let before = tags.len();
            tags.retain(|tag| tag.id != id);
            if tags.len() == before {
                return Err(not_found("tag", &id));
            }
            self.note_tags.lock().retain(|(_, tag)| *tag != id);
            Ok(())
        }

        fn tags_of_note(&self, note_id: NoteId) -> AppResult<Vec<Tag>> {
            self.calls.lock().push("tags_of_note");
            let links = self.note_tags.lock();
            Ok(self
                .tags
                .lock()
                .iter()
                .filter(|tag| {
                    links
                        .iter()
                        .any(|(note, id)| *note == note_id && *id == tag.id)
                })
                .cloned()
                .collect())
        }

        fn set_note_tags(&self, note_id: NoteId, tags: &[TagId], _now: Timestamp) -> AppResult<()> {
            self.calls.lock().push("set_note_tags");
            let known = self.tags.lock();
            if let Some(missing) = tags
                .iter()
                .find(|id| !known.iter().any(|tag| tag.id == **id))
            {
                return Err(not_found("tag", missing));
            }
            drop(known);
            let mut links = self.note_tags.lock();
            links.retain(|(note, _)| *note != note_id);
            links.extend(tags.iter().map(|tag| (note_id, *tag)));
            Ok(())
        }

        fn folders(&self) -> AppResult<Vec<Folder>> {
            self.calls.lock().push("folders");
            Ok(self.folders.lock().clone())
        }

        fn create_folder(&self, name: &LabelName, _now: Timestamp) -> AppResult<Folder> {
            self.calls.lock().push("create_folder");
            let folder = Folder {
                id: FolderId::new(),
                name: name.display().to_owned(),
                note_count: 0,
            };
            self.folders.lock().push(folder.clone());
            Ok(folder)
        }

        fn delete_folder(&self, id: FolderId) -> AppResult<()> {
            self.calls.lock().push("delete_folder");
            let mut folders = self.folders.lock();
            let before = folders.len();
            folders.retain(|folder| folder.id != id);
            if folders.len() == before {
                return Err(not_found("folder", &id));
            }
            self.note_folders.lock().retain(|(_, folder)| *folder != id);
            Ok(())
        }

        fn folders_of_note(&self, note_id: NoteId) -> AppResult<Vec<Folder>> {
            self.calls.lock().push("folders_of_note");
            let links = self.note_folders.lock();
            Ok(self
                .folders
                .lock()
                .iter()
                .filter(|folder| {
                    links
                        .iter()
                        .any(|(note, id)| *note == note_id && *id == folder.id)
                })
                .cloned()
                .collect())
        }

        fn set_note_folders(
            &self,
            note_id: NoteId,
            folders: &[FolderId],
            _now: Timestamp,
        ) -> AppResult<()> {
            self.calls.lock().push("set_note_folders");
            let known = self.folders.lock();
            if let Some(missing) = folders
                .iter()
                .find(|id| !known.iter().any(|folder| folder.id == **id))
            {
                return Err(not_found("folder", missing));
            }
            drop(known);
            let mut links = self.note_folders.lock();
            links.retain(|(note, _)| *note != note_id);
            links.extend(folders.iter().map(|folder| (note_id, *folder)));
            Ok(())
        }
    }

    fn fixture() -> (OrganisationUseCases, Arc<FakeOrganisation>) {
        let repository = Arc::new(FakeOrganisation::default());
        let clock: SharedClock = Arc::new(FixedClock::new(Timestamp::from_millis(1_000)));
        (
            OrganisationUseCases::new(
                Arc::clone(&repository) as Arc<dyn OrganisationRepository>,
                clock,
            ),
            repository,
        )
    }

    #[test]
    fn setting_the_tags_of_a_note_replaces_rather_than_adds() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let work = organisation.ensure_tag("работа").expect("creates");
        let home = organisation.ensure_tag("дом").expect("creates");

        organisation
            .set_note_tags(&note.to_string(), &[work.id.to_string()])
            .expect("sets");
        let remaining = organisation
            .set_note_tags(&note.to_string(), &[home.id.to_string()])
            .expect("replaces");

        assert_eq!(
            remaining,
            vec![home],
            "the list arrives whole, so what is missing from it is meant to go"
        );
    }

    #[test]
    fn setting_an_empty_list_of_tags_takes_them_all_off() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let work = organisation.ensure_tag("работа").expect("creates");
        organisation
            .set_note_tags(&note.to_string(), &[work.id.to_string()])
            .expect("sets");

        let remaining = organisation
            .set_note_tags(&note.to_string(), &[])
            .expect("clears");

        assert!(remaining.is_empty());
    }

    #[test]
    fn setting_the_folders_of_a_note_replaces_rather_than_adds() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let projects = organisation.create_folder("Проекты").expect("creates");
        let archive = organisation.create_folder("Архив").expect("creates");

        organisation
            .set_note_folders(&note.to_string(), &[projects.id.to_string()])
            .expect("files");
        let remaining = organisation
            .set_note_folders(&note.to_string(), &[archive.id.to_string()])
            .expect("refiles");

        assert_eq!(remaining, vec![archive]);
    }

    #[test]
    fn a_tag_typed_in_another_case_is_the_same_tag() {
        let (organisation, _repository) = fixture();

        let first = organisation.ensure_tag("Работа").expect("creates");
        let second = organisation.ensure_tag("работа").expect("finds");

        assert_eq!(
            first.id, second.id,
            "one tag, not two that look identical in every list the user sees"
        );
        assert_eq!(organisation.tags().expect("reads").len(), 1);
    }

    #[test]
    fn a_tag_typed_with_stray_spaces_is_the_same_tag() {
        let (organisation, _repository) = fixture();

        let first = organisation.ensure_tag("работа").expect("creates");
        let second = organisation.ensure_tag("  работа  ").expect("finds");

        assert_eq!(first.id, second.id);
    }

    #[test]
    fn deleting_a_tag_takes_the_label_off_the_note_but_not_the_note() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let work = organisation.ensure_tag("работа").expect("creates");
        organisation
            .set_note_tags(&note.to_string(), &[work.id.to_string()])
            .expect("sets");
        let notes_before = repository.notes.lock().clone();

        organisation
            .delete_tag(&work.id.to_string())
            .expect("deletes");

        assert!(organisation.tags().expect("reads").is_empty());
        assert!(organisation
            .tags_of_note(&note.to_string())
            .expect("reads")
            .is_empty());
        assert_eq!(
            *repository.notes.lock(),
            notes_before,
            "a label is a label; removing it must not remove what wore it"
        );
    }

    #[test]
    fn deleting_a_folder_leaves_the_notes_that_were_in_it() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let projects = organisation.create_folder("Проекты").expect("creates");
        organisation
            .set_note_folders(&note.to_string(), &[projects.id.to_string()])
            .expect("files");
        let notes_before = repository.notes.lock().clone();

        organisation
            .delete_folder(&projects.id.to_string())
            .expect("deletes");

        assert!(organisation.folders().expect("reads").is_empty());
        assert!(organisation
            .folders_of_note(&note.to_string())
            .expect("reads")
            .is_empty());
        assert_eq!(
            *repository.notes.lock(),
            notes_before,
            "emptying a drawer is not the same as throwing away what was in it"
        );
    }

    #[test]
    fn a_tag_id_that_is_not_an_identifier_never_reaches_the_repository() {
        let (organisation, repository) = fixture();

        let error = organisation
            .delete_tag("tag-1")
            .expect_err("a plain word is not an id");

        assert_eq!(error.code(), "validation_invalid");
        assert!(
            repository.calls.lock().is_empty(),
            "malformed input is refused before anything can be written"
        );
    }

    #[test]
    fn a_note_id_that_is_not_an_identifier_never_reaches_the_repository() {
        let (organisation, repository) = fixture();

        for error in [
            organisation
                .tags_of_note("note-1")
                .expect_err("must refuse"),
            organisation
                .folders_of_note("note-1")
                .expect_err("must refuse"),
            organisation
                .set_note_tags("note-1", &[])
                .expect_err("must refuse"),
            organisation
                .set_note_folders("note-1", &[])
                .expect_err("must refuse"),
        ] {
            assert_eq!(error.code(), "validation_invalid");
        }
        assert!(repository.calls.lock().is_empty());
    }

    #[test]
    fn one_malformed_tag_id_leaves_the_whole_set_as_it_was() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let work = organisation.ensure_tag("работа").expect("creates");
        organisation
            .set_note_tags(&note.to_string(), &[work.id.to_string()])
            .expect("sets");
        repository.calls.lock().clear();

        let error = organisation
            .set_note_tags(
                &note.to_string(),
                &[work.id.to_string(), "not-a-tag".to_owned()],
            )
            .expect_err("must refuse");

        assert_eq!(error.code(), "validation_invalid");
        assert!(
            !repository.calls.lock().contains(&"set_note_tags"),
            "a set that is only half understood must not be written at all"
        );
        assert_eq!(
            organisation.tags_of_note(&note.to_string()).expect("reads"),
            vec![work]
        );
    }

    #[test]
    fn one_malformed_folder_id_leaves_the_whole_set_as_it_was() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");

        let error = organisation
            .set_note_folders(&note.to_string(), &["not-a-folder".to_owned()])
            .expect_err("must refuse");

        assert_eq!(error.code(), "validation_invalid");
        assert!(!repository.calls.lock().contains(&"set_note_folders"));
    }

    #[test]
    fn deleting_a_tag_that_is_not_there_is_reported_rather_than_passed_over() {
        let (organisation, _repository) = fixture();
        let absent = TagId::new();

        let error = organisation
            .delete_tag(&absent.to_string())
            .expect_err("nothing to delete");

        assert_eq!(error.code(), "not_found");
        assert!(
            error.to_string().contains(&absent.to_string()),
            "the error names the id that was missed"
        );
    }

    #[test]
    fn deleting_a_folder_that_is_not_there_is_reported_rather_than_passed_over() {
        let (organisation, _repository) = fixture();
        let absent = FolderId::new();

        let error = organisation
            .delete_folder(&absent.to_string())
            .expect_err("nothing to delete");

        assert_eq!(error.code(), "not_found");
        assert!(error.to_string().contains(&absent.to_string()));
    }

    #[test]
    fn deleting_the_same_tag_twice_succeeds_only_once() {
        let (organisation, _repository) = fixture();
        let work = organisation.ensure_tag("работа").expect("creates");

        organisation
            .delete_tag(&work.id.to_string())
            .expect("deletes");
        let error = organisation
            .delete_tag(&work.id.to_string())
            .expect_err("already gone");

        assert_eq!(error.code(), "not_found");
    }

    #[test]
    fn a_tag_that_does_not_exist_leaves_the_whole_set_as_it_was() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let work = organisation.ensure_tag("работа").expect("creates");
        organisation
            .set_note_tags(&note.to_string(), &[work.id.to_string()])
            .expect("sets");
        let absent = TagId::new();

        let error = organisation
            .set_note_tags(
                &note.to_string(),
                &[work.id.to_string(), absent.to_string()],
            )
            .expect_err("must refuse");

        assert_eq!(error.code(), "not_found");
        assert!(
            error.to_string().contains(&absent.to_string()),
            "the error names the tag that is missing, not the set"
        );
        assert_eq!(
            organisation.tags_of_note(&note.to_string()).expect("reads"),
            vec![work],
            "a set that is only half known must not be written at all"
        );
    }

    #[test]
    fn a_folder_that_does_not_exist_leaves_the_whole_set_as_it_was() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let projects = organisation.create_folder("Проекты").expect("creates");
        organisation
            .set_note_folders(&note.to_string(), &[projects.id.to_string()])
            .expect("files");
        let absent = FolderId::new();

        let error = organisation
            .set_note_folders(&note.to_string(), &[absent.to_string()])
            .expect_err("must refuse");

        assert_eq!(error.code(), "not_found");
        assert!(error.to_string().contains(&absent.to_string()));
        assert_eq!(
            organisation
                .folders_of_note(&note.to_string())
                .expect("reads"),
            vec![projects]
        );
    }

    #[test]
    fn a_folder_without_a_name_is_refused() {
        let (organisation, repository) = fixture();

        let error = organisation.create_folder("   ").expect_err("must refuse");

        assert_eq!(error.code(), "validation_required");
        assert!(organisation.folders().expect("reads").is_empty());
        assert!(
            !repository.calls.lock().contains(&"create_folder"),
            "the name is refused here, not by whoever stores it"
        );
    }

    #[test]
    fn a_tag_without_a_name_is_refused() {
        let (organisation, repository) = fixture();

        let error = organisation.ensure_tag("").expect_err("must refuse");

        assert_eq!(error.code(), "validation_required");
        assert!(organisation.tags().expect("reads").is_empty());
        assert!(!repository.calls.lock().contains(&"ensure_tag"));
    }

    #[test]
    fn a_label_too_long_to_show_never_reaches_the_repository() {
        let (organisation, repository) = fixture();
        let long = "я".repeat(MAX_LABEL + 1);

        for error in [
            organisation.ensure_tag(&long).expect_err("must refuse"),
            organisation.create_folder(&long).expect_err("must refuse"),
        ] {
            assert_eq!(error.code(), "validation_too_long");
        }
        assert!(repository.calls.lock().is_empty());
    }

    #[test]
    fn a_tag_named_twice_in_a_set_is_filed_once() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let work = organisation.ensure_tag("работа").expect("creates");

        let on_note = organisation
            .set_note_tags(
                &note.to_string(),
                &[work.id.to_string(), work.id.to_string()],
            )
            .expect("a repeat is not an error");

        assert_eq!(on_note, vec![work]);
        assert_eq!(
            repository.note_tags.lock().len(),
            1,
            "the repository is handed a set, so it never has to fold one itself"
        );
    }

    #[test]
    fn a_folder_named_twice_in_a_set_is_filed_once() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let projects = organisation.create_folder("Проекты").expect("creates");

        let on_note = organisation
            .set_note_folders(
                &note.to_string(),
                &[projects.id.to_string(), projects.id.to_string()],
            )
            .expect("a repeat is not an error");

        assert_eq!(on_note, vec![projects]);
        assert_eq!(repository.note_folders.lock().len(), 1);
    }

    #[test]
    fn a_set_keeps_the_order_it_arrived_in_when_repeats_are_dropped() {
        let (organisation, repository) = fixture();
        let note = repository.add_note("Заметка");
        let work = organisation.ensure_tag("работа").expect("creates");
        let home = organisation.ensure_tag("дом").expect("creates");

        organisation
            .set_note_tags(
                &note.to_string(),
                &[
                    work.id.to_string(),
                    home.id.to_string(),
                    work.id.to_string(),
                ],
            )
            .expect("sets");

        assert_eq!(
            *repository.note_tags.lock(),
            vec![(note, work.id), (note, home.id)],
            "the first mention is the one that stays"
        );
    }
}
