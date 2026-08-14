//! Notes dictated in one breath.
//!
//! A quick note is the shortest path the app has: press, say "встреча, 15:00",
//! and end up with a note called «Встреча» and an alarm before it. Everything
//! that decides what those words meant lives here — the recogniser only hands
//! over text, and the screen only draws the result.
//!
//! Three rules shape the module.
//!
//! The reminder is placed *before* the time that was said, because someone
//! naming the hour of a meeting is naming the meeting and not the moment they
//! want to be told about it; how far before is theirs to set.
//!
//! Nothing is taken off a time the app itself chose. «Завтра утром» and «купить
//! молоко» land on an hour nobody named, and being warned half an hour early
//! for a guess means nothing; a timer — «через 20 минут» — is likewise the
//! moment the person asked for, not an event to be early for.
//!
//! And a phrase with no time in it is still a quick note. It lands at the
//! fallback hour rather than being refused, so dictating an errand works the
//! same way as dictating an appointment.

pub mod numbers;
pub mod phrase;
pub mod settings;

pub use phrase::{next_time_of_day, parse_phrase, time_of_day_in_days, ParsedPhrase};
pub use settings::{QuickNoteSettings, QUICK_NOTES_SETTING_KEY};
