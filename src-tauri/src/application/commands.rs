//! Tauri commands.
//!
//! Each command does three things and nothing more: take a validated request,
//! hand it to a use case on a blocking thread, and wrap the outcome in
//! [`CommandResult`]. No SQL and no business rules live here.
//!
//! Every command is `async` so Tauri runs it off the WebView thread, and the
//! database work itself goes to a blocking pool — a slow query must never make
//! the UI drop frames.

use std::sync::Arc;

use tauri::State;

use crate::error::{AppError, AppResult, PlatformError};
use crate::state::AppState;

use super::dto::{
    AppIconCatalogDto, BackupOutcomeDto, BackupRecordDto, CommandResult, CreateNoteRequest,
    ListNotesRequest, NoteDto, NoteSummaryDto, NoteWithRemindersDto, PageDto, QuickNoteDto,
    QuickNoteSettingsDto, ReminderDto, ReminderSoundCatalogDto, ReminderSoundDto, SearchHitDto,
    SearchRequest, SnoozeSettingDto, TagDto, TaskDto, TaskProgressDto, UpdateNoteRequest,
    UpsertReminderRequest,
};
use super::use_cases::move_note_to_trash;

/// Runs blocking work on the pool and folds a panic or a join failure into a
/// normal error, so a bug in one command cannot take the whole app down.
async fn blocking<T, F>(work: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(work).await {
        Ok(result) => CommandResult::from(result),
        Err(error) => {
            tracing::error!(%error, "a command worker did not finish");
            CommandResult::failed(&AppError::Platform(PlatformError::PluginCall {
                reason: "worker join failed".to_owned(),
            }))
        }
    }
}

/// Runtime facts for the diagnostics screen.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub name: String,
    pub version: String,
    pub platform: String,
    pub schema_version: i64,
    pub note_count: u32,
}

#[tauri::command]
pub async fn app_info(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<CommandResult<AppInfoDto>, ()> {
    let package = app.package_info().clone();
    let notes = Arc::clone(&state.notes);

    Ok(blocking(move || {
        Ok(AppInfoDto {
            name: package.name.clone(),
            version: package.version.to_string(),
            platform: std::env::consts::OS.to_owned(),
            schema_version: crate::infrastructure::sqlite::migrations::latest_version(),
            note_count: notes.count(&ListNotesRequest::default())?,
        })
    })
    .await)
}

#[tauri::command]
pub async fn notes_create(
    state: State<'_, AppState>,
    request: CreateNoteRequest,
) -> Result<CommandResult<NoteDto>, ()> {
    let notes = Arc::clone(&state.notes);
    Ok(blocking(move || notes.create(request.into()).map(NoteDto::from)).await)
}

#[tauri::command]
pub async fn notes_get(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResult<Option<NoteDto>>, ()> {
    let notes = Arc::clone(&state.notes);
    Ok(blocking(move || notes.get(&id).map(|note| note.map(NoteDto::from))).await)
}

#[tauri::command]
pub async fn notes_update(
    state: State<'_, AppState>,
    id: String,
    request: UpdateNoteRequest,
) -> Result<CommandResult<NoteDto>, ()> {
    let notes = Arc::clone(&state.notes);
    Ok(blocking(move || notes.update(&id, request.into()).map(NoteDto::from)).await)
}

#[tauri::command]
pub async fn notes_list(
    state: State<'_, AppState>,
    request: ListNotesRequest,
) -> Result<CommandResult<PageDto<NoteSummaryDto>>, ()> {
    let notes = Arc::clone(&state.notes);
    let organisation = Arc::clone(&state.organisation);
    Ok(blocking(move || {
        let page = notes.list(&request)?;
        // One read for the whole page rather than one per card.
        let ids: Vec<_> = page.items.iter().map(|note| note.id).collect();
        let tags = organisation.tags_of_notes(&ids)?;
        Ok(PageDto::from_page(page, |note| {
            let names = tags
                .get(&note.id)
                .map(|tags| tags.iter().map(|tag| tag.name.clone()).collect())
                .unwrap_or_default();
            NoteSummaryDto::from(note).with_tags(names)
        }))
    })
    .await)
}

/// The notes with a reminder still to come, each carrying its own.
///
/// The screen behind this is a list of what is going to happen, so it is one
/// call rather than a list followed by a reminder read per card. The request is
/// the ordinary one with `hasReminder` set, which keeps paging, tag filtering
/// and sorting the same as everywhere else.
#[tauri::command]
pub async fn notes_list_with_reminders(
    state: State<'_, AppState>,
    request: ListNotesRequest,
) -> Result<CommandResult<PageDto<NoteWithRemindersDto>>, ()> {
    let notes = Arc::clone(&state.notes);
    let organisation = Arc::clone(&state.organisation);
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || {
        let page = notes.list(&ListNotesRequest {
            has_reminder: true,
            ..request
        })?;
        let ids: Vec<_> = page.items.iter().map(|note| note.id).collect();
        let tags = organisation.tags_of_notes(&ids)?;
        let owned: Vec<String> = ids.iter().map(ToString::to_string).collect();
        let mut due = reminders.list_for_notes(&owned)?;

        Ok(PageDto::from_page(page, move |note| {
            let names = tags
                .get(&note.id)
                .map(|tags| tags.iter().map(|tag| tag.name.clone()).collect())
                .unwrap_or_default();
            let mine = due.remove(&note.id.to_string()).unwrap_or_default();
            NoteWithRemindersDto {
                note: NoteSummaryDto::from(note).with_tags(names),
                reminders: mine.into_iter().map(ReminderDto::from).collect(),
            }
        }))
    })
    .await)
}

#[tauri::command]
pub async fn notes_trash(state: State<'_, AppState>, id: String) -> Result<CommandResult<()>, ()> {
    let notes = Arc::clone(&state.notes);
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || move_note_to_trash(&notes, &reminders, &id)).await)
}

#[tauri::command]
pub async fn notes_restore(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResult<NoteDto>, ()> {
    let notes = Arc::clone(&state.notes);
    Ok(blocking(move || notes.restore(&id).map(NoteDto::from)).await)
}

#[tauri::command]
pub async fn notes_purge(state: State<'_, AppState>, id: String) -> Result<CommandResult<()>, ()> {
    let notes = Arc::clone(&state.notes);
    Ok(blocking(move || notes.purge(&id)).await)
}

#[tauri::command]
pub async fn notes_empty_trash(state: State<'_, AppState>) -> Result<CommandResult<u32>, ()> {
    let notes = Arc::clone(&state.notes);
    Ok(blocking(move || notes.empty_trash()).await)
}

#[tauri::command]
pub async fn notes_purge_expired(state: State<'_, AppState>) -> Result<CommandResult<u32>, ()> {
    let notes = Arc::clone(&state.notes);
    Ok(blocking(move || notes.purge_expired_trash()).await)
}

#[tauri::command]
pub async fn notes_duplicate(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResult<NoteDto>, ()> {
    let notes = Arc::clone(&state.notes);
    Ok(blocking(move || notes.duplicate(&id).map(NoteDto::from)).await)
}

#[tauri::command]
pub async fn reminders_list_for_note(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<CommandResult<Vec<ReminderDto>>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || {
        reminders
            .list_for_note(&note_id)
            .map(|views| views.into_iter().map(ReminderDto::from).collect())
    })
    .await)
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
pub async fn reminders_delete(
    state: State<'_, AppState>,
    reminder_id: String,
) -> Result<CommandResult<()>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.delete(&reminder_id).map(|_| ())).await)
}

/// Note a notification tap asked to open, or nothing if the app was opened the
/// usual way. Answering clears it, so asking again returns nothing.
#[tauri::command]
pub async fn reminders_take_launch_target(
    state: State<'_, AppState>,
) -> Result<CommandResult<Option<String>>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.take_launch_target()).await)
}

/// Re-renders pending reminders in the zone the device is in now.
///
/// Asked on every start: nothing notifies an app that it has changed country,
/// and a reminder that means 09:00 has to keep meaning 09:00.
#[tauri::command]
pub async fn reminders_reconcile_zone(
    state: State<'_, AppState>,
    timezone: String,
) -> Result<CommandResult<u32>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.reconcile_zone(&timezone)).await)
}

/// Arms repeating reminders further ahead.
///
/// Asked on every start: nothing wakes the core when an alarm fires, so a
/// repeat lives on the firings armed in advance of it.
#[tauri::command]
pub async fn reminders_top_up(state: State<'_, AppState>) -> Result<CommandResult<u32>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.top_up_windows()).await)
}

#[tauri::command]
pub async fn reminder_sounds_list(
    state: State<'_, AppState>,
) -> Result<CommandResult<ReminderSoundCatalogDto>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.sound_catalog().map(ReminderSoundCatalogDto::from)).await)
}

/// Opens the system file picker so the user can import a sound of their own.
/// `None` means the user backed out, which is not an error.
#[tauri::command]
pub async fn reminder_sound_pick_custom(
    state: State<'_, AppState>,
) -> Result<CommandResult<Option<ReminderSoundDto>>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || {
        reminders
            .pick_custom_sound()
            .map(|sound| sound.map(ReminderSoundDto::custom))
    })
    .await)
}

#[tauri::command]
pub async fn reminder_sound_delete_custom(
    state: State<'_, AppState>,
    sound_id: String,
) -> Result<CommandResult<()>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.delete_custom_sound(&sound_id)).await)
}

#[tauri::command]
pub async fn reminder_sound_preview(
    state: State<'_, AppState>,
    sound_id: String,
) -> Result<CommandResult<()>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.preview_sound(&sound_id)).await)
}

#[tauri::command]
pub async fn reminder_sound_preview_stop(
    state: State<'_, AppState>,
) -> Result<CommandResult<()>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.stop_preview()).await)
}

/// Writes a snapshot and asks the user where to keep it.
///
/// The picker blocks until the user decides, which is why this — like every
/// other command here — runs on a blocking task rather than on the thread
/// serving the WebView.
#[tauri::command]
pub async fn backup_export(
    state: State<'_, AppState>,
    timezone: String,
) -> Result<CommandResult<BackupOutcomeDto>, ()> {
    let backup = Arc::clone(&state.backup);
    Ok(blocking(move || backup.export(&timezone).map(BackupOutcomeDto::from)).await)
}

#[tauri::command]
pub async fn backup_import(
    state: State<'_, AppState>,
) -> Result<CommandResult<BackupOutcomeDto>, ()> {
    let backup = Arc::clone(&state.backup);
    Ok(blocking(move || backup.import().map(BackupOutcomeDto::from)).await)
}

#[tauri::command]
pub async fn backup_latest(
    state: State<'_, AppState>,
) -> Result<CommandResult<Option<BackupRecordDto>>, ()> {
    let backup = Arc::clone(&state.backup);
    Ok(blocking(move || {
        backup
            .latest()
            .map(|record| record.map(BackupRecordDto::from))
    })
    .await)
}

#[tauri::command]
pub async fn tasks_list_for_note(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<CommandResult<Vec<TaskDto>>, ()> {
    let tasks = Arc::clone(&state.tasks);
    Ok(blocking(move || {
        tasks
            .list_for_note(&note_id)
            .map(|items| items.into_iter().map(TaskDto::from).collect())
    })
    .await)
}

#[tauri::command]
pub async fn tasks_create_for_note(
    state: State<'_, AppState>,
    note_id: String,
    title: String,
) -> Result<CommandResult<TaskDto>, ()> {
    let tasks = Arc::clone(&state.tasks);
    Ok(blocking(move || tasks.create_for_note(&note_id, &title).map(TaskDto::from)).await)
}

#[tauri::command]
pub async fn tasks_set_completed(
    state: State<'_, AppState>,
    id: String,
    completed: bool,
) -> Result<CommandResult<TaskDto>, ()> {
    let tasks = Arc::clone(&state.tasks);
    Ok(blocking(move || tasks.set_completed(&id, completed).map(TaskDto::from)).await)
}

#[tauri::command]
pub async fn tasks_delete(state: State<'_, AppState>, id: String) -> Result<CommandResult<()>, ()> {
    let tasks = Arc::clone(&state.tasks);
    Ok(blocking(move || tasks.delete(&id)).await)
}

/// Empties a note's checklist in one call, answering how many rows went.
#[tauri::command]
pub async fn tasks_clear_for_note(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<CommandResult<u32>, ()> {
    let tasks = Arc::clone(&state.tasks);
    Ok(blocking(move || tasks.clear_for_note(&note_id)).await)
}

#[tauri::command]
pub async fn tasks_progress_for_note(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<CommandResult<TaskProgressDto>, ()> {
    let tasks = Arc::clone(&state.tasks);
    Ok(blocking(move || tasks.progress_for_note(&note_id).map(TaskProgressDto::from)).await)
}

#[tauri::command]
pub async fn tags_list(state: State<'_, AppState>) -> Result<CommandResult<Vec<TagDto>>, ()> {
    let organisation = Arc::clone(&state.organisation);
    Ok(blocking(move || {
        organisation
            .tags()
            .map(|tags| tags.into_iter().map(TagDto::from).collect())
    })
    .await)
}

#[tauri::command]
pub async fn tags_ensure(
    state: State<'_, AppState>,
    name: String,
) -> Result<CommandResult<TagDto>, ()> {
    let organisation = Arc::clone(&state.organisation);
    Ok(blocking(move || organisation.ensure_tag(&name).map(TagDto::from)).await)
}

#[tauri::command]
pub async fn tags_delete(state: State<'_, AppState>, id: String) -> Result<CommandResult<()>, ()> {
    let organisation = Arc::clone(&state.organisation);
    Ok(blocking(move || organisation.delete_tag(&id)).await)
}

#[tauri::command]
pub async fn tags_of_note(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<CommandResult<Vec<TagDto>>, ()> {
    let organisation = Arc::clone(&state.organisation);
    Ok(blocking(move || {
        organisation
            .tags_of_note(&note_id)
            .map(|tags| tags.into_iter().map(TagDto::from).collect())
    })
    .await)
}

#[tauri::command]
pub async fn tags_set_for_note(
    state: State<'_, AppState>,
    note_id: String,
    tags: Vec<String>,
) -> Result<CommandResult<Vec<TagDto>>, ()> {
    let organisation = Arc::clone(&state.organisation);
    Ok(blocking(move || {
        organisation
            .set_note_tags(&note_id, &tags)
            .map(|tags| tags.into_iter().map(TagDto::from).collect())
    })
    .await)
}

#[tauri::command]
pub async fn app_icons_list(
    state: State<'_, AppState>,
) -> Result<CommandResult<AppIconCatalogDto>, ()> {
    let icons = Arc::clone(&state.app_icons);
    Ok(blocking(move || icons.catalog().map(AppIconCatalogDto::from)).await)
}

#[tauri::command]
pub async fn app_icons_select(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResult<AppIconCatalogDto>, ()> {
    let icons = Arc::clone(&state.app_icons);
    Ok(blocking(move || icons.select(&id).map(AppIconCatalogDto::from)).await)
}

#[tauri::command]
pub async fn reminder_time_presets_list(
    state: State<'_, AppState>,
) -> Result<CommandResult<Vec<String>>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.time_presets()).await)
}

#[tauri::command]
pub async fn reminder_time_presets_save(
    state: State<'_, AppState>,
    presets: Vec<String>,
) -> Result<CommandResult<Vec<String>>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || reminders.save_time_presets(&presets)).await)
}

/// How long the notification's "later" button moves a reminder by, and the
/// amounts the picker offers.
#[tauri::command]
pub async fn reminder_snooze_get(
    state: State<'_, AppState>,
) -> Result<CommandResult<SnoozeSettingDto>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || {
        Ok(SnoozeSettingDto {
            minutes: reminders.snooze_minutes()?,
            offered: reminders.offered_snoozes()?,
        })
    })
    .await)
}

#[tauri::command]
pub async fn reminder_snooze_save(
    state: State<'_, AppState>,
    minutes: i64,
) -> Result<CommandResult<SnoozeSettingDto>, ()> {
    let reminders = Arc::clone(&state.reminders);
    Ok(blocking(move || {
        let minutes = reminders.save_snooze_minutes(minutes)?;
        Ok(SnoozeSettingDto {
            minutes,
            offered: reminders.offered_snoozes()?,
        })
    })
    .await)
}

#[tauri::command]
pub async fn quick_notes_settings(
    state: State<'_, AppState>,
) -> Result<CommandResult<QuickNoteSettingsDto>, ()> {
    let quick_notes = Arc::clone(&state.quick_notes);
    Ok(blocking(move || quick_notes.settings().map(QuickNoteSettingsDto::from)).await)
}

#[tauri::command]
pub async fn quick_notes_settings_save(
    state: State<'_, AppState>,
    lead_minutes: u16,
    fallback_time: String,
    fallback_day_offset: u16,
) -> Result<CommandResult<QuickNoteSettingsDto>, ()> {
    let quick_notes = Arc::clone(&state.quick_notes);
    Ok(blocking(move || {
        quick_notes
            .save_settings(lead_minutes, &fallback_time, fallback_day_offset)
            .map(QuickNoteSettingsDto::from)
    })
    .await)
}

/// Turns one dictated sentence into a note, and into a reminder when it can.
///
/// The recogniser runs in the WebView's own screen and hands the finished text
/// here; the core does not listen to a microphone and never sees audio.
#[tauri::command]
pub async fn quick_notes_create(
    state: State<'_, AppState>,
    transcript: String,
    timezone: String,
) -> Result<CommandResult<QuickNoteDto>, ()> {
    let quick_notes = Arc::clone(&state.quick_notes);
    Ok(blocking(move || {
        quick_notes
            .create(&transcript, &timezone)
            .map(QuickNoteDto::from)
    })
    .await)
}

#[tauri::command]
pub async fn search_run(
    state: State<'_, AppState>,
    request: SearchRequest,
) -> Result<CommandResult<PageDto<SearchHitDto>>, ()> {
    let search = Arc::clone(&state.search);
    Ok(blocking(move || {
        search
            .search(&request)
            .map(|page| PageDto::from_page(page, SearchHitDto::from))
    })
    .await)
}

#[tauri::command]
pub async fn search_recent(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<CommandResult<Vec<String>>, ()> {
    let search = Arc::clone(&state.search);
    Ok(blocking(move || search.recent_queries(limit)).await)
}

#[tauri::command]
pub async fn search_clear_history(state: State<'_, AppState>) -> Result<CommandResult<()>, ()> {
    let search = Arc::clone(&state.search);
    Ok(blocking(move || search.clear_history()).await)
}
