//! Making a backup and putting one back.
//!
//! Both operations stage the file in the app's own directory first and only
//! then involve the user's storage. Exporting writes a snapshot, then asks
//! where to keep it; importing copies the chosen file in, then decides whether
//! it is acceptable — so a file the app will refuse never gets as far as
//! touching the live database.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::domain::backup::{
    ensure_restorable, file_name_for, BackupArchive, BackupRecord, BackupRepository,
    BACKUP_MIME_TYPE,
};
use crate::domain::clock::SharedClock;
use crate::domain::reminders::{resolve_sound, ReminderRepository};
use crate::error::AppResult;
use crate::infrastructure::sqlite::migrations;
use crate::platform::{AlarmClock, DocumentStore};

use super::use_cases::alarm_from;

/// What happened, in terms the user can be told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupOutcome {
    /// False when the user backed out of the picker. Not an error.
    pub completed: bool,
    /// The name the file ended up with, when there is one.
    pub file_name: Option<String>,
    pub note_count: i64,
    pub reminder_count: i64,
    pub size_bytes: u64,
}

impl BackupOutcome {
    fn cancelled() -> Self {
        Self {
            completed: false,
            file_name: None,
            note_count: 0,
            reminder_count: 0,
            size_bytes: 0,
        }
    }
}

pub struct BackupUseCases {
    archive: Arc<dyn BackupArchive>,
    history: Arc<dyn BackupRepository>,
    reminders: Arc<dyn ReminderRepository>,
    alarms: Arc<dyn AlarmClock>,
    documents: Arc<dyn DocumentStore>,
    clock: SharedClock,
    /// Scratch space inside the app's own directory. Cleared as we go: a copy
    /// of every note is not something to leave lying about after the user has
    /// already put the real one where they wanted it.
    staging_dir: PathBuf,
}

impl BackupUseCases {
    #[must_use]
    pub fn new(
        archive: Arc<dyn BackupArchive>,
        history: Arc<dyn BackupRepository>,
        reminders: Arc<dyn ReminderRepository>,
        alarms: Arc<dyn AlarmClock>,
        documents: Arc<dyn DocumentStore>,
        clock: SharedClock,
        staging_dir: PathBuf,
    ) -> Self {
        Self {
            archive,
            history,
            reminders,
            alarms,
            documents,
            clock,
            staging_dir,
        }
    }

    /// Writes a snapshot and asks the user where to keep it.
    ///
    /// # Errors
    /// Fails when the snapshot cannot be written or the chosen destination
    /// cannot be. A user who backs out of the picker is not a failure.
    pub fn export(&self, zone: &str) -> AppResult<BackupOutcome> {
        let now = self.clock.now();
        let name = file_name_for(now, zone.parse().unwrap_or(chrono_tz::UTC));
        let staged = self.staging_dir.join(&name);

        self.archive.snapshot_to(&staged)?;
        let outcome = (|| {
            let contents = self.archive.inspect(&staged)?;
            let picked =
                self.documents
                    .export(&staged.to_string_lossy(), &name, BACKUP_MIME_TYPE)?;
            if !picked.completed {
                return Ok(BackupOutcome::cancelled());
            }

            let file_name = picked.display_name.clone().unwrap_or_else(|| name.clone());
            self.history.record(&BackupRecord {
                location: picked.display_name.unwrap_or_default(),
                file_name: file_name.clone(),
                size_bytes: contents.size_bytes,
                sha256: contents.sha256,
                note_count: contents.note_count,
                created_at: now,
            })?;

            Ok(BackupOutcome {
                completed: true,
                file_name: Some(file_name),
                note_count: contents.note_count,
                reminder_count: contents.reminder_count,
                size_bytes: contents.size_bytes,
            })
        })();

        discard(&staged);
        outcome
    }

    /// Asks the user for a backup and puts it back.
    ///
    /// Everything currently in the app is replaced, so the alarms the OS holds
    /// are taken back first and re-armed afterwards from what the restored
    /// database says. Skipping that would leave notifications firing for
    /// reminders that no longer exist.
    ///
    /// # Errors
    /// Fails when the chosen file is not a backup of ours, when it comes from a
    /// newer build, or when the restore itself fails.
    pub fn import(&self) -> AppResult<BackupOutcome> {
        let staged = self.staging_dir.join("incoming.sqlite");
        let outcome = (|| {
            let picked = self
                .documents
                .import(&staged.to_string_lossy(), BACKUP_MIME_TYPE)?;
            if !picked.completed {
                return Ok(BackupOutcome::cancelled());
            }

            let contents = self.archive.inspect(&staged)?;
            ensure_restorable(&contents, migrations::latest_version())?;

            self.alarms.cancel_all()?;
            self.archive.restore_from(&staged)?;
            let rearmed = self.rearm()?;
            tracing::info!(rearmed, "reminders re-armed after a restore");

            Ok(BackupOutcome {
                completed: true,
                file_name: picked.display_name,
                note_count: contents.note_count,
                reminder_count: contents.reminder_count,
                size_bytes: contents.size_bytes,
            })
        })();

        discard(&staged);
        outcome
    }

    /// The most recent backup, so the user can see how long it has been.
    ///
    /// # Errors
    /// Fails on a database error.
    pub fn latest(&self) -> AppResult<Option<BackupRecord>> {
        self.history.latest()
    }

    /// Arms every reminder the restored database still considers due.
    ///
    /// One reminder that cannot be armed — a sound the build no longer ships,
    /// say — is logged and skipped rather than failing the restore, because by
    /// this point the user's notes are already back and refusing would leave
    /// them with no way to finish.
    fn rearm(&self) -> AppResult<usize> {
        let now = self.clock.now();
        let configured_default = self.reminders.default_sound_id()?;
        let mut armed = 0;

        for scheduled in self.reminders.active_scheduled(now)? {
            let Ok(sound) = resolve_sound(&scheduled.reminder.sound, configured_default.as_deref())
            else {
                tracing::warn!("a restored reminder names a sound this build does not have");
                continue;
            };
            match self.alarms.schedule(&alarm_from(&scheduled, sound)) {
                Ok(_) => armed += 1,
                Err(error) => tracing::warn!(%error, "a restored reminder could not be armed"),
            }
        }
        Ok(armed)
    }
}

/// Removes a staged file, saying so if it will not go.
///
/// Failing to clean up is not worth failing a backup over, but it is worth
/// knowing about: the file holds a copy of every note.
fn discard(path: &Path) {
    if path.exists() {
        if let Err(error) = std::fs::remove_file(path) {
            tracing::warn!(%error, "a staged backup file could not be removed");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use parking_lot::Mutex;

    use crate::domain::backup::BackupContents;
    use crate::domain::clock::{FixedClock, Timestamp};
    use crate::domain::ids::NoteId;
    use crate::domain::reminders::{ReminderDraft, ScheduledReminder};
    use crate::error::{AppError, BackupError};
    use crate::platform::{Alarm, AlarmPermissions, PickedDocument};

    use super::*;

    #[derive(Default)]
    struct FakeArchive {
        snapshots: Mutex<Vec<PathBuf>>,
        restores: Mutex<Vec<PathBuf>>,
        contents: Mutex<Option<BackupContents>>,
    }

    impl BackupArchive for FakeArchive {
        fn snapshot_to(&self, destination: &Path) -> AppResult<()> {
            self.snapshots.lock().push(destination.to_path_buf());
            Ok(())
        }

        fn inspect(&self, _path: &Path) -> AppResult<BackupContents> {
            self.contents
                .lock()
                .clone()
                .ok_or(AppError::Backup(BackupError::Corrupt))
        }

        fn restore_from(&self, source: &Path) -> AppResult<()> {
            self.restores.lock().push(source.to_path_buf());
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeHistory {
        records: Mutex<Vec<BackupRecord>>,
    }

    impl BackupRepository for FakeHistory {
        fn record(&self, entry: &BackupRecord) -> AppResult<()> {
            self.records.lock().push(entry.clone());
            Ok(())
        }

        fn latest(&self) -> AppResult<Option<BackupRecord>> {
            Ok(self.records.lock().last().cloned())
        }
    }

    struct FakeDocuments {
        outcome: PickedDocument,
    }

    impl DocumentStore for FakeDocuments {
        fn export(&self, _source: &str, _name: &str, _mime: &str) -> AppResult<PickedDocument> {
            Ok(self.outcome.clone())
        }

        fn import(&self, _destination: &str, _mime: &str) -> AppResult<PickedDocument> {
            Ok(self.outcome.clone())
        }
    }

    #[derive(Default)]
    struct FakeAlarms {
        scheduled: Mutex<Vec<Alarm>>,
        cancelled_all: Mutex<bool>,
    }

    impl AlarmClock for FakeAlarms {
        fn schedule(&self, alarm: &Alarm) -> AppResult<bool> {
            self.scheduled.lock().push(alarm.clone());
            Ok(true)
        }

        fn cancel(&self, _request_code: i32) -> AppResult<()> {
            Ok(())
        }

        fn cancel_all(&self) -> AppResult<()> {
            *self.cancelled_all.lock() = true;
            Ok(())
        }

        fn take_launch_target(&self) -> AppResult<Option<String>> {
            Ok(None)
        }

        fn permissions(&self) -> AppResult<AlarmPermissions> {
            Ok(AlarmPermissions {
                notifications_granted: true,
                exact_allowed: true,
            })
        }

        fn request_notification_permission(&self) -> AppResult<bool> {
            Ok(true)
        }
    }

    #[derive(Default)]
    struct FakeReminders {
        active: Mutex<Vec<ScheduledReminder>>,
    }

    impl ReminderRepository for FakeReminders {
        fn find_active_for_note(
            &self,
            _note_id: NoteId,
            _now: Timestamp,
        ) -> AppResult<Option<ScheduledReminder>> {
            Ok(None)
        }

        fn upsert_for_note(
            &self,
            _draft: ReminderDraft,
            _arm: &mut dyn FnMut(&ScheduledReminder) -> AppResult<bool>,
            _disarm: &mut dyn FnMut(i32) -> AppResult<()>,
        ) -> AppResult<ScheduledReminder> {
            unreachable!("not used by backup")
        }

        fn delete_for_note(
            &self,
            _note_id: NoteId,
            _disarm: &mut dyn FnMut(i32) -> AppResult<()>,
        ) -> AppResult<Option<ScheduledReminder>> {
            Ok(None)
        }

        fn retime(
            &self,
            _scheduled: &ScheduledReminder,
            _at: Timestamp,
            _zone: &str,
        ) -> AppResult<ScheduledReminder> {
            unreachable!("not used by backup")
        }

        fn mark_elapsed(&self, _now: Timestamp) -> AppResult<u32> {
            Ok(0)
        }

        fn thin_windows(
            &self,
            _now: Timestamp,
            _target: usize,
        ) -> AppResult<Vec<crate::domain::reminders::repository::ThinWindow>> {
            Ok(Vec::new())
        }

        fn extend_window(
            &self,
            _reminder: &crate::domain::reminders::Reminder,
            _instants: &[Timestamp],
            _arm: &mut dyn FnMut(&ScheduledReminder) -> AppResult<bool>,
        ) -> AppResult<u32> {
            Ok(0)
        }

        fn active_scheduled(&self, _now: Timestamp) -> AppResult<Vec<ScheduledReminder>> {
            Ok(self.active.lock().clone())
        }

        fn default_sound_id(&self) -> AppResult<Option<String>> {
            Ok(None)
        }

        fn time_presets(&self) -> AppResult<Option<String>> {
            Ok(None)
        }

        fn set_time_presets(&self, _raw: &str) -> AppResult<()> {
            Ok(())
        }
    }

    struct Fixture {
        use_cases: BackupUseCases,
        archive: Arc<FakeArchive>,
        history: Arc<FakeHistory>,
        alarms: Arc<FakeAlarms>,
        reminders: Arc<FakeReminders>,
        _directory: tempfile::TempDir,
    }

    fn fixture(outcome: PickedDocument, contents: Option<BackupContents>) -> Fixture {
        let directory = tempfile::tempdir().expect("temp dir");
        let archive = Arc::new(FakeArchive::default());
        *archive.contents.lock() = contents;
        let history = Arc::new(FakeHistory::default());
        let alarms = Arc::new(FakeAlarms::default());
        let reminders = Arc::new(FakeReminders::default());

        let use_cases = BackupUseCases::new(
            Arc::clone(&archive) as Arc<dyn BackupArchive>,
            Arc::clone(&history) as Arc<dyn BackupRepository>,
            Arc::clone(&reminders) as Arc<dyn ReminderRepository>,
            Arc::clone(&alarms) as Arc<dyn AlarmClock>,
            Arc::new(FakeDocuments { outcome }) as Arc<dyn DocumentStore>,
            Arc::new(FixedClock::new(Timestamp::from_millis(1_000))),
            directory.path().to_path_buf(),
        );

        Fixture {
            use_cases,
            archive,
            history,
            alarms,
            reminders,
            _directory: directory,
        }
    }

    fn contents() -> BackupContents {
        BackupContents {
            schema_version: migrations::latest_version(),
            note_count: 12,
            reminder_count: 2,
            size_bytes: 8192,
            sha256: "digest".into(),
        }
    }

    fn saved(name: &str) -> PickedDocument {
        PickedDocument {
            completed: true,
            display_name: Some(name.to_owned()),
        }
    }

    #[test]
    fn exporting_writes_a_snapshot_and_logs_it() {
        let fixture = fixture(saved("копия.sqlite"), Some(contents()));

        let outcome = fixture
            .use_cases
            .export("Asia/Almaty")
            .expect("export succeeds");

        assert!(outcome.completed);
        assert_eq!(outcome.note_count, 12);
        assert_eq!(fixture.archive.snapshots.lock().len(), 1);
        assert_eq!(fixture.history.records.lock().len(), 1);
    }

    #[test]
    fn backing_out_of_the_picker_leaves_no_trace() {
        let fixture = fixture(PickedDocument::default(), Some(contents()));

        let outcome = fixture.use_cases.export("UTC").expect("export succeeds");

        assert!(!outcome.completed);
        assert!(
            fixture.history.records.lock().is_empty(),
            "a backup nobody kept is not a backup that happened"
        );
    }

    #[test]
    fn importing_takes_the_old_alarms_back_before_restoring() {
        let fixture = fixture(saved("копия.sqlite"), Some(contents()));

        let outcome = fixture.use_cases.import().expect("import succeeds");

        assert!(outcome.completed);
        assert!(
            *fixture.alarms.cancelled_all.lock(),
            "alarms for reminders that no longer exist must not survive a restore"
        );
        assert_eq!(fixture.archive.restores.lock().len(), 1);
    }

    #[test]
    fn importing_arms_the_reminders_the_restored_database_carries() {
        let fixture = fixture(saved("копия.sqlite"), Some(contents()));
        *fixture.reminders.active.lock() = vec![scheduled_reminder()];

        fixture.use_cases.import().expect("import succeeds");

        assert_eq!(
            fixture.alarms.scheduled.lock().len(),
            1,
            "a restored reminder that is still due has to be armed again"
        );
    }

    #[test]
    fn a_backup_from_a_newer_build_never_reaches_the_database() {
        let mut newer = contents();
        newer.schema_version = migrations::latest_version() + 1;
        let fixture = fixture(saved("будущее.sqlite"), Some(newer));

        let error = fixture.use_cases.import().expect_err("must refuse");

        assert_eq!(error.code(), "backup_unsupported_version");
        assert!(
            fixture.archive.restores.lock().is_empty(),
            "the live database must be untouched when the file is refused"
        );
        assert!(!*fixture.alarms.cancelled_all.lock());
    }

    #[test]
    fn backing_out_of_the_import_picker_changes_nothing() {
        let fixture = fixture(PickedDocument::default(), Some(contents()));

        let outcome = fixture.use_cases.import().expect("import succeeds");

        assert!(!outcome.completed);
        assert!(fixture.archive.restores.lock().is_empty());
        assert!(!*fixture.alarms.cancelled_all.lock());
    }

    fn scheduled_reminder() -> ScheduledReminder {
        use crate::domain::ids::{ReminderId, ReminderOccurrenceId};
        use crate::domain::reminders::{Reminder, ReminderOccurrence};

        let reminder_id = ReminderId::new();
        ScheduledReminder {
            reminder: Reminder {
                id: reminder_id,
                note_id: NoteId::new(),
                title: "Позвонить".into(),
                body: String::new(),
                scheduled_at: Timestamp::from_millis(9_000),
                timezone: "Asia/Almaty".into(),
                sound: "default".into(),
                recurrence: None,
                snooze_minutes: 10,
                is_enabled: true,
            },
            occurrence: ReminderOccurrence {
                id: ReminderOccurrenceId::new(),
                reminder_id,
                occurrence_at: Timestamp::from_millis(9_000),
                alarm_request_code: 42,
                is_exact: true,
            },
        }
    }
}
