# Note Reminders With Sound Presets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Добавить одно активное одноразовое напоминание на заметку с индивидуальным выбором встроенного звукового пресета и общим пресетом по умолчанию.

**Architecture:** React только собирает дату, время, заголовок и ID звука. Rust валидирует запрос, владеет каталогом пресетов, хранит `reminders`/`reminder_occurrences` и оркестрирует постановку через `AlarmClock`. Android-плагин создаёт отдельный `NotificationChannel` на каждый raw-пресет и ставит `AlarmManager`.

**Tech Stack:** Tauri 2, Rust 1.97, rusqlite/SQLite, React 19, TypeScript, TanStack Query, Zod, Tailwind CSS v4, Kotlin, Android `AlarmManager`/`NotificationChannel`, Bun/Vitest, ffmpeg.

## Global Constraints

- Первая версия поддерживает только одно активное одноразовое напоминание на заметку.
- Повторения, snooze, boot restore, timezone restore, notification actions и импорт аудио с телефона исключены.
- Первый пресет: `death_and_rebirth`; исходник: `/Users/samgold/Desktop/Фото и Видео/музыка/BLESSED_MANE_-_Death_and_rebirth_(musmore.org).mp3`; фрагмент — первые 2 секунды без нормализации.
- Общий звук хранится в `app_settings['reminders.default_sound']`; при отсутствии ключа используется `death_and_rebirth`.
- Каждый пресет — отдельный Android `NotificationChannel`; ID канала: `reminders_sound_<sound_id>_v1`.
- Не добавлять новые npm/Cargo-зависимости, кроме `serde_json` как dev-dependency в in-tree плагине для проверки Rust↔Kotlin JSON-контракта.
- Не изменять и не стейджить уже локально изменённый `src/features/notes/editor/RichTextEditor.tsx` в коммиты напоминаний.
- Все сенсорные цели в UI имеют минимум 44 px; существующая тёмная визуальная система сохраняется.

---

## File Map

**Create**

- `src-tauri/src/domain/reminders/mod.rs` — модели, валидация и каталог звуков.
- `src-tauri/src/domain/reminders/repository.rs` — объектно-безопасный порт хранилища.
- `src-tauri/src/infrastructure/sqlite/reminder_repository.rs` — SQL и транзакционный upsert/delete.
- `src-tauri/plugins/reminders/android/src/main/res/raw/death_and_rebirth.mp3` — двухсекундный встроенный звук.
- `src/features/reminders/api.ts` — Zod-контракты и Tauri-вызовы.
- `src/features/reminders/api.test.ts` — проверка DTO и аргументов invoke.
- `src/features/reminders/ui/ReminderPanel.tsx` — чистая форма даты, времени, названия и звука.
- `src/features/reminders/ui/ReminderPanel.test.tsx` — поведение формы.

**Modify**

- `src-tauri/plugins/reminders/Cargo.toml`, `src-tauri/plugins/reminders/src/models.rs` — JSON-контракт звука.
- `src-tauri/plugins/reminders/android/src/main/java/dev/local/organizer/reminders/{RemindersPlugin,AlarmScheduler,ReminderIntents,ReminderNotifications,ReminderReceiver}.kt` — канал пресета и `PendingIntent`.
- `src-tauri/src/{domain/mod.rs,platform/alarms.rs,platform/tauri_alarms.rs,infrastructure/sqlite/mod.rs,error.rs,state.rs,lib.rs}` — подключение домена и платформы.
- `src-tauri/src/application/{dto.rs,use_cases.rs,commands.rs}` — use case, DTO и Tauri-команды.
- `src/pages/NoteEditorPage.tsx` — кнопка, запросы, мутации и встраивание формы.

### Task 1: Reminder Domain and Sound Catalog

**Files:**
- Create: `src-tauri/src/domain/reminders/mod.rs`
- Create: `src-tauri/src/domain/reminders/repository.rs`
- Modify: `src-tauri/src/domain/mod.rs`
- Test: `src-tauri/src/domain/reminders/mod.rs`

**Interfaces:**
- Consumes: `NoteId`, `ReminderId`, `ReminderOccurrenceId`, `Timestamp`, `AppResult`.
- Produces: `ReminderDraft`, `Reminder`, `ReminderOccurrence`, `ScheduledReminder`, `SoundPreset`, `sound_presets()`, `resolve_sound()`, `ReminderRepository`.

- [ ] **Step 1: Write failing catalog and validation tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_the_bundled_default() {
        let preset = resolve_sound("default", None).expect("default resolves");
        assert_eq!(preset.id, "death_and_rebirth");
        assert_eq!(preset.resource_name, "death_and_rebirth");
    }

    #[test]
    fn unknown_sound_is_rejected() {
        let error = resolve_sound("missing", None).expect_err("unknown sound must fail");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn stored_default_uses_the_configured_catalog_entry() {
        let preset = resolve_sound("default", Some("death_and_rebirth"))
            .expect("configured default resolves");
        assert_eq!(preset.label, "Death & Rebirth");
    }
}
```

- [ ] **Step 2: Run the focused Rust test and verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml domain::reminders -- --nocapture`

Expected: FAIL because `domain::reminders` and `resolve_sound` do not exist.

- [ ] **Step 3: Add reminder models and the fixed sound catalog**

```rust
pub mod repository;

use crate::domain::clock::Timestamp;
use crate::domain::ids::{NoteId, ReminderId, ReminderOccurrenceId};
use crate::error::{AppError, AppResult, ValidationError};

pub use repository::ReminderRepository;

pub const DEFAULT_SOUND_SETTING_KEY: &str = "reminders.default_sound";
pub const FALLBACK_SOUND_ID: &str = "death_and_rebirth";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub resource_name: &'static str,
}

pub const SOUND_PRESETS: &[SoundPreset] = &[SoundPreset {
    id: "death_and_rebirth",
    label: "Death & Rebirth",
    resource_name: "death_and_rebirth",
}];

#[must_use]
pub const fn sound_presets() -> &'static [SoundPreset] {
    SOUND_PRESETS
}

pub fn resolve_sound(selected: &str, configured_default: Option<&str>) -> AppResult<SoundPreset> {
    let concrete = if selected == "default" {
        configured_default.unwrap_or(FALLBACK_SOUND_ID)
    } else {
        selected
    };
    SOUND_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.id == concrete)
        .ok_or(AppError::Validation(ValidationError::Invalid { field: "sound" }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reminder {
    pub id: ReminderId,
    pub note_id: NoteId,
    pub title: String,
    pub body: String,
    pub scheduled_at: Timestamp,
    pub timezone: String,
    pub sound: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderOccurrence {
    pub id: ReminderOccurrenceId,
    pub reminder_id: ReminderId,
    pub occurrence_at: Timestamp,
    pub alarm_request_code: i32,
    pub is_exact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledReminder {
    pub reminder: Reminder,
    pub occurrence: ReminderOccurrence,
}

#[derive(Debug, Clone)]
pub struct ReminderDraft {
    pub note_id: NoteId,
    pub title: String,
    pub body: String,
    pub scheduled_at: Timestamp,
    pub timezone: String,
    pub sound: String,
}
```

- [ ] **Step 4: Define the repository transaction callbacks**

```rust
use crate::domain::clock::Timestamp;
use crate::domain::ids::NoteId;
use crate::error::AppResult;

use super::{ReminderDraft, ScheduledReminder};

pub trait ReminderRepository: Send + Sync {
    fn find_active_for_note(
        &self,
        note_id: NoteId,
        now: Timestamp,
    ) -> AppResult<Option<ScheduledReminder>>;

    fn upsert_for_note(
        &self,
        draft: ReminderDraft,
        schedule: &mut dyn FnMut(
            Option<&ScheduledReminder>,
            &ScheduledReminder,
        ) -> AppResult<bool>,
    ) -> AppResult<ScheduledReminder>;

    fn delete_for_note(
        &self,
        note_id: NoteId,
        cancel: &mut dyn FnMut(&ScheduledReminder) -> AppResult<()>,
    ) -> AppResult<Option<ScheduledReminder>>;

    fn default_sound_id(&self) -> AppResult<Option<String>>;
}
```

- [ ] **Step 5: Export the module and run domain tests**

```rust
// src-tauri/src/domain/mod.rs
pub mod reminders;
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml domain::reminders -- --nocapture`

Expected: PASS for all three sound-catalog tests.

- [ ] **Step 6: Commit the domain boundary**

```bash
git add src-tauri/src/domain/mod.rs src-tauri/src/domain/reminders
git commit -m "Добавить домен напоминаний и каталог звуков"
```

### Task 2: Transactional SQLite Reminder Repository

**Files:**
- Create: `src-tauri/src/infrastructure/sqlite/reminder_repository.rs`
- Modify: `src-tauri/src/infrastructure/sqlite/mod.rs`
- Test: `src-tauri/src/infrastructure/sqlite/reminder_repository.rs`

**Interfaces:**
- Consumes: `ReminderRepository`, `ReminderDraft`, `ScheduledReminder`, `Database`, `SharedClock`.
- Produces: `SqliteReminderRepository::new(database: Arc<Database>, clock: SharedClock)`.

- [ ] **Step 1: Write repository tests for create, replace, rollback and delete**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clock::{FixedClock, SharedClock, Timestamp};
    use crate::domain::notes::{NoteDraft, NoteRepository};
    use crate::error::{AppError, NotificationError};
    use crate::infrastructure::sqlite::SqliteNoteRepository;

    fn fixture() -> (SqliteReminderRepository, crate::domain::ids::NoteId) {
        let clock: SharedClock = Arc::new(FixedClock::new(Timestamp::from_millis(1_000)));
        let database = Arc::new(Database::open_in_memory(1_000).expect("opens"));
        let note = SqliteNoteRepository::new(Arc::clone(&database), Arc::clone(&clock))
            .create(NoteDraft { title: Some("Заметка".into()), ..NoteDraft::default() })
            .expect("creates note");
        (SqliteReminderRepository::new(database, clock), note.id)
    }

    fn draft(note_id: crate::domain::ids::NoteId, at: i64) -> ReminderDraft {
        ReminderDraft {
            note_id,
            title: "Проверить".into(),
            body: "Текст".into(),
            scheduled_at: Timestamp::from_millis(at),
            timezone: "Asia/Almaty".into(),
            sound: "default".into(),
        }
    }

    #[test]
    fn upsert_keeps_one_active_row_and_reuses_the_request_code() {
        let (repository, note_id) = fixture();
        let first = repository
            .upsert_for_note(draft(note_id, 2_000), &mut |_, _| Ok(true))
            .expect("creates");
        let second = repository
            .upsert_for_note(draft(note_id, 3_000), &mut |previous, _| {
                assert!(previous.is_some());
                Ok(false)
            })
            .expect("replaces");
        assert_eq!(first.occurrence.alarm_request_code, second.occurrence.alarm_request_code);
        assert!(!second.occurrence.is_exact);
    }

    #[test]
    fn a_schedule_failure_rolls_the_sql_transaction_back() {
        let (repository, note_id) = fixture();
        let result = repository.upsert_for_note(draft(note_id, 2_000), &mut |_, _| {
            Err(AppError::Notification(NotificationError::ScheduleFailed {
                reason: "test".into(),
            }))
        });
        assert!(result.is_err());
        assert!(repository
            .find_active_for_note(note_id, Timestamp::from_millis(1_000))
            .expect("reads")
            .is_none());
    }

    #[test]
    fn delete_cancels_inside_the_same_transaction() {
        let (repository, note_id) = fixture();
        let stored = repository
            .upsert_for_note(draft(note_id, 2_000), &mut |_, _| Ok(true))
            .expect("creates");
        let mut cancelled = None;
        repository
            .delete_for_note(note_id, &mut |current| {
                cancelled = Some(current.occurrence.alarm_request_code);
                Ok(())
            })
            .expect("deletes");
        assert_eq!(cancelled, Some(stored.occurrence.alarm_request_code));
    }
}
```

- [ ] **Step 2: Run the repository tests and verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml reminder_repository -- --nocapture`

Expected: FAIL because `SqliteReminderRepository` is absent.

- [ ] **Step 3: Implement row mapping and stable request-code allocation**

```rust
pub struct SqliteReminderRepository {
    database: Arc<Database>,
    clock: SharedClock,
}

impl SqliteReminderRepository {
    #[must_use]
    pub fn new(database: Arc<Database>, clock: SharedClock) -> Self {
        Self { database, clock }
    }
}

fn request_code(id: ReminderOccurrenceId) -> i32 {
    let bytes = id.as_uuid().as_bytes();
    i32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) & i32::MAX
}
```

Use a joined query over `reminders` and `reminder_occurrences`; map all ID columns with the existing rusqlite `FromSql` implementations. When a generated request code collides with `idx_reminder_occurrences_code`, generate a new `ReminderOccurrenceId` before inserting.

- [ ] **Step 4: Implement transactional upsert with the scheduling callback**

```rust
fn upsert_for_note(
    &self,
    draft: ReminderDraft,
    schedule: &mut dyn FnMut(Option<&ScheduledReminder>, &ScheduledReminder) -> AppResult<bool>,
) -> AppResult<ScheduledReminder> {
    let now = self.clock.now();
    self.database.in_transaction(|tx| {
        let previous = fetch_active(tx, draft.note_id, now)?;
        let ids = previous.as_ref().map_or_else(
            || (ReminderId::new(), ReminderOccurrenceId::new()),
            |stored| (stored.reminder.id, stored.occurrence.id),
        );
        let code = previous
            .as_ref()
            .map_or_else(|| allocate_request_code(tx, ids.1), |stored| stored.occurrence.alarm_request_code)?;
        write_reminder_and_occurrence(tx, ids, code, &draft, now)?;
        let mut next = require_scheduled(tx, ids.0)?;
        let exact = schedule(previous.as_ref(), &next)?;
        tx.execute(
            "UPDATE reminder_occurrences SET is_exact = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![exact, now.as_millis(), next.occurrence.id],
        )?;
        next.occurrence.is_exact = exact;
        Ok(next)
    })
}
```

`write_reminder_and_occurrence` updates the existing IDs when present and otherwise inserts rows with schema defaults. It always sets `recurrence_rule = NULL`, `exactness = 'exact'`, `state = 'scheduled'`, `is_enabled = 1`, `deleted_at = NULL`, and `occurrence_at = scheduled_at`.

- [ ] **Step 5: Implement delete, active lookup and default setting lookup**

```rust
fn default_sound_id(&self) -> AppResult<Option<String>> {
    self.database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [DEFAULT_SOUND_SETTING_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(AppError::from)
    })
}
```

`find_active_for_note` filters `deleted_at IS NULL`, `is_enabled = 1`, `state IN ('scheduled', 'snoozed')` and `occurrence_at > now`. `delete_for_note` loads the current record, invokes `cancel`, then sets the reminder's `deleted_at`, `is_enabled = 0` and occurrence `state = 'cancelled'` in the same SQLite transaction.

- [ ] **Step 6: Export the repository and run tests**

```rust
// src-tauri/src/infrastructure/sqlite/mod.rs
pub mod reminder_repository;
pub use reminder_repository::SqliteReminderRepository;
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml reminder_repository -- --nocapture`

Expected: PASS for create/replace, rollback and delete.

- [ ] **Step 7: Commit SQLite persistence**

```bash
git add src-tauri/src/infrastructure/sqlite/mod.rs src-tauri/src/infrastructure/sqlite/reminder_repository.rs
git commit -m "Добавить SQLite-хранилище напоминаний"
```

### Task 3: Bundled Audio and Android Sound Channels

**Files:**
- Create: `src-tauri/plugins/reminders/android/src/main/res/raw/death_and_rebirth.mp3`
- Modify: `src-tauri/plugins/reminders/Cargo.toml`
- Modify: `src-tauri/plugins/reminders/src/models.rs`
- Modify: `src-tauri/src/platform/alarms.rs`
- Modify: `src-tauri/src/platform/tauri_alarms.rs`
- Modify: `src-tauri/plugins/reminders/android/src/main/java/dev/local/organizer/reminders/RemindersPlugin.kt`
- Modify: `src-tauri/plugins/reminders/android/src/main/java/dev/local/organizer/reminders/AlarmScheduler.kt`
- Modify: `src-tauri/plugins/reminders/android/src/main/java/dev/local/organizer/reminders/ReminderIntents.kt`
- Modify: `src-tauri/plugins/reminders/android/src/main/java/dev/local/organizer/reminders/ReminderNotifications.kt`
- Modify: `src-tauri/plugins/reminders/android/src/main/java/dev/local/organizer/reminders/ReminderReceiver.kt`
- Test: `src-tauri/plugins/reminders/src/models.rs`

**Interfaces:**
- Consumes: concrete `SoundPreset` resolved by Rust.
- Produces: `ScheduleRequest { sound_id, sound_label, vibrate }`; `Alarm` with the same fields; Android channel ID carried in `PendingIntent`.

- [ ] **Step 1: Write a failing Rust serialization-contract test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_request_serializes_the_sound_contract_in_camel_case() {
        let value = serde_json::to_value(ScheduleRequest {
            occurrence_id: "occurrence".into(),
            request_code: 7,
            trigger_at_millis: 1_000,
            title: "Title".into(),
            body: "Body".into(),
            exact: true,
            sound_id: "death_and_rebirth".into(),
            sound_label: "Death & Rebirth".into(),
            vibrate: true,
        })
        .expect("serializes");
        assert_eq!(value["soundId"], "death_and_rebirth");
        assert_eq!(value["soundLabel"], "Death & Rebirth");
        assert_eq!(value["vibrate"], true);
    }
}
```

- [ ] **Step 2: Add the test dependency and verify failure**

```toml
[dev-dependencies]
serde_json = "1"
```

Run: `cargo test --manifest-path src-tauri/plugins/reminders/Cargo.toml`

Expected: FAIL because the three fields are absent.

- [ ] **Step 3: Extend the Rust platform contract**

```rust
pub struct Alarm {
    pub occurrence_id: String,
    pub request_code: i32,
    pub trigger_at: Timestamp,
    pub title: String,
    pub body: String,
    pub exact: bool,
    pub sound_id: String,
    pub sound_label: String,
    pub vibrate: bool,
}
```

Add the same three fields to plugin `ScheduleRequest`, then map them in `TauriAlarmClock::schedule` without transformations.

- [ ] **Step 4: Create and verify the two-second MP3 resource**

```bash
mkdir -p src-tauri/plugins/reminders/android/src/main/res/raw
ffmpeg -y \
  -i '/Users/samgold/Desktop/Фото и Видео/музыка/BLESSED_MANE_-_Death_and_rebirth_(musmore.org).mp3' \
  -af 'atrim=start=0:end=2,asetpts=PTS-STARTPTS' \
  -map_metadata -1 -ar 44100 -ac 2 -c:a libmp3lame -b:a 192k \
  src-tauri/plugins/reminders/android/src/main/res/raw/death_and_rebirth.mp3
ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1 \
  src-tauri/plugins/reminders/android/src/main/res/raw/death_and_rebirth.mp3
```

Expected: an MP3 around 2.0 seconds; MPEG frame padding may report up to 2.10 seconds.

- [ ] **Step 5: Extend Kotlin arguments and alarm intent extras**

```kotlin
@InvokeArg
internal class ScheduleArgs {
    var occurrenceId: String = ""
    var requestCode: Int = 0
    var triggerAtMillis: Long = 0
    var title: String = ""
    var body: String = ""
    var exact: Boolean = true
    var soundId: String = ""
    var soundLabel: String = ""
    var vibrate: Boolean = true
}
```

```kotlin
const val EXTRA_CHANNEL_ID = "channel_id"
const val EXTRA_VIBRATE = "vibrate"
```

Pass `soundId`, `soundLabel` and `vibrate` from `RemindersPlugin.schedule` into `AlarmScheduler.schedule`. Create the channel before arming; pass the returned channel ID into `pendingIntent`, so the cold-started receiver does not need a sound catalog.

- [ ] **Step 6: Replace the single channel with one immutable channel per preset**

```kotlin
internal object ReminderNotifications {
    fun ensureChannel(
        context: Context,
        soundId: String,
        soundLabel: String,
        vibrate: Boolean,
    ): String {
        require(soundId.matches(Regex("^[a-z0-9_]+$"))) { "некорректный ID звука" }
        val resourceId = context.resources.getIdentifier(soundId, "raw", context.packageName)
        require(resourceId != 0) { "звук $soundId не найден" }
        val channelId = "reminders_sound_${soundId}_v1"
        val manager = context.getSystemService(NotificationManager::class.java)
            ?: error("NotificationManager недоступен")
        if (manager.getNotificationChannel(channelId) == null) {
            val uri = Uri.parse("android.resource://${context.packageName}/$resourceId")
            val attributes = AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_ALARM)
                .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                .build()
            val channel = NotificationChannel(
                channelId,
                "Напоминания — $soundLabel",
                NotificationManager.IMPORTANCE_HIGH,
            ).apply {
                description = "Напоминания из заметок и задач"
                setSound(uri, attributes)
                enableVibration(vibrate)
                setShowBadge(true)
            }
            manager.createNotificationChannel(channel)
        }
        return channelId
    }
}
```

- [ ] **Step 7: Publish fired notifications on the carried channel**

```kotlin
val channelId = intent.getStringExtra(ReminderIntents.EXTRA_CHANNEL_ID) ?: return
val vibrate = intent.getBooleanExtra(ReminderIntents.EXTRA_VIBRATE, true)
val notification = NotificationCompat.Builder(context, channelId)
    .setSmallIcon(android.R.drawable.ic_popup_reminder)
    .setContentTitle(title.ifEmpty { "Напоминание" })
    .setContentText(body)
    .setStyle(NotificationCompat.BigTextStyle().bigText(body))
    .setPriority(NotificationCompat.PRIORITY_HIGH)
    .setCategory(NotificationCompat.CATEGORY_REMINDER)
    .setAutoCancel(true)
    .setOnlyAlertOnce(false)
    .setVibrate(if (vibrate) longArrayOf(0, 250, 150, 250) else longArrayOf(0))
    .setContentIntent(contentIntent)
    .build()
```

- [ ] **Step 8: Run plugin and host checks**

Run: `cargo test --manifest-path src-tauri/plugins/reminders/Cargo.toml`

Run: `cargo check --manifest-path src-tauri/Cargo.toml`

Expected: both PASS; the serialized request contains `soundId`, `soundLabel`, and `vibrate`.

- [ ] **Step 9: Commit platform sound support**

```bash
git add src-tauri/plugins/reminders/Cargo.toml \
  src-tauri/plugins/reminders/src/models.rs \
  src-tauri/plugins/reminders/android/src/main/res/raw/death_and_rebirth.mp3 \
  src-tauri/plugins/reminders/android/src/main/java/dev/local/organizer/reminders \
  src-tauri/src/platform/alarms.rs src-tauri/src/platform/tauri_alarms.rs
git commit -m "Добавить звуковые каналы Android для напоминаний"
```

### Task 4: Reminder Use Cases, DTOs, Commands and State Wiring

**Files:**
- Modify: `src-tauri/src/application/use_cases.rs`
- Modify: `src-tauri/src/application/dto.rs`
- Modify: `src-tauri/src/application/commands.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/error.rs`
- Test: `src-tauri/src/application/use_cases.rs`
- Test: `src-tauri/src/application/dto.rs`
- Test: `src-tauri/src/state.rs`

**Interfaces:**
- Consumes: `ReminderRepository`, `AlarmClock`, sound catalog, `SqliteReminderRepository`.
- Produces: `ReminderUseCases::{get_for_note, upsert_for_note, delete_for_note, sound_catalog}`, four Tauri commands, bridge DTOs.

- [ ] **Step 1: Write failing use-case tests with a fake alarm clock**

```rust
#[derive(Default)]
struct FakeAlarmClock {
    scheduled: parking_lot::Mutex<Vec<crate::platform::Alarm>>,
    cancelled: parking_lot::Mutex<Vec<i32>>,
    notifications_granted: bool,
    exact: bool,
}

impl AlarmClock for FakeAlarmClock {
    fn schedule(&self, alarm: &crate::platform::Alarm) -> AppResult<bool> {
        self.scheduled.lock().push(alarm.clone());
        Ok(self.exact)
    }
    fn cancel(&self, request_code: i32) -> AppResult<()> {
        self.cancelled.lock().push(request_code);
        Ok(())
    }
    fn permissions(&self) -> AppResult<AlarmPermissions> {
        Ok(AlarmPermissions {
            notifications_granted: self.notifications_granted,
            exact_allowed: self.exact,
        })
    }
    fn request_notification_permission(&self) -> AppResult<bool> {
        Ok(self.notifications_granted)
    }
}

#[test]
fn upsert_rejects_past_time_before_touching_android() {
    let fixture = reminder_fixture(FakeAlarmClock { notifications_granted: true, exact: true, ..Default::default() });
    let error = fixture.reminders.upsert_for_note(UpsertReminderRequest {
        note_id: fixture.note_id.to_string(),
        title: "Проверить".into(),
        body: String::new(),
        scheduled_at: 999,
        timezone: "Asia/Almaty".into(),
        sound: "default".into(),
    }).expect_err("past time fails");
    assert_eq!(error.code(), "validation_time_in_past");
}

#[test]
fn upsert_resolves_default_sound_and_records_inexact_fallback() {
    let fixture = reminder_fixture(FakeAlarmClock { notifications_granted: true, exact: false, ..Default::default() });
    let stored = fixture.reminders.upsert_for_note(valid_request(fixture.note_id)).expect("schedules");
    assert_eq!(stored.effective_sound.id, "death_and_rebirth");
    assert!(!stored.scheduled.occurrence.is_exact);
}

#[test]
fn denied_notifications_do_not_create_a_database_row() {
    let fixture = reminder_fixture(FakeAlarmClock::default());
    let error = fixture.reminders.upsert_for_note(valid_request(fixture.note_id)).expect_err("denied");
    assert_eq!(error.code(), "notification_permission_denied");
    assert!(fixture.reminders.get_for_note(&fixture.note_id.to_string()).expect("reads").is_none());
}
```

- [ ] **Step 2: Run focused use-case tests and verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml application::use_cases::tests -- --nocapture`

Expected: FAIL because `ReminderUseCases` and request types do not exist.

- [ ] **Step 3: Define request and response DTOs**

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertReminderRequest {
    pub note_id: String,
    pub title: String,
    pub body: String,
    pub scheduled_at: i64,
    pub timezone: String,
    pub sound: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderDto {
    pub id: String,
    pub note_id: String,
    pub occurrence_id: String,
    pub title: String,
    pub body: String,
    pub scheduled_at: i64,
    pub timezone: String,
    pub sound: String,
    pub effective_sound_id: String,
    pub effective_sound_label: String,
    pub is_exact: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderSoundDto {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderSoundCatalogDto {
    pub default_sound_id: String,
    pub items: Vec<ReminderSoundDto>,
}
```

Add a DTO test that serializes `ReminderDto` and asserts `effectiveSoundId`, `effectiveSoundLabel`, and `isExact`.

- [ ] **Step 4: Implement ReminderUseCases validation and orchestration**

```rust
pub struct ReminderView {
    pub scheduled: ScheduledReminder,
    pub effective_sound: SoundPreset,
}

impl UpsertReminderRequest {
    fn into_draft(self, note_id: NoteId) -> ReminderDraft {
        ReminderDraft {
            note_id,
            title: self.title.trim().to_owned(),
            body: self.body,
            scheduled_at: Timestamp::from_millis(self.scheduled_at),
            timezone: self.timezone,
            sound: self.sound,
        }
    }
}

fn alarm_from(scheduled: &ScheduledReminder, sound: SoundPreset) -> crate::platform::Alarm {
    crate::platform::Alarm {
        occurrence_id: scheduled.occurrence.id.to_string(),
        request_code: scheduled.occurrence.alarm_request_code,
        trigger_at: scheduled.occurrence.occurrence_at,
        title: scheduled.reminder.title.clone(),
        body: scheduled.reminder.body.clone(),
        exact: true,
        sound_id: sound.resource_name.to_owned(),
        sound_label: sound.label.to_owned(),
        vibrate: true,
    }
}

pub struct ReminderUseCases {
    reminders: Arc<dyn ReminderRepository>,
    alarms: Arc<dyn AlarmClock>,
    clock: SharedClock,
}

impl ReminderUseCases {
    #[must_use]
    pub fn new(
        reminders: Arc<dyn ReminderRepository>,
        alarms: Arc<dyn AlarmClock>,
        clock: SharedClock,
    ) -> Self {
        Self { reminders, alarms, clock }
    }

    pub fn upsert_for_note(&self, request: UpsertReminderRequest) -> AppResult<ReminderView> {
        let note_id = NoteId::parse(&request.note_id)?;
        let scheduled_at = Timestamp::from_millis(request.scheduled_at);
        if scheduled_at <= self.clock.now() {
            return Err(AppError::Validation(ValidationError::TimeInPast));
        }
        if request.title.trim().is_empty() {
            return Err(AppError::Validation(ValidationError::Required { field: "title" }));
        }
        crate::domain::notes::validate_title(request.title.trim())?;
        crate::domain::notes::validate_content(&request.body)?;
        if request.timezone.parse::<chrono_tz::Tz>().is_err() {
            return Err(AppError::Validation(ValidationError::UnknownTimeZone {
                value: request.timezone.clone(),
            }));
        }
        let configured_default = self.reminders.default_sound_id()?;
        let effective_sound = resolve_sound(&request.sound, configured_default.as_deref())?;
        let permissions = self.alarms.permissions()?;
        if !permissions.notifications_granted
            && !self.alarms.request_notification_permission()?
        {
            return Err(AppError::Notification(NotificationError::NotificationsDenied));
        }

        let alarms = Arc::clone(&self.alarms);
        let mut applied: Option<(Option<ScheduledReminder>, ScheduledReminder)> = None;
        let result = self.reminders.upsert_for_note(request.into_draft(note_id), &mut |previous, next| {
            let exact = alarms.schedule(&alarm_from(next, effective_sound))?;
            applied = Some((previous.cloned(), next.clone()));
            Ok(exact)
        });
        match result {
            Ok(scheduled) => Ok(ReminderView { scheduled, effective_sound }),
            Err(error) => {
                if let Some((previous, next)) = applied {
                    let _ = self.alarms.cancel(next.occurrence.alarm_request_code);
                    if let Some(old) = previous {
                        if let Ok(old_sound) = resolve_sound(
                            &old.reminder.sound,
                            configured_default.as_deref(),
                        ) {
                            let _ = self.alarms.schedule(&alarm_from(&old, old_sound));
                        }
                    }
                }
                Err(error)
            }
        }
    }
}
```

Implement `get_for_note` with `find_active_for_note(note_id, clock.now())`. Implement `delete_for_note` as `AppResult<Option<ReminderView>>` with a cancel callback; if SQLite commit fails after cancellation, re-arm the captured previous record using `alarm_from`. `sound_catalog` validates the configured default and falls back to `FALLBACK_SOUND_ID` only when the setting key is absent.

- [ ] **Step 5: Wire repository and platform into AppState**

```rust
pub struct AppState {
    pub notes: Arc<NoteUseCases>,
    pub reminders: Arc<ReminderUseCases>,
    pub search: Arc<SearchUseCases>,
    pub database: Arc<Database>,
    pub clock: SharedClock,
}

pub fn bootstrap(data_dir: &Path, alarms: Arc<dyn AlarmClock>) -> AppResult<Self> {
    let clock: SharedClock = Arc::new(SystemClock);
    Self::with_services(data_dir, clock, alarms)
}
```

Update state tests to use a local `FakeAlarmClock`. In `lib.rs`, construct `Arc::new(TauriAlarmClock::new(app.handle().clone()))` after the plugin is installed and pass it to `AppState::bootstrap`.

- [ ] **Step 6: Add four Tauri commands**

```rust
#[tauri::command]
pub async fn reminders_get_for_note(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<CommandResult<Option<ReminderDto>>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.get_for_note(&note_id).map(|value| value.map(ReminderDto::from))).await)
}

#[tauri::command]
pub async fn reminders_upsert_for_note(
    state: State<'_, AppState>,
    request: UpsertReminderRequest,
) -> Result<CommandResult<ReminderDto>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.upsert_for_note(request).map(ReminderDto::from)).await)
}

#[tauri::command]
pub async fn reminders_delete_for_note(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<CommandResult<()>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.delete_for_note(&note_id).map(|_| ())).await)
}

#[tauri::command]
pub async fn reminder_sounds_list(
    state: State<'_, AppState>,
) -> Result<CommandResult<ReminderSoundCatalogDto>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.sound_catalog().map(ReminderSoundCatalogDto::from)).await)
}
```

Register all four names in `tauri::generate_handler!`.

- [ ] **Step 7: Cancel reminders when a note is moved to trash**

Add an application-layer coordinator and call it from `notes_trash`:

```rust
pub fn move_note_to_trash(
    notes: &NoteUseCases,
    reminders: &ReminderUseCases,
    id: &str,
) -> AppResult<()> {
    notes.move_to_trash(id)?;
    if let Err(error) = reminders.delete_for_note(id) {
        let _ = notes.restore(id);
        return Err(error);
    }
    Ok(())
}
```

Add a Rust test that creates a note reminder, calls `move_note_to_trash`, and asserts the fake alarm received the request code in `cancelled` and the note no longer appears in the active scope.

- [ ] **Step 8: Run Rust tests and strict checks**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml`

Run: `cargo fmt --check --manifest-path src-tauri/Cargo.toml`

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

Expected: all PASS, including past-time, permissions, default sound, inexact fallback and trash cancellation.

- [ ] **Step 9: Commit application wiring**

```bash
git add src-tauri/src/application src-tauri/src/error.rs src-tauri/src/state.rs src-tauri/src/lib.rs
git commit -m "Подключить команды и сценарии напоминаний"
```

### Task 5: TypeScript Reminder Bridge

**Files:**
- Create: `src/features/reminders/api.ts`
- Create: `src/features/reminders/api.test.ts`

**Interfaces:**
- Consumes: `callCommand`, `NoteId`, `ReminderId`, `ReminderOccurrenceId`.
- Produces: `getReminderForNote`, `upsertReminderForNote`, `deleteReminderForNote`, `listReminderSounds`, schemas and inferred types.

- [ ] **Step 1: Write failing bridge-schema tests**

```ts
import { describe, expect, it } from "vitest";

import { reminderSchema, reminderSoundCatalogSchema } from "./api";

const REMINDER_ID = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e6f";
const NOTE_ID = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e70";
const OCCURRENCE_ID = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e71";

describe("reminder bridge schemas", () => {
  it("accepts the Rust reminder DTO", () => {
    expect(
      reminderSchema.parse({
        id: REMINDER_ID,
        noteId: NOTE_ID,
        occurrenceId: OCCURRENCE_ID,
        title: "Проверить",
        body: "",
        scheduledAt: 1_800_000_000_000,
        timezone: "Asia/Almaty",
        sound: "default",
        effectiveSoundId: "death_and_rebirth",
        effectiveSoundLabel: "Death & Rebirth",
        isExact: true,
      }).effectiveSoundId,
    ).toBe("death_and_rebirth");
  });

  it("rejects an empty sound catalog", () => {
    expect(() =>
      reminderSoundCatalogSchema.parse({ defaultSoundId: "death_and_rebirth", items: [] }),
    ).toThrow();
  });
});
```

- [ ] **Step 2: Run the focused Vitest file and verify failure**

Run: `bun run test -- src/features/reminders/api.test.ts`

Expected: FAIL because `src/features/reminders/api.ts` does not exist.

- [ ] **Step 3: Implement schemas and command wrappers**

```ts
import { z } from "zod";

import { callCommand } from "@/shared/api/command";
import {
  noteId,
  reminderId,
  reminderOccurrenceId,
  type NoteId,
} from "@/shared/types/ids";

const branded = <T>(parse: (value: string) => T) =>
  z.string().transform((value, context): T => {
    try {
      return parse(value);
    } catch {
      context.addIssue({ code: "custom", message: "Ядро вернуло некорректный id" });
      return z.NEVER;
    }
  });

export const reminderSoundSchema = z.object({
  id: z.string().regex(/^[a-z0-9_]+$/),
  label: z.string().min(1),
});

export const reminderSoundCatalogSchema = z.object({
  defaultSoundId: z.string().min(1),
  items: z.array(reminderSoundSchema).min(1),
});

export const reminderSchema = z.object({
  id: branded(reminderId),
  noteId: branded(noteId),
  occurrenceId: branded(reminderOccurrenceId),
  title: z.string().min(1),
  body: z.string(),
  scheduledAt: z.number().int(),
  timezone: z.string().min(1),
  sound: z.string().min(1),
  effectiveSoundId: z.string().min(1),
  effectiveSoundLabel: z.string().min(1),
  isExact: z.boolean(),
});

export type Reminder = z.infer<typeof reminderSchema>;
export type ReminderSound = z.infer<typeof reminderSoundSchema>;
export type ReminderSoundCatalog = z.infer<typeof reminderSoundCatalogSchema>;

export interface UpsertReminderRequest {
  readonly noteId: NoteId;
  readonly title: string;
  readonly body: string;
  readonly scheduledAt: number;
  readonly timezone: string;
  readonly sound: string;
}

export const getReminderForNote = (noteIdValue: NoteId) =>
  callCommand("reminders_get_for_note", reminderSchema.nullable(), { noteId: noteIdValue });

export const upsertReminderForNote = (request: UpsertReminderRequest) =>
  callCommand("reminders_upsert_for_note", reminderSchema, { request });

export const deleteReminderForNote = (noteIdValue: NoteId) =>
  callCommand("reminders_delete_for_note", z.null(), { noteId: noteIdValue });

export const listReminderSounds = () =>
  callCommand("reminder_sounds_list", reminderSoundCatalogSchema);
```

- [ ] **Step 4: Run TypeScript tests and check**

Run: `bun run test -- src/features/reminders/api.test.ts`

Run: `bun run check:ts`

Expected: PASS.

- [ ] **Step 5: Commit the TypeScript bridge**

```bash
git add src/features/reminders/api.ts src/features/reminders/api.test.ts
git commit -m "Добавить TypeScript API напоминаний"
```

### Task 6: Reminder Form UI

**Files:**
- Create: `src/features/reminders/ui/ReminderPanel.tsx`
- Create: `src/features/reminders/ui/ReminderPanel.test.tsx`

**Interfaces:**
- Consumes: `Reminder`, `ReminderSoundCatalog` from Task 5.
- Produces: `ReminderPanel`, `ReminderFormValue`, `localDateTimeToMillis`.

- [ ] **Step 1: Write failing pure-form tests**

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ReminderPanel, localDateTimeToMillis } from "./ReminderPanel";

const sounds = {
  defaultSoundId: "death_and_rebirth",
  items: [{ id: "death_and_rebirth", label: "Death & Rebirth" }],
} as const;

describe("ReminderPanel", () => {
  it("combines local date and time into milliseconds", () => {
    expect(localDateTimeToMillis("2030-01-02", "03:04")).toBe(
      new Date("2030-01-02T03:04:00").getTime(),
    );
  });

  it("submits the selected preset without closing the form", async () => {
    const onSave = vi.fn();
    render(
      <ReminderPanel
        initial={null}
        sounds={sounds}
        noteTitle="Проверить"
        busy={false}
        error={null}
        onSave={onSave}
        onDelete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    await userEvent.type(screen.getByLabelText("Дата"), "2030-01-02");
    await userEvent.type(screen.getByLabelText("Время"), "03:04");
    await userEvent.click(screen.getByRole("button", { name: "Сохранить напоминание" }));
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ sound: "default" }));
  });
});
```

- [ ] **Step 2: Run the UI test and verify failure**

Run: `bun run test -- src/features/reminders/ui/ReminderPanel.test.tsx`

Expected: FAIL because the component is absent.

- [ ] **Step 3: Implement date/time helpers and controlled form state**

```tsx
export interface ReminderFormValue {
  readonly title: string;
  readonly scheduledAt: number;
  readonly sound: string;
}

export function localDateTimeToMillis(date: string, time: string): number {
  return new Date(`${date}T${time}:00`).getTime();
}

function initialParts(initial: Reminder | null): { date: string; time: string } {
  const value = initial === null ? new Date(Date.now() + 60 * 60 * 1000) : new Date(initial.scheduledAt);
  const local = new Date(value.getTime() - value.getTimezoneOffset() * 60_000)
    .toISOString()
    .slice(0, 16);
  return { date: local.slice(0, 10), time: local.slice(11, 16) };
}
```

- [ ] **Step 4: Implement the compact panel with 44 px controls**

Render native `type="date"` and `type="time"` inputs, a title input, and a radio group. The first radio is `value="default"` with the label `По умолчанию · <actual label>`; the remaining radios use concrete catalog IDs. Use existing color tokens only: `bg-surface-sunken`, `border-border-subtle`, `text-content`, `text-content-muted`, `bg-accent`, `text-danger`. The submit handler must reject `scheduledAt <= Date.now()` locally and show `Указанное время уже прошло.` without calling `onSave`.

- [ ] **Step 5: Add edit, delete, error and inexact states**

When `initial !== null`, prefill its fields, label the submit button `Сохранить напоминание`, and show a `Удалить` button. Show the provided `error` without clearing form state. When `initial.isExact === false`, render `Android может доставить это напоминание с небольшой задержкой.`

- [ ] **Step 6: Run UI tests and TypeScript check**

Run: `bun run test -- src/features/reminders/ui/ReminderPanel.test.tsx`

Run: `bun run check:ts`

Expected: PASS.

- [ ] **Step 7: Commit the pure UI**

```bash
git add src/features/reminders/ui/ReminderPanel.tsx src/features/reminders/ui/ReminderPanel.test.tsx
git commit -m "Добавить форму напоминания в редактор"
```

### Task 7: Integrate Reminder UI Into Note Editor

**Files:**
- Modify: `src/pages/NoteEditorPage.tsx`
- Test: `src/features/reminders/ui/ReminderPanel.test.tsx`

**Interfaces:**
- Consumes: all four Task 5 bridge functions and Task 6 `ReminderPanel`.
- Produces: alarm button, React Query state, save/delete mutations, notification body synchronized with the current editor text.

- [ ] **Step 1: Extend the UI test for existing reminder prefill and deletion**

Add a test that renders `ReminderPanel` with a full `Reminder`, asserts its title and sound are selected, clicks `Удалить`, and expects `onDelete` once. Run the test and confirm it fails before adjusting the component if any required behavior is missing.

- [ ] **Step 2: Add reminder queries and mutations to Loaded**

```tsx
const reminder = useQuery({
  queryKey: ["reminder", note.id],
  queryFn: () => getReminderForNote(note.id),
});
const sounds = useQuery({ queryKey: ["reminder-sounds"], queryFn: listReminderSounds });
const saveReminder = useMutation({
  mutationFn: upsertReminderForNote,
  onSuccess: async () => {
    await client.invalidateQueries({ queryKey: ["reminder", note.id] });
  },
});
const removeReminder = useMutation({
  mutationFn: () => deleteReminderForNote(note.id),
  onSuccess: async () => {
    await client.invalidateQueries({ queryKey: ["reminder", note.id] });
  },
});
```

Move `useQueryClient()` into `Loaded`, add `showReminder`, and keep `currentContentText` alongside `title` so a newly scheduled notification uses the editor's current plain-text projection.

- [ ] **Step 3: Add the alarm button to the existing header**

```tsx
<button
  type="button"
  aria-label="Напоминание"
  aria-pressed={showReminder}
  onClick={() => setShowReminder((open) => !open)}
  className={`flex size-11 shrink-0 items-center justify-center rounded-full ${
    reminder.data === null ? "text-content" : "text-accent"
  }`}
>
  <AlarmClock className="size-5" />
</button>
```

Place it immediately before the palette button. Do not move or restyle the existing back, save-state or palette controls.

- [ ] **Step 4: Render the panel and connect save/delete**

```tsx
{showReminder && reminder.data !== undefined && sounds.data !== undefined && (
  <div className="px-4 pb-2">
    <ReminderPanel
      initial={reminder.data}
      sounds={sounds.data}
      noteTitle={title}
      busy={saveReminder.isPending || removeReminder.isPending}
      error={
        saveReminder.error !== null
          ? describeError(saveReminder.error)
          : removeReminder.error !== null
            ? describeError(removeReminder.error)
            : null
      }
      onSave={(value) => {
        saveReminder.mutate({
          noteId: note.id,
          title: value.title,
          body: currentContentText,
          scheduledAt: value.scheduledAt,
          timezone: deviceTimeZone(),
          sound: value.sound,
        });
      }}
      onDelete={() => removeReminder.mutate()}
      onClose={() => setShowReminder(false)}
    />
  </div>
)}
```

Use `describeError` directly when the mutation error is non-null; do not add a second global error helper. Show loading/error text in the same panel area for query states.

- [ ] **Step 5: Run frontend tests and build**

Run: `bun run test`

Run: `bun run check:ts`

Run: `bun run build`

Expected: all PASS. Confirm `git diff -- src/features/notes/editor/RichTextEditor.tsx` still contains only the earlier toolbar relocation.

- [ ] **Step 6: Commit editor integration without the toolbar file**

```bash
git add src/pages/NoteEditorPage.tsx src/features/reminders
git commit -m "Подключить напоминания к редактору заметки"
```

### Task 8: Full Android Verification on Pixel 8a

**Files:**
- No source changes expected; if a verification failure requires a fix, stage only the files directly involved and make a focused fix commit.

**Interfaces:**
- Consumes: completed Rust, React and Android implementation.
- Produces: installed debug APK and evidence that the custom channel fires while the UI process is stopped.

- [ ] **Step 1: Run the full local verification set**

```bash
source ./scripts/android-env.sh
bun run check:ts
bun run test
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/plugins/reminders/Cargo.toml
```

Expected: every command exits 0.

- [ ] **Step 2: Build the current debug APK**

Run: `source ./scripts/android-env.sh && bun run android:build:debug`

Expected: `src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk` exists and contains `res/raw/death_and_rebirth.mp3`.

- [ ] **Step 3: Install without deleting local data and launch**

```bash
source ./scripts/android-env.sh
adb devices -l
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
adb shell am start -n dev.local.organizer/.MainActivity
```

Expected: Pixel 8a is `device`, install reports `Success`, and the editor shows the alarm button.

- [ ] **Step 4: Verify the complete reminder scenario**

On Pixel 8a: open a note, create a reminder two minutes ahead, choose `По умолчанию · Death & Rebirth`, grant notification permission, close the app, turn the screen off, and wait. Expected: the notification appears and the bundled sound plays once for approximately two seconds. Tap the notification; expected: Organizer opens.

- [ ] **Step 5: Verify replacement, deletion and fallback state**

Create another reminder, change its time, and use `adb shell dumpsys alarm | rg dev.local.organizer` to confirm only one current `PendingIntent` for the note. Delete it and rerun the same command; expected: the corresponding alarm is absent. If the device reports `isExact = false`, confirm the UI shows the delay warning instead of failing creation.

- [ ] **Step 6: Inspect final repository state**

```bash
git status -sb
git log --oneline --decorate -10
git diff --check
git diff -- src/features/notes/editor/RichTextEditor.tsx
```

Expected: reminder commits are present; only the pre-existing toolbar relocation remains uncommitted; no whitespace errors exist.
