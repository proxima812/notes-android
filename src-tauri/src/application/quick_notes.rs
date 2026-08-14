//! Dictated notes.
//!
//! One press, one sentence, one note — and, when the sentence named a time, one
//! alarm before it. The parsing lives in the domain and the recording lives in
//! Kotlin; what is left here is the order the two are put together in, and the
//! one decision neither can make: what to do when the note is created and the
//! alarm cannot be.

use std::sync::Arc;

use crate::domain::clock::{SharedClock, Timestamp};
use crate::domain::notes::{Note, NoteDraft, NoteType};
use crate::domain::quick_notes::{
    parse_phrase, settings as quick_settings, time_of_day_in_days, QuickNoteSettings,
    QUICK_NOTES_SETTING_KEY,
};
use crate::domain::reminders::{parse_zone, time_presets::TimePreset};
use crate::domain::settings::SettingsRepository;
use crate::error::{AppError, AppResult, ValidationError};

use super::dto::UpsertReminderRequest;
use super::use_cases::{NoteUseCases, ReminderUseCases, ReminderView};

/// How close to now a reminder may be pushed when the lead would otherwise put
/// it in the past.
///
/// Saying «встреча в 15:00» at 14:45 with a half-hour lead asks for 14:15,
/// which has gone. Refusing would be pedantic — the meeting is real and still
/// ahead — so the reminder lands as soon as the alarm layer will accept it.
const MINIMUM_NOTICE_MINUTES: i64 = 1;

/// What a dictation turned into.
pub struct QuickNoteOutcome {
    pub note: Note,
    /// The reminder that was armed, or `None` when arming it failed.
    pub reminder: Option<ReminderView>,
    /// Why no reminder was armed. The note is kept either way: the words were
    /// said, and losing them because a notification permission is off would be
    /// the worse failure.
    pub reminder_error: Option<AppError>,
    /// The instant the phrase itself named, before anything was subtracted.
    ///
    /// The screen needs both this and the reminder to say something true:
    /// «встреча в 15:00 — напомню в 14:30» reads as a correct parse, while the
    /// alarm time alone reads as a mishearing of the time that was said.
    pub spoken_at: Option<Timestamp>,
    /// Minutes actually taken off the spoken time, which is the setting for a
    /// named hour and nothing at all for a day, a part of the day or a timer.
    pub lead_minutes: u16,
    /// What the recogniser heard, echoed back so the screen can show the person
    /// what the app thinks they said.
    pub transcript: String,
}

/// A reminder that was placed, and the two facts about how it was placed.
struct ArmedReminder {
    view: ReminderView,
    spoken_at: Timestamp,
    lead_minutes: u16,
}

pub struct QuickNoteUseCases {
    notes: Arc<NoteUseCases>,
    reminders: Arc<ReminderUseCases>,
    settings: Arc<dyn SettingsRepository>,
    clock: SharedClock,
}

impl QuickNoteUseCases {
    #[must_use]
    pub fn new(
        notes: Arc<NoteUseCases>,
        reminders: Arc<ReminderUseCases>,
        settings: Arc<dyn SettingsRepository>,
        clock: SharedClock,
    ) -> Self {
        Self {
            notes,
            reminders,
            settings,
            clock,
        }
    }

    /// # Errors
    /// Fails when the stored settings are not the form this build writes, or on
    /// a database error.
    pub fn settings(&self) -> AppResult<QuickNoteSettings> {
        let stored = self.settings.read(QUICK_NOTES_SETTING_KEY)?;
        quick_settings::parse_stored(stored.as_deref())
    }

    /// Replaces all three settings at once.
    ///
    /// # Errors
    /// Fails when the lead is longer than a day, when the time is not a real
    /// `HH:MM`, when the day is further off than a month, or on a database
    /// error.
    pub fn save_settings(
        &self,
        lead_minutes: u16,
        fallback_time: &str,
        fallback_day_offset: u16,
    ) -> AppResult<QuickNoteSettings> {
        let settings = QuickNoteSettings::new(
            lead_minutes,
            TimePreset::parse(fallback_time)?,
            fallback_day_offset,
        )?;
        self.settings.write(
            QUICK_NOTES_SETTING_KEY,
            &quick_settings::serialise(settings)?,
        )?;
        Ok(settings)
    }

    /// Turns a dictated sentence into a note, and into a reminder when it can.
    ///
    /// The note is written first and on purpose. Arming an alarm can fail for
    /// reasons that have nothing to do with what was said — notifications
    /// switched off, a time that has already gone — and in every one of them
    /// the person still dictated something they wanted kept.
    ///
    /// # Errors
    /// Fails when the transcript holds no words, when the zone is unknown, or
    /// on a database error. A reminder that could not be armed is reported in
    /// the outcome rather than as an error.
    pub fn create(&self, transcript: &str, timezone: &str) -> AppResult<QuickNoteOutcome> {
        let zone = parse_zone(timezone)?;
        let now = self.clock.now();
        let settings = self.settings()?;
        let phrase = parse_phrase(transcript, now, zone, settings.fallback_time())?;

        let note = self.notes.create(NoteDraft {
            // A dictated note is plain text until someone opens it and makes it
            // something else; claiming `voice_note` would promise an audio file
            // that is not kept.
            note_type: Some(NoteType::Text),
            title: Some(phrase.title.clone()),
            // The transcript is the body only when it says more than the title
            // already does, so a two-word note does not open on its own echo.
            content_text: Some(body_for(transcript, &phrase.title)),
            ..NoteDraft::default()
        })?;

        let armed = self.arm(&note, &phrase, settings, now, timezone);
        let (reminder, reminder_error, spoken_at, lead_minutes) = match armed {
            Ok(armed) => (
                Some(armed.view),
                None,
                Some(armed.spoken_at),
                armed.lead_minutes,
            ),
            Err(error) => {
                tracing::info!(
                    code = error.code(),
                    "a dictated note kept its text but not its alarm"
                );
                (None, Some(error), phrase.named_at, 0)
            }
        };

        Ok(QuickNoteOutcome {
            note,
            reminder,
            reminder_error,
            spoken_at,
            lead_minutes,
            transcript: transcript.trim().to_owned(),
        })
    }

    /// Places the reminder the phrase asked for.
    fn arm(
        &self,
        note: &Note,
        phrase: &crate::domain::quick_notes::ParsedPhrase,
        settings: QuickNoteSettings,
        now: Timestamp,
        timezone: &str,
    ) -> AppResult<ArmedReminder> {
        let zone = parse_zone(timezone)?;
        // A phrase with no time in it still gets a reminder: the fallback hour
        // is what the setting is for. Nothing is subtracted from it, because
        // there is no named event to be early for.
        let (named_at, lead_applies) = match phrase.named_at {
            Some(at) => (at, phrase.named_clock_time),
            None => (
                time_of_day_in_days(
                    settings.fallback_time(),
                    now,
                    zone,
                    settings.fallback_day_offset(),
                )?,
                false,
            ),
        };

        let lead = if lead_applies {
            settings.lead_minutes()
        } else {
            0
        };
        let mut at = named_at.saturating_add_minutes(-i64::from(lead));

        if at <= now {
            // A clock time that has gone is the person's own words — «сегодня в
            // 9:00» said in the afternoon. Only they can tell whether they
            // meant tomorrow, so it is reported rather than fixed.
            //
            // An hour the *app* chose is a different matter. «сегодня забрать
            // посылку» said at eight in the evening lands on the shipped 19:00
            // and would be refused for a guess nobody made, so it rings as soon
            // as the alarm layer will take it instead.
            if named_at <= now && phrase.named_clock_time {
                return Err(AppError::Validation(ValidationError::TimeInPast));
            }
            at = now.saturating_add_minutes(MINIMUM_NOTICE_MINUTES);
        }

        let sound = self.reminders.sound_catalog()?.default_sound_id;

        let view = self.reminders.upsert_for_note(UpsertReminderRequest {
            reminder_id: None,
            note_id: note.id.to_string(),
            title: phrase.title.clone(),
            body: String::new(),
            scheduled_at: at.as_millis(),
            timezone: timezone.to_owned(),
            sound,
            recurrence: None,
        })?;

        Ok(ArmedReminder {
            view,
            spoken_at: named_at,
            // Reported as the lead that was actually taken off, not the one in
            // the settings: the screen says «напомню в 14:30» only when there
            // is a 15:00 to be early for.
            lead_minutes: lead,
        })
    }
}

/// The transcript, unless the title already is the transcript.
///
/// The comparison folds case through `to_lowercase` rather than
/// `eq_ignore_ascii_case`: the title is capitalised on the way out, and the
/// letter that gets capitalised is Cyrillic more often than not.
fn body_for(transcript: &str, title: &str) -> String {
    let transcript = transcript.trim();
    if transcript.to_lowercase() == title.to_lowercase() {
        String::new()
    } else {
        transcript.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clock::{Clock as _, FixedClock};

    #[test]
    fn a_title_that_is_the_whole_transcript_leaves_the_body_empty() {
        assert_eq!(body_for("купить молоко", "Купить молоко"), "");
    }

    #[test]
    fn a_transcript_that_says_more_than_the_title_is_kept_in_the_body() {
        assert_eq!(
            body_for("встреча. 15:00", "Встреча"),
            "встреча. 15:00",
            "the time that was cut from the title is still what was said"
        );
    }

    /// The lead is the whole point of the feature, so the arithmetic is pinned
    /// here as well as in the parser: 15:00 with a half-hour lead is 14:30.
    #[test]
    fn the_lead_is_subtracted_from_the_time_that_was_said() {
        let zone = chrono_tz::Europe::Moscow;
        let now = FixedClock::at_local(zone, 2026, 8, 10, 12, 0).now();
        let settings = QuickNoteSettings::default();
        let phrase =
            parse_phrase("встреча. 15:00", now, zone, settings.fallback_time()).expect("parses");

        let named = phrase.named_at.expect("a time was named");
        let at = named.saturating_add_minutes(-i64::from(settings.lead_minutes()));

        assert_eq!(
            at.to_zoned(zone)
                .expect("representable")
                .format("%H:%M")
                .to_string(),
            "14:30"
        );
    }
}
