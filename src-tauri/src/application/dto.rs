//! Wire types.
//!
//! Domain structs never cross the bridge directly: a DTO is a deliberate,
//! stable contract that can stay put while the domain model moves underneath.
//! Field names are camelCase to match the TypeScript side without a translation
//! layer on the frontend.

use serde::{Deserialize, Serialize};

use crate::domain::notes::{
    Note, NoteDraft, NoteFilter, NotePatch, NoteScope, NoteSort, NoteType, Page, PageRequest,
};
use crate::domain::search::{SearchEntity, SearchHit, SearchQuery};
use crate::error::{AppError, AppErrorDto};

use super::use_cases::{ReminderSoundCatalog, ReminderView};

/// Uniform envelope for every command.
///
/// A failure is an ordinary value here rather than a rejected promise carrying
/// a stringified Rust error, which is what lets the frontend handle domain
/// failures and bridge failures differently.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult<T> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<AppErrorDto>,
}

impl<T> CommandResult<T> {
    #[must_use]
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    #[must_use]
    pub fn failed(error: &AppError) -> Self {
        // The full error, including any driver detail, stays in the log; only
        // the sanitised DTO travels to the UI.
        tracing::warn!(code = error.code(), "command failed");
        Self {
            success: false,
            data: None,
            error: Some(error.to_dto()),
        }
    }
}

impl<T> From<Result<T, AppError>> for CommandResult<T> {
    fn from(result: Result<T, AppError>) -> Self {
        match result {
            Ok(value) => Self::ok(value),
            Err(error) => Self::failed(&error),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageDto<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

impl<T> PageDto<T> {
    pub fn from_page<S, F>(page: Page<S>, convert: F) -> Self
    where
        F: Fn(S) -> T,
    {
        let has_more = page.has_more();
        Self {
            items: page.items.into_iter().map(convert).collect(),
            total: page.total,
            limit: page.limit,
            offset: page.offset,
            has_more,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub id: String,
    pub note_type: NoteType,
    pub title: String,
    pub content_text: String,
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
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

impl From<Note> for NoteDto {
    fn from(note: Note) -> Self {
        Self {
            id: note.id.to_string(),
            note_type: note.note_type,
            title: note.title,
            content_text: note.content_text,
            content_json: note.content_json,
            color: note.color,
            background: note.background,
            is_pinned: note.is_pinned,
            is_favorite: note.is_favorite,
            is_archived: note.is_archived,
            is_readonly: note.is_readonly,
            position: note.position,
            word_count: note.word_count,
            char_count: note.char_count,
            revision: note.revision,
            created_at: note.created_at.as_millis(),
            updated_at: note.updated_at.as_millis(),
            deleted_at: note
                .deleted_at
                .map(crate::domain::clock::Timestamp::as_millis),
        }
    }
}

/// Summary shape for list rows. Sending the full body for every row would push
/// megabytes through JSON to render a list that shows two lines per note.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummaryDto {
    pub id: String,
    pub note_type: NoteType,
    pub title: String,
    pub preview: String,
    pub color: Option<String>,
    pub is_pinned: bool,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub word_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

/// How much body text a list row gets. Enough for two lines on a Pixel 8a.
const PREVIEW_CHARS: usize = 160;

impl From<Note> for NoteSummaryDto {
    fn from(note: Note) -> Self {
        let preview: String = note
            .content_text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<&str>>()
            .join(" ")
            .chars()
            .take(PREVIEW_CHARS)
            .collect();

        Self {
            id: note.id.to_string(),
            note_type: note.note_type,
            title: note.title,
            preview,
            color: note.color,
            is_pinned: note.is_pinned,
            is_favorite: note.is_favorite,
            is_archived: note.is_archived,
            word_count: note.word_count,
            created_at: note.created_at.as_millis(),
            updated_at: note.updated_at.as_millis(),
            deleted_at: note
                .deleted_at
                .map(crate::domain::clock::Timestamp::as_millis),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub entity: SearchEntity,
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub rank: f64,
}

impl From<SearchHit> for SearchHitDto {
    fn from(hit: SearchHit) -> Self {
        Self {
            entity: hit.entity,
            id: hit.id,
            title: hit.title,
            snippet: hit.snippet,
            rank: hit.rank,
        }
    }
}

// ------------------------------------------------------------- requests --

/// Deserialises `Option<Option<T>>` so that an absent key and an explicit
/// `null` stay distinguishable.
///
/// Serde collapses both to `None` by default, which would make it impossible to
/// tell "do not touch the colour" from "remove the colour".
///
/// # Errors
/// Propagates the inner deserialiser's error.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteRequest {
    pub note_type: Option<NoteType>,
    pub title: Option<String>,
    pub content_text: Option<String>,
    pub content_json: Option<String>,
    pub color: Option<String>,
    pub background: Option<String>,
}

impl From<CreateNoteRequest> for NoteDraft {
    fn from(request: CreateNoteRequest) -> Self {
        Self {
            note_type: request.note_type,
            title: request.title,
            content_text: request.content_text,
            content_json: request.content_json,
            color: request.color,
            background: request.background,
        }
    }
}

/// Partial update.
///
/// `Option<Option<T>>` distinguishes "not mentioned" from "set to null", which
/// is what lets the editor autosave the body without wiping the note colour.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNoteRequest {
    pub note_type: Option<NoteType>,
    pub title: Option<String>,
    pub content_text: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub content_json: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub color: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub background: Option<Option<String>>,
    pub is_pinned: Option<bool>,
    pub is_favorite: Option<bool>,
    pub is_archived: Option<bool>,
    pub is_readonly: Option<bool>,
    pub position: Option<i64>,
}

impl From<UpdateNoteRequest> for NotePatch {
    fn from(request: UpdateNoteRequest) -> Self {
        Self {
            note_type: request.note_type,
            title: request.title,
            content_text: request.content_text,
            content_json: request.content_json,
            color: request.color,
            background: request.background,
            is_pinned: request.is_pinned,
            is_favorite: request.is_favorite,
            is_archived: request.is_archived,
            is_readonly: request.is_readonly,
            position: request.position,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNotesRequest {
    #[serde(default)]
    pub scope: NoteScope,
    #[serde(default)]
    pub sort: NoteSort,
    pub folder_id: Option<String>,
    pub tag_id: Option<String>,
    pub note_type: Option<NoteType>,
    #[serde(default)]
    pub pinned_only: bool,
    pub updated_after: Option<i64>,
    pub updated_before: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl ListNotesRequest {
    /// Converts to a domain filter, parsing any identifiers.
    ///
    /// # Errors
    /// Fails when an identifier is not a UUID.
    pub fn to_filter(&self) -> Result<NoteFilter, AppError> {
        Ok(NoteFilter {
            scope: self.scope,
            folder_id: self
                .folder_id
                .as_deref()
                .map(crate::domain::ids::FolderId::parse)
                .transpose()?,
            tag_id: self
                .tag_id
                .as_deref()
                .map(crate::domain::ids::TagId::parse)
                .transpose()?,
            note_type: self.note_type,
            pinned_only: self.pinned_only,
            updated_after: self
                .updated_after
                .map(crate::domain::clock::Timestamp::from_millis),
            updated_before: self
                .updated_before
                .map(crate::domain::clock::Timestamp::from_millis),
        })
    }

    #[must_use]
    pub fn to_page(&self) -> PageRequest {
        PageRequest::new(self.limit.unwrap_or(50), self.offset.unwrap_or(0))
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub text: String,
    #[serde(default)]
    pub entities: Vec<SearchEntity>,
    pub folder_id: Option<String>,
    pub tag_id: Option<String>,
    pub note_type: Option<NoteType>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
    #[serde(default)]
    pub has_reminder: bool,
    #[serde(default)]
    pub has_attachment: bool,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default)]
    pub include_trashed: bool,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl SearchRequest {
    /// Converts to a domain query, parsing any identifiers.
    ///
    /// # Errors
    /// Fails when an identifier is not a UUID.
    pub fn to_query(&self) -> Result<SearchQuery, AppError> {
        Ok(SearchQuery {
            text: self.text.clone(),
            entities: self.entities.clone(),
            folder_id: self
                .folder_id
                .as_deref()
                .map(crate::domain::ids::FolderId::parse)
                .transpose()?,
            tag_id: self
                .tag_id
                .as_deref()
                .map(crate::domain::ids::TagId::parse)
                .transpose()?,
            note_type: self.note_type,
            created_after: self
                .created_after
                .map(crate::domain::clock::Timestamp::from_millis),
            created_before: self
                .created_before
                .map(crate::domain::clock::Timestamp::from_millis),
            has_reminder: self.has_reminder,
            has_attachment: self.has_attachment,
            include_archived: self.include_archived,
            include_trashed: self.include_trashed,
        })
    }

    #[must_use]
    pub fn to_page(&self) -> PageRequest {
        PageRequest::new(self.limit.unwrap_or(30), self.offset.unwrap_or(0))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertReminderRequest {
    pub note_id: String,
    pub title: String,
    pub body: String,
    pub scheduled_at: i64,
    pub timezone: String,
    pub sound: String,
    /// RFC 5545 rule, or absent for a reminder that happens once.
    #[serde(default)]
    pub recurrence: Option<String>,
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
    /// RFC 5545 rule, or absent when the reminder happens once.
    pub recurrence: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderSoundDto {
    pub id: String,
    pub label: String,
}

/// What a backup or restore did, as the settings screen reports it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOutcomeDto {
    /// False when the user backed out of the picker, which the screen shows as
    /// nothing having happened rather than as a failure.
    pub completed: bool,
    pub file_name: Option<String>,
    pub note_count: i64,
    pub reminder_count: i64,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecordDto {
    pub file_name: String,
    pub size_bytes: u64,
    pub note_count: i64,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderSoundCatalogDto {
    pub default_sound_id: String,
    pub items: Vec<ReminderSoundDto>,
}

impl From<ReminderView> for ReminderDto {
    fn from(view: ReminderView) -> Self {
        let scheduled = view.scheduled;
        Self {
            id: scheduled.reminder.id.to_string(),
            note_id: scheduled.reminder.note_id.to_string(),
            occurrence_id: scheduled.occurrence.id.to_string(),
            title: scheduled.reminder.title,
            body: scheduled.reminder.body,
            scheduled_at: scheduled.reminder.scheduled_at.as_millis(),
            timezone: scheduled.reminder.timezone,
            sound: scheduled.reminder.sound,
            effective_sound_id: view.effective_sound.id.to_owned(),
            effective_sound_label: view.effective_sound.label.to_owned(),
            is_exact: scheduled.occurrence.is_exact,
            recurrence: scheduled
                .reminder
                .recurrence
                .map(|rule| rule.rule().to_owned()),
        }
    }
}

impl From<crate::application::backup::BackupOutcome> for BackupOutcomeDto {
    fn from(outcome: crate::application::backup::BackupOutcome) -> Self {
        Self {
            completed: outcome.completed,
            file_name: outcome.file_name,
            note_count: outcome.note_count,
            reminder_count: outcome.reminder_count,
            size_bytes: outcome.size_bytes,
        }
    }
}

impl From<crate::domain::backup::BackupRecord> for BackupRecordDto {
    fn from(record: crate::domain::backup::BackupRecord) -> Self {
        Self {
            file_name: record.file_name,
            size_bytes: record.size_bytes,
            note_count: record.note_count,
            created_at: record.created_at.as_millis(),
        }
    }
}

impl From<ReminderSoundCatalog> for ReminderSoundCatalogDto {
    fn from(catalog: ReminderSoundCatalog) -> Self {
        Self {
            default_sound_id: catalog.default_sound_id,
            items: catalog
                .items
                .into_iter()
                .map(|preset| ReminderSoundDto {
                    id: preset.id.to_owned(),
                    label: preset.label.to_owned(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_successful_result_carries_data_and_no_error() {
        let json = serde_json::to_value(CommandResult::ok(42_i32)).expect("serialises");
        assert_eq!(json["success"], serde_json::json!(true));
        assert_eq!(json["data"], serde_json::json!(42));
        assert!(json.get("error").is_none());
    }

    #[test]
    fn a_failed_result_carries_the_dto_and_no_data() {
        let error = AppError::Notification(crate::error::NotificationError::ExactAlarmDenied);
        let json = serde_json::to_value(CommandResult::<i32>::failed(&error)).expect("serialises");
        assert_eq!(json["success"], serde_json::json!(false));
        assert!(json.get("data").is_none());
        assert_eq!(
            json["error"]["code"],
            serde_json::json!("exact_alarm_permission_denied")
        );
        assert_eq!(json["error"]["kind"], serde_json::json!("notification"));
    }

    #[test]
    fn a_list_preview_collapses_blank_lines() {
        let note = Note {
            id: crate::domain::ids::NoteId::new(),
            note_type: NoteType::Text,
            title: "Заметка".to_owned(),
            content_text: "первая\n\n\n   вторая  ".to_owned(),
            content_json: None,
            color: None,
            background: None,
            is_pinned: false,
            is_favorite: false,
            is_archived: false,
            is_readonly: false,
            position: 0,
            word_count: 2,
            char_count: 20,
            revision: 1,
            created_at: crate::domain::clock::Timestamp::from_millis(0),
            updated_at: crate::domain::clock::Timestamp::from_millis(0),
            deleted_at: None,
        };
        let summary = NoteSummaryDto::from(note);
        assert_eq!(summary.preview, "первая вторая");
    }

    #[test]
    fn a_list_preview_is_bounded() {
        let note = Note {
            id: crate::domain::ids::NoteId::new(),
            note_type: NoteType::Text,
            title: String::new(),
            content_text: "я".repeat(1000),
            content_json: None,
            color: None,
            background: None,
            is_pinned: false,
            is_favorite: false,
            is_archived: false,
            is_readonly: false,
            position: 0,
            word_count: 1,
            char_count: 1000,
            revision: 1,
            created_at: crate::domain::clock::Timestamp::from_millis(0),
            updated_at: crate::domain::clock::Timestamp::from_millis(0),
            deleted_at: None,
        };
        let summary = NoteSummaryDto::from(note);
        assert_eq!(
            summary.preview.chars().count(),
            PREVIEW_CHARS,
            "a huge note must not be shipped whole to render one row"
        );
    }

    #[test]
    fn a_list_request_defaults_to_the_active_scope() {
        let request: ListNotesRequest = serde_json::from_str("{}").expect("deserialises");
        assert_eq!(request.scope, NoteScope::Active);
        assert_eq!(request.to_page().limit, 50);
    }

    #[test]
    fn a_bad_identifier_in_a_request_is_rejected() {
        let request = ListNotesRequest {
            folder_id: Some("not-a-uuid".to_owned()),
            ..ListNotesRequest::default()
        };
        let error = request.to_filter().expect_err("must not reach SQL");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn an_update_distinguishes_absent_from_null() {
        let absent: UpdateNoteRequest =
            serde_json::from_str(r#"{"title":"x"}"#).expect("deserialises");
        assert_eq!(absent.color, None, "an unmentioned field is left alone");

        let cleared: UpdateNoteRequest =
            serde_json::from_str(r#"{"color":null}"#).expect("deserialises");
        assert_eq!(
            cleared.color,
            Some(None),
            "an explicit null clears the field"
        );

        let set: UpdateNoteRequest =
            serde_json::from_str(r#"{"color":"red"}"#).expect("deserialises");
        assert_eq!(set.color, Some(Some("red".to_owned())));
    }

    #[test]
    fn a_reminder_dto_exposes_the_effective_sound_and_accuracy() {
        let json = serde_json::to_value(ReminderDto {
            id: "reminder".to_owned(),
            note_id: "note".to_owned(),
            occurrence_id: "occurrence".to_owned(),
            title: "Проверить".to_owned(),
            body: String::new(),
            scheduled_at: 1_800_000_000_000,
            timezone: "Asia/Almaty".to_owned(),
            sound: "default".to_owned(),
            effective_sound_id: "death_and_rebirth".to_owned(),
            effective_sound_label: "Death & Rebirth".to_owned(),
            is_exact: false,
            recurrence: None,
        })
        .expect("serialises");

        assert_eq!(json["effectiveSoundId"], "death_and_rebirth");
        assert_eq!(json["effectiveSoundLabel"], "Death & Rebirth");
        assert_eq!(json["isExact"], false);
    }

    #[test]
    fn a_reminder_view_converts_to_the_wire_contract() {
        use crate::application::use_cases::ReminderView;
        use crate::domain::clock::Timestamp;
        use crate::domain::ids::{NoteId, ReminderId, ReminderOccurrenceId};
        use crate::domain::reminders::{
            Reminder, ReminderOccurrence, ScheduledReminder, SOUND_PRESETS,
        };

        let note_id = NoteId::new();
        let reminder_id = ReminderId::new();
        let occurrence_id = ReminderOccurrenceId::new();
        let dto = ReminderDto::from(ReminderView {
            scheduled: ScheduledReminder {
                reminder: Reminder {
                    id: reminder_id,
                    note_id,
                    title: "Проверить".into(),
                    body: "Текст".into(),
                    scheduled_at: Timestamp::from_millis(2_000),
                    timezone: "Asia/Almaty".into(),
                    sound: "default".into(),
                    recurrence: None,
                    is_enabled: true,
                },
                occurrence: ReminderOccurrence {
                    id: occurrence_id,
                    reminder_id,
                    occurrence_at: Timestamp::from_millis(2_000),
                    alarm_request_code: 7,
                    is_exact: true,
                },
            },
            effective_sound: SOUND_PRESETS[0],
        });

        assert_eq!(dto.note_id, note_id.to_string());
        assert_eq!(dto.occurrence_id, occurrence_id.to_string());
        assert_eq!(dto.effective_sound_id, "death_and_rebirth");
    }

    #[test]
    fn a_sound_catalog_converts_to_the_wire_contract() {
        use crate::application::use_cases::ReminderSoundCatalog;
        use crate::domain::reminders::SOUND_PRESETS;

        let dto = ReminderSoundCatalogDto::from(ReminderSoundCatalog {
            default_sound_id: "death_and_rebirth".into(),
            items: SOUND_PRESETS.to_vec(),
        });

        assert_eq!(dto.default_sound_id, "death_and_rebirth");
        assert_eq!(dto.items.len(), 1);
        assert_eq!(dto.items[0].id, "death_and_rebirth");
    }
}
