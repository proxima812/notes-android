//! Turning a dictated sentence into a title and an instant.
//!
//! The recogniser hands over one line of text — «встреча. 15:00», "meeting
//! tomorrow at 9", «купить молоко» — and this module answers two questions
//! about it: what should the note be called, and when was it for. Nothing else.
//! Creating the note, applying the lead and arming the alarm belong to the use
//! case above; keeping them out is what lets every rule below be tested against
//! a frozen clock.
//!
//! **What is understood.** A clock time in digits (`15:00`, `15.30`, «в 15»,
//! «в 15 30», «в 15 часов 30 минут», «в 9 утра», "at 9 pm") or in words («в три
//! часа», «полвторого», «в половине третьего», «без пятнадцати шесть»,
//! «полдень», "half past three", "quarter to six"); a relative day (сегодня /
//! завтра / послезавтра, today / tomorrow); a weekday («в понедельник», "on
//! monday"); a part of the day with no hour in it («завтра утром» → 09:00); and
//! a distance from now («через 20 минут», «через час», "in 2 hours").
//!
//! **Digits and words are placed differently, on purpose.** `15:00` says which
//! hour of twenty-four it is and is taken at its word. «В три» does not — a
//! clock face has no afternoon on it — so a spoken hour is read as the daytime
//! one and, if that has gone, as its twin twelve hours away: «в девять» said at
//! noon is nine tonight, «в три» said at four is three tomorrow afternoon.
//!
//! **What is not.** Calendar dates («12 марта») and repeats («каждый день»).
//! They stay in the title rather than being guessed at: a phrase that quietly
//! became the wrong instant is worse than one that became a note with no
//! reminder, because only the second one is visible on the screen the person is
//! already looking at. The same rule governs numbers that are not times at all
//! — «на 20 минут», «в 2 раза больше», «встреча 12.03» — which keep their
//! numbers and get no alarm.
//!
//! **Which languages.** The word lists are Russian and English. The digit forms
//! carry the other six interface languages: `15:00` means the same thing in all
//! of them, and a word this module does not know is left in the title, where it
//! reads as part of what was said.

use chrono::{Datelike as _, Duration, NaiveDate, NaiveTime, TimeZone as _, Weekday};
use chrono_tz::Tz;

use crate::domain::clock::Timestamp;
use crate::domain::quick_notes::numbers;
use crate::domain::reminders::time_presets::TimePreset;
use crate::error::{AppError, AppResult, ValidationError};

/// Longest title a dictated phrase produces.
///
/// Anything past this is a paragraph rather than a name, and the whole
/// transcript is kept in the body regardless, so nothing said is lost by
/// cutting here.
pub const MAX_SPOKEN_TITLE_LEN: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPhrase {
    /// What the note is called: the phrase with the time words taken out.
    pub title: String,
    /// The instant the phrase pointed at, or `None` when it named no time.
    pub named_at: Option<Timestamp>,
    /// True when a clock time or a distance from now was said, false when only
    /// a day was — the difference decides whether the lead applies, since being
    /// warned half an hour before «завтра» means nothing.
    pub named_clock_time: bool,
}

/// Reads a dictated phrase against a given moment and zone.
///
/// `fallback_time` is the hour a phrase that named a day but no time lands on;
/// a phrase that named neither comes back with `named_at: None` and lets the
/// caller decide, because "no time was said" and "the time said was today at
/// seven" are different facts.
///
/// # Errors
/// Returns [`ValidationError::Required`] for a transcript with no words in it,
/// and propagates a zone conversion failure.
pub fn parse_phrase(
    transcript: &str,
    now: Timestamp,
    zone: Tz,
    fallback_time: TimePreset,
) -> AppResult<ParsedPhrase> {
    let tokens = tokenise(transcript);
    if tokens.is_empty() {
        return Err(AppError::Validation(ValidationError::Required {
            field: "transcript",
        }));
    }

    let found = scan(&tokens);
    let title = title_from(&tokens, &found.consumed, transcript);
    let (named_at, named_clock_time) = resolve(&found, now, zone, fallback_time)?;

    Ok(ParsedPhrase {
        title,
        named_at,
        named_clock_time,
    })
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

/// One word of the phrase, kept twice: as it was said, for the title, and
/// folded to lowercase, for matching.
struct Token<'a> {
    text: &'a str,
    lower: String,
    /// The value when the whole token is a run of digits, which is what
    /// separates «15» from «15:00» and from «встреча».
    number: Option<u32>,
}

/// Splits on everything that is not part of a word.
///
/// `:`, `.` and `-` are kept inside a token because they hold times together
/// (`15:00`, `15.30`), and trimmed off the ends because there they are only
/// punctuation — «встреча.» is the same word as «встреча». Apostrophes are kept
/// for the opposite reason: they are never punctuation between words, and
/// splitting on one turns "mum's" into two tokens that the title then rebuilds
/// as «mum s».
fn tokenise(text: &str) -> Vec<Token<'_>> {
    text.split(|character: char| {
        !character.is_alphanumeric() && !matches!(character, ':' | '.' | '-' | '\'' | '\u{2019}')
    })
    .map(|word| word.trim_matches(|character| matches!(character, ':' | '.' | '-')))
    .filter(|word| !word.is_empty())
    .map(|word| Token {
        text: word,
        lower: word.to_lowercase(),
        number: (word.len() <= 4 && word.chars().all(|c| c.is_ascii_digit()))
            .then(|| word.parse().ok())
            .flatten(),
    })
    .collect()
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Which day the phrase pointed at, before it is turned into a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DayHint {
    /// Days from today: сегодня, завтра, послезавтра.
    Offset(i64),
    /// The next day that falls on this weekday.
    Named(Weekday),
}

#[derive(Default)]
struct Found {
    consumed: Vec<bool>,
    day: Option<DayHint>,
    clock: Option<Clock>,
    /// The hour a part of the day stands for, when no clock time was said:
    /// «завтра утром». Weaker than `clock` and only used in its absence.
    part_of_day: Option<(u32, u32)>,
    /// Minutes from now, for «через …».
    distance: Option<i64>,
}

/// Walks the phrase once, left to right, taking the first match of each kind.
///
/// First rather than best: a second time in one phrase («встреча в 15:00,
/// перезвонить в 18:00») is one the parser has no way to choose between, and
/// leaving it in the title shows the person both, which is the honest outcome.
fn scan(tokens: &[Token<'_>]) -> Found {
    let mut found = Found {
        consumed: vec![false; tokens.len()],
        ..Found::default()
    };

    let mut index = 0;
    while index < tokens.len() {
        if found.consumed[index] {
            index += 1;
            continue;
        }

        // «через …» is tried first because it opens with a preposition and
        // holds a number that the clock rules would otherwise take for an hour.
        if found.distance.is_none() {
            if let Some((length, minutes)) = match_distance(tokens, index) {
                found.distance = Some(minutes);
                consume(&mut found.consumed, index, length);
                index += length;
                continue;
            }
        }

        if found.clock.is_none() {
            if let Some(hit) = match_clock(tokens, index, &found.consumed) {
                found.clock = Some(hit.clock);
                consume(&mut found.consumed, hit.start, hit.end - hit.start);
                index = hit.end;
                continue;
            }
        }

        if found.day.is_none() {
            if let Some((start, length, hint)) = match_day(tokens, index, &found.consumed) {
                found.day = Some(hint);
                consume(&mut found.consumed, start, length);
                index = start + length;
                continue;
            }
        }

        // Last, because «утра» in «в 9 утра» belongs to the hour in front of it
        // and only means "morning" on its own.
        if found.part_of_day.is_none() {
            if let Some((start, length, at)) = match_part_of_day(tokens, index, &found.consumed) {
                found.part_of_day = Some(at);
                consume(&mut found.consumed, start, length);
                index = start + length;
                continue;
            }
        }

        index += 1;
    }

    found
}

fn consume(consumed: &mut [bool], start: usize, length: usize) {
    for flag in consumed.iter_mut().skip(start).take(length) {
        *flag = true;
    }
}

/// Words that introduce a time and belong to it rather than to the title:
/// «встреча в 15:00» is a note called «Встреча», not «Встреча в».
fn is_time_preposition(word: &str) -> bool {
    matches!(word, "в" | "во" | "к" | "ко" | "на" | "at" | "on" | "by")
}

/// The narrower set that can turn a lone number into an hour.
///
/// «на» is missing on purpose, and so are "on" and "by": «на 20 минут»,
/// «на 5 число», "on 3 counts" all put a number behind a preposition without
/// naming any time at all, and «на» in particular is the commonest way in
/// Russian to say how many of something there are. A written `15:00` still
/// counts behind any of them — it cannot be mistaken for a count.
fn vouches_bare_hour(word: &str) -> bool {
    matches!(word, "в" | "во" | "к" | "ко" | "at")
}

/// Nouns that count things rather than mark time.
///
/// A number in front of one of these is an amount — «в 2 раза больше», «на 5
/// число» — and reading it as an hour costs the phrase both its number and its
/// reminder.
fn is_counted_noun(word: &str) -> bool {
    matches!(
        word,
        "раз"
            | "раза"
            | "разa"
            | "число"
            | "числа"
            | "штук"
            | "штуки"
            | "штуку"
            | "процентов"
            | "процента"
            | "рублей"
            | "человек"
            | "билета"
            | "билетов"
            | "times"
            | "pieces"
            | "people"
    )
}

fn unit_minutes(word: &str) -> Option<i64> {
    let minutes = match word {
        "мин" | "минуту" | "минуты" | "минут" | "минуток" | "min" | "minute" | "minutes" => {
            1
        }
        "час" | "часа" | "часов" | "hour" | "hours" => 60,
        "день" | "дня" | "дней" | "day" | "days" => 24 * 60,
        "неделю" | "недели" | "недель" | "week" | "weeks" => 7 * 24 * 60,
        _ => return None,
    };
    Some(minutes)
}

/// True for the units that make a number a length of time rather than an hour.
///
/// «на 20 минут опоздаю» says how long, not when, and without this the twenty
/// would be read as eight in the evening — the preposition in front of it looks
/// exactly like the one in «в 20».
fn is_duration_unit(word: &str) -> bool {
    matches!(unit_minutes(word), Some(minutes) if minutes != 60)
}

/// A number, whether it arrived as digits or as words.
///
/// Returns how many tokens it took, because «двадцать три» is one number in two
/// words and everything downstream counts in tokens.
fn number_at(tokens: &[Token<'_>], index: usize) -> Option<(u32, usize)> {
    let token = tokens.get(index)?;

    if let Some(value) = token.number {
        return Some((value, 1));
    }

    // "twenty-three" arrives as one token, because the tokeniser keeps hyphens
    // inside words — a time can be written `15-30`.
    if let Some((tens, unit)) = token.lower.split_once('-') {
        if let (Some(tens), Some(unit)) = (numbers::cardinal(tens), numbers::cardinal(unit)) {
            if numbers::is_tens(tens) && (1..10).contains(&unit) {
                return Some((tens + unit, 1));
            }
        }
    }

    let value = numbers::cardinal(&token.lower)?;

    // «двадцать три», "forty five" — a round ten can carry a unit behind it.
    if numbers::is_tens(value) {
        if let Some(unit) = tokens
            .get(index + 1)
            .and_then(|next| numbers::cardinal(&next.lower))
            .filter(|unit| (1..10).contains(unit))
        {
            return Some((value + unit, 2));
        }
    }

    Some((value, 1))
}

/// «через 20 минут», «через час», «через полчаса», «через две недели»,
/// "in 2 hours", "in an hour".
fn match_distance(tokens: &[Token<'_>], index: usize) -> Option<(usize, i64)> {
    if !matches!(tokens[index].lower.as_str(), "через" | "in") {
        return None;
    }
    let next = tokens.get(index + 1)?;

    if next.lower == "полчаса" {
        return Some((2, 30));
    }
    // A unit with no count in front of it means one of them.
    if let Some(minutes) = unit_minutes(&next.lower) {
        return Some((2, minutes));
    }

    // "in an hour" — the article carries no count of its own.
    let (count, length) = match number_at(tokens, index + 1) {
        Some((count, length)) => (i64::from(count), length),
        None if matches!(next.lower.as_str(), "a" | "an") => (1, 1),
        None => return None,
    };
    let minutes = unit_minutes(&tokens.get(index + 1 + length)?.lower)?;
    Some((2 + length, count.saturating_mul(minutes)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Clock {
    hour: u32,
    minute: u32,
    /// True when the hour was said as a word with no part of day beside it.
    ///
    /// Nobody says «в пятнадцать» out loud — they say «в три», and a clock face
    /// has no afternoon on it. A time that arrived as digits carries its own
    /// answer and is left alone; one that arrived as a word has to be placed on
    /// the right half of the day, which is what this flag lets [`resolve`] do.
    twelve_hour: bool,
}

struct ClockHit {
    clock: Clock,
    /// Token range the time occupies, preposition and meridiem included.
    start: usize,
    end: usize,
}

/// A clock time, in every shape a person says one.
///
/// The shapes are tried from the most specific to the least, because they
/// overlap: «полвторого» starts with a word the half rule owns, «без
/// пятнадцати шесть» starts with a number that the plain-hour rule would take
/// for the hour itself, and only what is left over is an ordinary «в 15:00».
fn match_clock(tokens: &[Token<'_>], index: usize, consumed: &[bool]) -> Option<ClockHit> {
    let preceded_by_preposition =
        index > 0 && !consumed[index - 1] && is_time_preposition(&tokens[index - 1].lower);
    let start = if preceded_by_preposition {
        index - 1
    } else {
        index
    };

    // «полдень», «полночь» — the two hours that say which half of the day they
    // are on.
    if let Some((hour, minute)) = numbers::named_hour(&tokens[index].lower) {
        return Some(ClockHit {
            clock: Clock {
                hour,
                minute,
                twelve_hour: false,
            },
            start,
            end: index + 1,
        });
    }

    if let Some(hit) = match_half(tokens, index, start) {
        return Some(hit);
    }

    if let Some(hit) = match_minutes_around(tokens, index, start) {
        return Some(hit);
    }

    match_plain_clock(tokens, index, start, preceded_by_preposition)
}

/// «полвторого», «пол второго», «в половине третьего», "half past three".
///
/// Russian counts the hour it is halfway *into*, so «полвторого» is 01:30 and
/// not 02:30 — the commonest way to be an hour wrong about a dictated time, and
/// the reason this has its own rule rather than a subtraction somewhere.
fn match_half(tokens: &[Token<'_>], index: usize, start: usize) -> Option<ClockHit> {
    let word = tokens[index].lower.as_str();

    // "half past three" — English counts the hour it has left.
    if word == "half" {
        let past = tokens.get(index + 1)?;
        if past.lower != "past" {
            return None;
        }
        let (hour, length) = number_at(tokens, index + 2)?;
        if hour > 23 {
            return None;
        }
        return Some(ClockHit {
            clock: Clock {
                hour,
                minute: 30,
                twelve_hour: true,
            },
            start,
            end: index + 2 + length,
        });
    }

    let (ordinal, end) = match numbers::half_of(word) {
        Some(ordinal) => (ordinal, index + 1),
        None if matches!(word, "пол" | "половина" | "половине" | "половины") => {
            (numbers::ordinal(&tokens.get(index + 1)?.lower)?, index + 2)
        }
        None => return None,
    };

    Some(ClockHit {
        clock: Clock {
            // Half of the second hour is half past one; half of the first is
            // half past twelve.
            hour: ordinal - 1,
            minute: 30,
            twelve_hour: true,
        },
        start,
        end,
    })
}

/// «без пятнадцати шесть», «без четверти шесть», "quarter to six",
/// "ten past three".
fn match_minutes_around(tokens: &[Token<'_>], index: usize, start: usize) -> Option<ClockHit> {
    let word = tokens[index].lower.as_str();

    // «без N H» — N minutes short of hour H.
    if word == "без" {
        let (minutes, minutes_length) = number_at(tokens, index + 1)?;
        if !(1..60).contains(&minutes) {
            return None;
        }
        let (hour, hour_length) = number_at(tokens, index + 1 + minutes_length)?;
        if hour > 23 {
            return None;
        }
        return Some(ClockHit {
            clock: Clock {
                // Twelve hours rather than twenty-four: nobody says «без пяти
                // двадцать четыре», so the hour before one is midnight.
                hour: if hour == 0 { 23 } else { hour - 1 },
                minute: 60 - minutes,
                twelve_hour: spoken_in_words(tokens, index + 1 + minutes_length),
            },
            start,
            end: index + 1 + minutes_length + hour_length,
        });
    }

    // "quarter to six", "ten past three".
    let (minutes, minutes_length) = number_at(tokens, index)?;
    if !(1..60).contains(&minutes) {
        return None;
    }
    let direction = tokens.get(index + minutes_length)?.lower.as_str();
    let (hour, hour_length) = number_at(tokens, index + minutes_length + 1)?;
    if hour > 23 {
        return None;
    }
    let end = index + minutes_length + 1 + hour_length;

    let twelve_hour = spoken_in_words(tokens, index + minutes_length + 1);
    match direction {
        "to" => Some(ClockHit {
            clock: Clock {
                hour: if hour == 0 { 23 } else { hour - 1 },
                minute: 60 - minutes,
                twelve_hour,
            },
            start,
            end,
        }),
        "past" => Some(ClockHit {
            clock: Clock {
                hour,
                minute: minutes,
                twelve_hour,
            },
            start,
            end,
        }),
        _ => None,
    }
}

/// `15:00`, «в 15», «в 15 30», «в три часа дня», "at 9 pm".
fn match_plain_clock(
    tokens: &[Token<'_>],
    index: usize,
    start: usize,
    preceded_by_preposition: bool,
) -> Option<ClockHit> {
    let token = &tokens[index];
    let vouching_preposition =
        index > 0 && vouches_bare_hour(&tokens[index - 1].lower) && preceded_by_preposition;

    // A time written with digits says which hour of twenty-four it is; one said
    // in words does not, and the difference decides how it is placed later.
    let mut twelve_hour = spoken_in_words(tokens, index);

    let (hour, minute, mut end) = if let Some((hour, minute)) = split_written_time(&token.lower) {
        // `15:00` and `15.30`. The colon spelling is a time wherever it stands;
        // the dot needs the same vouching as a bare pair, because «встреча
        // 12.03» is a date and «хлеб 2.50» is a price.
        twelve_hour = false;
        if !token.lower.contains(':') && !(vouching_preposition || index + 1 == tokens.len()) {
            return None;
        }
        (hour, minute, index + 1)
    } else {
        let (hour, hour_length) = number_at(tokens, index)?;
        let after_hour = index + hour_length;

        // «на 20 минут» is a length of time and «в 2 раза» is a multiplier.
        // Neither is an hour, and reading one as an hour costs the phrase both
        // its number and its reminder. «дня» is exempt because it is a unit
        // *and* a part of the day: «в 3 дня» is three in the afternoon, while
        // «на 3 дня» never reaches here — «на» does not vouch for a bare hour.
        if tokens.get(after_hour).is_some_and(|next| {
            (is_duration_unit(&next.lower) && meridiem_shift(&next.lower).is_none())
                || is_counted_noun(&next.lower)
        }) {
            return None;
        }

        let paired = number_at(tokens, after_hour).filter(|&(minute, _)| {
            minute <= 59
                // A written minute keeps its two digits — `15 30` is a time and
                // `15 3` is not — while a spoken one has no digits to count.
                && tokens[after_hour]
                    .number
                    .is_none_or(|_| tokens[after_hour].text.len() == 2)
        });

        match paired {
            // «в 15 30» — a bare pair only reads as a time behind a preposition
            // or at the very end of the phrase, because «купить 15 30» is a
            // shopping list and not an appointment.
            Some((minute, minute_length))
                if (vouching_preposition || after_hour + minute_length == tokens.len())
                    && hour <= 23 =>
            {
                (hour, minute, after_hour + minute_length)
            }
            _ => {
                // A lone number is only an hour when something says so: the
                // preposition in front of it, or the part of day behind it.
                let vouched = vouching_preposition
                    || tokens
                        .get(after_hour)
                        .is_some_and(|next| meridiem_shift(&next.lower).is_some())
                    || tokens.get(after_hour).is_some_and(|next| {
                        matches!(next.lower.as_str(), "час" | "часа" | "часов")
                            && tokens
                                .get(after_hour + 1)
                                .is_some_and(|after| meridiem_shift(&after.lower).is_some())
                    });
                if !vouched || hour > 23 {
                    return None;
                }
                (hour, 0, after_hour)
            }
        }
    };

    if hour > 23 || minute > 59 {
        return None;
    }

    // «в 15 часов» — the unit adds nothing to the time and everything to the
    // title if left behind.
    let mut minute = minute;
    if tokens
        .get(end)
        .is_some_and(|next| is_hour_unit(&next.lower))
    {
        end += 1;

        // «в 15 часов 30 минут» — the half of the time that the unit word
        // separates from its hour. Without this the minutes are both lost from
        // the time and left in the title.
        if minute == 0 {
            if let Some((spoken, length)) = number_at(tokens, end).filter(|&(value, _)| value <= 59)
            {
                let unit_at = end + length;
                if tokens
                    .get(unit_at)
                    .is_some_and(|next| is_minute_unit(&next.lower))
                {
                    minute = spoken;
                    end = unit_at + 1;
                }
            }
        }
    }

    let hour = match tokens.get(end).and_then(|next| meridiem_shift(&next.lower)) {
        Some(shift) => {
            end += 1;
            // «в три часа дня» said which half of the day it meant, so there is
            // nothing left to work out.
            twelve_hour = false;
            shift(hour)
        }
        None => hour,
    };

    Some(ClockHit {
        clock: Clock {
            hour,
            minute,
            twelve_hour: twelve_hour && hour <= 12,
        },
        start,
        end,
    })
}

/// «час», «часа», «часов», "hour", "o'clock" — the word that names the unit
/// rather than the time.
fn is_hour_unit(word: &str) -> bool {
    matches!(
        word,
        "час" | "часа" | "часов" | "hour" | "hours" | "o'clock" | "o\u{2019}clock" | "oclock"
    )
}

fn is_minute_unit(word: &str) -> bool {
    matches!(
        word,
        "мин" | "минут" | "минуты" | "минуту" | "min" | "minute" | "minutes"
    )
}

/// True when the number at this position was said rather than written.
fn spoken_in_words(tokens: &[Token<'_>], index: usize) -> bool {
    tokens
        .get(index)
        .is_some_and(|token| token.number.is_none() && !token.text.chars().any(|c| c.is_numeric()))
}

/// `HH:MM` and the one other form a recogniser writes instead of it.
///
/// The dash spelling is gone: Google writes «15:30» or «15 30» and never
/// «15-30», so a dash between two numbers is a range or a score — «матч 14-15»
/// is not a quarter past two.
fn split_written_time(word: &str) -> Option<(u32, u32)> {
    let (hour, minute) = word.split_once(':').or_else(|| word.split_once('.'))?;
    if !(1..=2).contains(&hour.len()) || minute.len() != 2 {
        return None;
    }
    Some((hour.parse().ok()?, minute.parse().ok()?))
}

/// Turns «9 вечера» into 21:00. Returns the correction, not the hour, so the
/// caller can tell "no part of day was said" from "it was said and changed
/// nothing".
fn meridiem_shift(word: &str) -> Option<fn(u32) -> u32> {
    match word {
        "утра" | "утром" | "am" | "morning" => {
            Some(|hour| if hour == 12 { 0 } else { hour })
        }
        "дня" | "вечера" | "вечером" | "pm" | "evening" => {
            Some(|hour| if hour < 12 { hour + 12 } else { hour })
        }
        "ночи" | "ночью" | "night" => Some(|hour| if hour == 12 { 0 } else { hour }),
        _ => None,
    }
}

/// «завтра», «в понедельник», "tomorrow", "on monday".
fn match_day(
    tokens: &[Token<'_>],
    index: usize,
    consumed: &[bool],
) -> Option<(usize, usize, DayHint)> {
    let word = tokens[index].lower.as_str();

    let hint = match word {
        "сегодня" | "today" => DayHint::Offset(0),
        "завтра" | "tomorrow" => DayHint::Offset(1),
        "послезавтра" => DayHint::Offset(2),
        other => DayHint::Named(weekday(other)?),
    };

    // «в понедельник»: the preposition goes with the day, the same way it goes
    // with a time.
    let takes_preposition = index > 0
        && !consumed[index - 1]
        && matches!(tokens[index - 1].lower.as_str(), "в" | "во" | "on");

    Some(if takes_preposition {
        (index - 1, 2, hint)
    } else {
        (index, 1, hint)
    })
}

/// «завтра утром», «вечером», "tomorrow morning", "in the evening".
///
/// A part of the day is not a time and is not treated as one: the reminder
/// lands on the hour this stands for, but nothing is subtracted from it,
/// because being warned half an hour before "the morning" means nothing.
fn match_part_of_day(
    tokens: &[Token<'_>],
    index: usize,
    consumed: &[bool],
) -> Option<(usize, usize, (u32, u32))> {
    let at = numbers::part_of_day(&tokens[index].lower)?;

    // «с утра», "in the morning" — the words that only lead into the phrase go
    // with it, or they stay behind in the title.
    let mut start = index;
    while start > 0
        && !consumed[start - 1]
        && matches!(
            tokens[start - 1].lower.as_str(),
            "в" | "во" | "с" | "со" | "под" | "in" | "the" | "at"
        )
    {
        start -= 1;
    }

    Some((start, index + 1 - start, at))
}

/// Both the nominative and the accusative, because «в среду» is what people
/// say and «среда» is what a dictionary holds.
fn weekday(word: &str) -> Option<Weekday> {
    let day = match word {
        "понедельник" | "monday" => Weekday::Mon,
        "вторник" | "tuesday" => Weekday::Tue,
        "среда" | "среду" | "wednesday" => Weekday::Wed,
        "четверг" | "thursday" => Weekday::Thu,
        "пятница" | "пятницу" | "friday" => Weekday::Fri,
        "суббота" | "субботу" | "saturday" => Weekday::Sat,
        "воскресенье" | "воскресение" | "sunday" => Weekday::Sun,
        _ => return None,
    };
    Some(day)
}

// ---------------------------------------------------------------------------
// Resolving
// ---------------------------------------------------------------------------

/// Places what was found on the calendar.
fn resolve(
    found: &Found,
    now: Timestamp,
    zone: Tz,
    fallback_time: TimePreset,
) -> AppResult<(Option<Timestamp>, bool)> {
    if let Some(minutes) = found.distance {
        // «через 20 минут» sets a timer rather than describing an event: the
        // person named the moment they want to hear from the app, so nothing is
        // taken off it. Subtracting the lead here would ring before they even
        // finished asking — with the shipped half hour, immediately.
        return Ok((Some(now.saturating_add_minutes(minutes)), false));
    }

    let today = now.to_zoned(zone)?.date_naive();
    let (hour, minute, named_clock_time) = match (found.clock, found.part_of_day) {
        (Some(clock), _) => (
            if clock.twelve_hour {
                daytime_reading(clock.hour)
            } else {
                clock.hour
            },
            clock.minute,
            true,
        ),
        // A part of the day is a rough hour, not an appointment: it fixes when
        // the reminder lands and still leaves nothing to be early for.
        (None, Some((hour, minute))) => (hour, minute, false),
        (None, None) if found.day.is_some() => (
            u32::from(fallback_time.hour()),
            u32::from(fallback_time.minute()),
            false,
        ),
        (None, None) => return Ok((None, false)),
    };

    let date = match found.day {
        Some(DayHint::Offset(days)) => today + Duration::days(days),
        Some(DayHint::Named(weekday)) => next_weekday(today, weekday),
        None => today,
    };

    let mut at = instant_at(date, hour, minute, zone)?;

    // An hour with no day attached means the next time the clock reads it:
    // «встреча в 9:00» said at noon is tomorrow's meeting, and nobody says
    // «завтра» about a time that has obviously passed. A day that *was* named
    // is left where it was put, even into the past — the person said it, and
    // the caller reports that back rather than silently moving it.
    if found.day.is_none() && at <= now {
        // A word-hour has a twin twelve hours away — «в девять» said at noon is
        // nine in the evening, not nine tomorrow morning. The twin is only
        // reached for once the daytime reading has gone, so «в три» said at
        // four in the afternoon becomes three tomorrow rather than three
        // tonight.
        let twin = match found.clock {
            Some(clock) if clock.twelve_hour => Some(twelve_hour_twin(hour)),
            _ => None,
        };
        let twin_at = match twin {
            Some(twin) => Some(instant_at(date, twin, minute, zone)?),
            None => None,
        };
        at = match twin_at {
            Some(twin_at) if twin_at > now => twin_at,
            _ => instant_at(date + Duration::days(1), hour, minute, zone)?,
        };
    }
    // A weekday resolves to the coming one, so today only counts while its
    // hour is still ahead.
    if matches!(found.day, Some(DayHint::Named(_))) && at <= now {
        at = instant_at(date + Duration::days(7), hour, minute, zone)?;
    }

    Ok((Some(at), named_clock_time))
}

/// The next time the clock reads a given hour: today while it is still ahead,
/// tomorrow once it has passed.
///
/// This is what a phrase with no time in it lands on, and it lives beside the
/// rest of the calendar arithmetic rather than in the use case so that the two
/// answer daylight saving the same way.
///
/// # Errors
/// Propagates a zone conversion failure.
pub fn next_time_of_day(time: TimePreset, now: Timestamp, zone: Tz) -> AppResult<Timestamp> {
    let today = now.to_zoned(zone)?.date_naive();
    let hour = u32::from(time.hour());
    let minute = u32::from(time.minute());

    let at = instant_at(today, hour, minute, zone)?;
    if at > now {
        return Ok(at);
    }
    instant_at(today + Duration::days(1), hour, minute, zone)
}

/// Which half of the day a spoken hour most likely meant.
///
/// «Встреча в три» is at three in the afternoon; «завтрак в девять» is at nine
/// in the morning. The line falls after six because the hours before it are
/// almost never said about the small hours — someone who means half past two at
/// night says «ночи», and that is a part of the day, which settles the question
/// before this is asked.
const fn daytime_reading(hour: u32) -> u32 {
    match hour {
        // «полпервого» is half past twelve, in the day rather than the night.
        0 => 12,
        1..=6 => hour + 12,
        _ => hour,
    }
}

/// The same reading on the other half of the clock face.
const fn twelve_hour_twin(hour: u32) -> u32 {
    if hour >= 12 {
        hour - 12
    } else {
        hour + 12
    }
}

fn next_weekday(today: NaiveDate, weekday: Weekday) -> NaiveDate {
    let ahead = i64::from(
        (weekday.num_days_from_monday() + 7 - today.weekday().num_days_from_monday()) % 7,
    );
    today + Duration::days(ahead)
}

/// A local reading turned into an instant.
///
/// The hour a country skips when it puts its clocks forward does not exist, and
/// the hour it repeats in autumn exists twice. The first is answered by moving
/// an hour on — a reminder that cannot be placed at 02:30 is wanted at 03:30,
/// not refused — and the second by taking the earlier of the two, which is the
/// one that comes sooner.
fn instant_at(date: NaiveDate, hour: u32, minute: u32, zone: Tz) -> AppResult<Timestamp> {
    let time = NaiveTime::from_hms_opt(hour, minute, 0).ok_or_else(|| {
        AppError::Validation(ValidationError::Invalid {
            field: "transcript",
        })
    })?;
    let local = date.and_time(time);

    let resolved = zone
        .from_local_datetime(&local)
        .earliest()
        .or_else(|| {
            zone.from_local_datetime(&(local + Duration::hours(1)))
                .earliest()
        })
        .ok_or_else(|| {
            AppError::Validation(ValidationError::Invalid {
                field: "transcript",
            })
        })?;

    Ok(Timestamp::from_millis(resolved.timestamp_millis()))
}

// ---------------------------------------------------------------------------
// Title
// ---------------------------------------------------------------------------

/// What is left once the time words are gone.
///
/// Words are re-joined with single spaces rather than sliced out of the
/// original: a recogniser's punctuation is its own guess, and «встреча. 15:00»
/// should not leave a trailing full stop in the title. A phrase that was
/// nothing *but* a time keeps the whole transcript instead of becoming an
/// untitled note — the person still has to recognise it in the list.
fn title_from(tokens: &[Token<'_>], consumed: &[bool], transcript: &str) -> String {
    let kept: Vec<&str> = tokens
        .iter()
        .zip(consumed)
        .filter(|(_, taken)| !**taken)
        .map(|(token, _)| token.text)
        .collect();

    let title = if kept.is_empty() {
        transcript.trim().to_owned()
    } else {
        kept.join(" ")
    };

    capitalise(&cut(&title))
}

fn cut(title: &str) -> String {
    if title.chars().count() <= MAX_SPOKEN_TITLE_LEN {
        return title.to_owned();
    }
    title.chars().take(MAX_SPOKEN_TITLE_LEN).collect()
}

/// A dictated phrase arrives lowercase more often than not, and a list of note
/// titles that all start small reads as broken.
fn capitalise(title: &str) -> String {
    let mut characters = title.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clock::{Clock as _, FixedClock};

    const ZONE: Tz = chrono_tz::Europe::Moscow;

    /// Monday, 10 August 2026, 12:00 in Moscow.
    fn noon() -> Timestamp {
        FixedClock::at_local(ZONE, 2026, 8, 10, 12, 0).now()
    }

    fn fallback() -> TimePreset {
        TimePreset::parse("19:00").expect("parses")
    }

    fn parse(text: &str) -> ParsedPhrase {
        parse_phrase(text, noon(), ZONE, fallback()).expect("parses")
    }

    fn local(phrase: &ParsedPhrase) -> String {
        phrase
            .named_at
            .expect("a time was named")
            .to_zoned(ZONE)
            .expect("representable")
            .format("%Y-%m-%d %H:%M")
            .to_string()
    }

    #[test]
    fn the_phrase_from_the_product_brief_becomes_a_title_and_a_time() {
        let phrase = parse("встреча. 15:00");
        assert_eq!(phrase.title, "Встреча");
        assert_eq!(local(&phrase), "2026-08-10 15:00");
        assert!(phrase.named_clock_time);
    }

    #[test]
    fn a_preposition_belongs_to_the_time_rather_than_the_title() {
        let phrase = parse("созвон с Ирой в 16:30");
        assert_eq!(phrase.title, "Созвон с Ирой");
        assert_eq!(local(&phrase), "2026-08-10 16:30");
    }

    #[test]
    fn a_time_the_recogniser_wrote_with_a_dot_is_still_a_time() {
        assert_eq!(local(&parse("встреча 15.30")), "2026-08-10 15:30");
    }

    #[test]
    fn a_bare_hour_counts_behind_a_preposition() {
        let phrase = parse("позвонить в 17");
        assert_eq!(phrase.title, "Позвонить");
        assert_eq!(local(&phrase), "2026-08-10 17:00");
    }

    #[test]
    fn a_bare_pair_of_numbers_counts_at_the_end_of_the_phrase() {
        assert_eq!(local(&parse("встреча 15 30")), "2026-08-10 15:30");
    }

    #[test]
    fn numbers_in_the_middle_of_a_phrase_are_not_a_time() {
        let phrase = parse("купить 15 яблок");
        assert_eq!(phrase.title, "Купить 15 яблок");
        assert_eq!(phrase.named_at, None);
    }

    #[test]
    fn the_part_of_day_moves_the_hour() {
        assert_eq!(local(&parse("встреча в 9 вечера")), "2026-08-10 21:00");
        assert_eq!(local(&parse("зарядка в 7 утра")), "2026-08-11 07:00");
    }

    #[test]
    fn an_hour_that_has_already_passed_today_means_tomorrow() {
        let phrase = parse("встреча в 9:00");
        assert_eq!(local(&phrase), "2026-08-11 09:00");
    }

    #[test]
    fn a_day_that_was_named_is_left_where_it_was_put() {
        // Said at noon, so the instant is behind us. The caller reports that
        // back rather than the parser quietly moving it to another day.
        let phrase = parse("сегодня в 9:00");
        assert_eq!(local(&phrase), "2026-08-10 09:00");
    }

    #[test]
    fn tomorrow_is_a_day_and_the_hour_is_still_the_one_that_was_said() {
        let phrase = parse("завтра встреча в 10:00");
        assert_eq!(phrase.title, "Встреча");
        assert_eq!(local(&phrase), "2026-08-11 10:00");
    }

    #[test]
    fn a_day_with_no_hour_lands_on_the_fallback_time() {
        let phrase = parse("послезавтра забрать посылку");
        assert_eq!(phrase.title, "Забрать посылку");
        assert_eq!(local(&phrase), "2026-08-12 19:00");
        assert!(
            !phrase.named_clock_time,
            "no clock time was said, so nothing should be subtracted from it"
        );
    }

    #[test]
    fn a_weekday_resolves_to_the_coming_one() {
        // The fixture is a Monday.
        assert_eq!(
            local(&parse("в пятницу зубной в 11:00")),
            "2026-08-14 11:00"
        );
        assert_eq!(local(&parse("в среду отчёт")), "2026-08-12 19:00");
    }

    #[test]
    fn today_as_a_weekday_counts_only_while_its_hour_is_ahead() {
        assert_eq!(local(&parse("в понедельник в 18:00")), "2026-08-10 18:00");
        assert_eq!(local(&parse("в понедельник в 09:00")), "2026-08-17 09:00");
    }

    #[test]
    fn a_distance_from_now_is_measured_from_now() {
        assert_eq!(
            local(&parse("через 20 минут позвонить")),
            "2026-08-10 12:20"
        );
        assert_eq!(local(&parse("через час выйти")), "2026-08-10 13:00");
        assert_eq!(local(&parse("через 2 часа выйти")), "2026-08-10 14:00");
        assert_eq!(local(&parse("через полчаса чай")), "2026-08-10 12:30");
    }

    #[test]
    fn english_reads_the_same_way() {
        assert_eq!(local(&parse("meeting at 15:00")), "2026-08-10 15:00");
        assert_eq!(
            local(&parse("call mum tomorrow at 9 pm")),
            "2026-08-11 21:00"
        );
        assert_eq!(
            local(&parse("in an hour take the cake out")),
            "2026-08-10 13:00"
        );
        assert_eq!(parse("meeting at 15:00").title, "Meeting");
    }

    #[test]
    fn a_phrase_with_no_time_in_it_is_still_a_phrase() {
        let phrase = parse("купить молоко");
        assert_eq!(phrase.title, "Купить молоко");
        assert_eq!(phrase.named_at, None);
        assert!(!phrase.named_clock_time);
    }

    #[test]
    fn a_phrase_that_was_nothing_but_a_time_keeps_it_as_the_title() {
        let phrase = parse("15:00");
        assert_eq!(phrase.title, "15:00");
        assert_eq!(local(&phrase), "2026-08-10 15:00");
    }

    #[test]
    fn a_second_time_stays_in_the_title_where_it_can_be_seen() {
        let phrase = parse("встреча в 15:00 и созвон в 18:00");
        assert_eq!(phrase.title, "Встреча и созвон в 18:00");
        assert_eq!(local(&phrase), "2026-08-10 15:00");
    }

    #[test]
    fn a_time_that_does_not_exist_on_a_clock_is_left_in_the_title() {
        let phrase = parse("встреча в 25:70");
        assert_eq!(phrase.title, "Встреча в 25:70");
        assert_eq!(phrase.named_at, None);
    }

    #[test]
    fn an_empty_transcript_is_a_validation_error() {
        let error = parse_phrase("   ", noon(), ZONE, fallback()).expect_err("must reject");
        assert_eq!(error.code(), "validation_required");
    }

    #[test]
    fn a_dictation_longer_than_a_title_is_cut_rather_than_refused() {
        let long = "слово ".repeat(60);
        let phrase = parse(&long);
        assert_eq!(phrase.title.chars().count(), MAX_SPOKEN_TITLE_LEN);
    }

    // -----------------------------------------------------------------------
    // Numbers said as words
    // -----------------------------------------------------------------------

    #[test]
    fn an_hour_said_as_a_word_lands_in_the_afternoon_where_people_mean_it() {
        // Said at noon. Nobody says «в три» about three in the morning.
        let phrase = parse("встреча в три часа");
        assert_eq!(phrase.title, "Встреча");
        assert_eq!(local(&phrase), "2026-08-10 15:00");
    }

    #[test]
    fn a_part_of_day_beside_a_word_hour_settles_it_outright() {
        assert_eq!(local(&parse("в три часа дня")), "2026-08-10 15:00");
        assert_eq!(local(&parse("в семь вечера")), "2026-08-10 19:00");
        // Nine in the morning has gone by noon, so it is tomorrow's nine.
        assert_eq!(local(&parse("в девять утра зарядка")), "2026-08-11 09:00");
    }

    #[test]
    fn a_morning_hour_that_has_passed_becomes_its_evening_twin_today() {
        // «в девять» at noon is nine tonight — reaching for tomorrow morning
        // would skip nine hours the person can still use.
        assert_eq!(local(&parse("позвонить в девять")), "2026-08-10 21:00");
        assert_eq!(local(&parse("в одиннадцать")), "2026-08-10 23:00");
    }

    #[test]
    fn the_twin_is_only_reached_for_when_the_daytime_reading_has_gone() {
        // 16:00 in Moscow. «в три» means three in the afternoon, which has
        // passed; three tonight is not what was meant, so it is tomorrow.
        let now = FixedClock::at_local(ZONE, 2026, 8, 10, 16, 0).now();
        let phrase = parse_phrase("в три", now, ZONE, fallback()).expect("parses");
        assert_eq!(
            phrase
                .named_at
                .expect("a time was named")
                .to_zoned(ZONE)
                .expect("representable")
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            "2026-08-11 15:00"
        );
    }

    #[test]
    fn digits_keep_the_hour_they_were_written_with() {
        // The deliberate difference: `9` is a clock reading and «девять» is a
        // word. Only the word has two possible halves of the day.
        assert_eq!(local(&parse("встреча в 9")), "2026-08-11 09:00");
        assert_eq!(local(&parse("встреча в 21")), "2026-08-10 21:00");
    }

    #[test]
    fn the_word_for_one_is_also_the_word_for_an_hour() {
        assert_eq!(local(&parse("обед в час дня")), "2026-08-10 13:00");
        assert_eq!(parse("обед в час дня").title, "Обед");
    }

    #[test]
    fn tens_and_units_make_one_number() {
        assert_eq!(local(&parse("в двадцать три тридцать")), "2026-08-10 23:30");
    }

    #[test]
    fn halves_count_the_hour_they_are_halfway_into() {
        // «полвторого» is 13:30, not 14:30 — the commonest way to be an hour
        // wrong about a Russian time.
        assert_eq!(local(&parse("полвторого созвон")), "2026-08-10 13:30");
        assert_eq!(local(&parse("в половине третьего")), "2026-08-10 14:30");
        assert_eq!(local(&parse("пол шестого")), "2026-08-10 17:30");
    }

    #[test]
    fn half_past_twelve_is_the_middle_of_the_day_rather_than_the_night() {
        assert_eq!(local(&parse("полпервого")), "2026-08-10 12:30");
    }

    #[test]
    fn minutes_short_of_an_hour_are_read_backwards_from_it() {
        assert_eq!(local(&parse("без пятнадцати шесть")), "2026-08-10 17:45");
        assert_eq!(local(&parse("без четверти шесть")), "2026-08-10 17:45");
        assert_eq!(local(&parse("выйти без десяти пять")), "2026-08-10 16:50");
        assert_eq!(parse("выйти без десяти пять").title, "Выйти");
    }

    #[test]
    fn noon_and_midnight_need_no_working_out() {
        // Said in the morning, because the fixture clock *is* noon and a time
        // that has arrived is not a time to be reminded at.
        let morning = FixedClock::at_local(ZONE, 2026, 8, 10, 9, 0).now();
        let phrase = parse_phrase("полдень обед", morning, ZONE, fallback()).expect("parses");
        assert_eq!(
            phrase
                .named_at
                .expect("a time was named")
                .to_zoned(ZONE)
                .expect("representable")
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            "2026-08-10 12:00"
        );
        assert_eq!(local(&parse("в полночь")), "2026-08-11 00:00");
    }

    #[test]
    fn english_says_the_same_things_its_own_way() {
        assert_eq!(local(&parse("call at half past three")), "2026-08-10 15:30");
        assert_eq!(local(&parse("leave at quarter to six")), "2026-08-10 17:45");
        assert_eq!(local(&parse("ten past three")), "2026-08-10 15:10");
        assert_eq!(local(&parse("at midnight")), "2026-08-11 00:00");
        assert_eq!(parse("call at half past three").title, "Call");
    }

    // -----------------------------------------------------------------------
    // Parts of the day
    // -----------------------------------------------------------------------

    #[test]
    fn a_part_of_the_day_with_no_hour_is_a_rough_time_rather_than_the_fallback() {
        let phrase = parse("завтра утром позвонить маме");
        assert_eq!(phrase.title, "Позвонить маме");
        assert_eq!(local(&phrase), "2026-08-11 09:00");
        assert!(
            !phrase.named_clock_time,
            "nothing was named to be early for, so the lead must not apply"
        );
    }

    #[test]
    fn each_part_of_the_day_has_its_own_hour() {
        assert_eq!(local(&parse("вечером забрать посылку")), "2026-08-10 19:00");
        assert_eq!(local(&parse("завтра днём в банк")), "2026-08-11 13:00");
        assert_eq!(local(&parse("завтра ночью рейс")), "2026-08-11 22:00");
        assert_eq!(local(&parse("tomorrow morning gym")), "2026-08-11 09:00");
    }

    #[test]
    fn the_words_that_only_lead_into_a_part_of_the_day_go_with_it() {
        // The second «в» belongs to «в спортзал» and stays where it was said.
        assert_eq!(parse("с утра в спортзал").title, "В спортзал");
        assert_eq!(parse("in the evening call back").title, "Call back");
    }

    // -----------------------------------------------------------------------
    // Numbers that are not times
    // -----------------------------------------------------------------------

    #[test]
    fn the_minutes_said_after_the_hour_word_are_part_of_the_time() {
        // «в 15 часов 30 минут» is what a recogniser writes when the hour and
        // the minutes are spoken with their units. Reading only the hour puts
        // the alarm half an hour early and leaves «30 минут» in the title.
        let phrase = parse("в 15 часов 30 минут созвон");
        assert_eq!(local(&phrase), "2026-08-10 15:30");
        assert_eq!(phrase.title, "Созвон");
    }

    #[test]
    fn a_count_behind_a_preposition_is_not_an_hour() {
        // «на» never vouches for a bare hour: it is the commonest way in
        // Russian to say how many of something there are.
        for text in [
            "уехать на 2 недели",
            "разогреть на 3 минуты",
            "купить билеты на 5 число",
        ] {
            let phrase = parse(text);
            assert_eq!(phrase.named_at, None, "{text}");
            assert!(
                phrase.title.to_lowercase().contains("на"),
                "{text} lost its preposition: {}",
                phrase.title
            );
        }
    }

    #[test]
    fn a_multiplier_is_not_an_hour_either() {
        let phrase = parse("купить в 2 раза больше молока");
        assert_eq!(phrase.named_at, None);
        assert_eq!(phrase.title, "Купить в 2 раза больше молока");
    }

    #[test]
    fn a_part_of_the_day_still_turns_a_bare_hour_into_the_afternoon() {
        // «дня» is a unit of time and a part of the day at once. Behind a
        // vouching preposition it is the second one.
        assert_eq!(local(&parse("в 3 дня встреча")), "2026-08-10 15:00");
    }

    #[test]
    fn a_dot_between_numbers_is_only_a_time_where_a_time_can_be() {
        // A date, a price and a score. Each used to become a reminder and lose
        // the number it was about.
        for text in ["встреча 12.03 в офисе", "купить хлеб 2.50 и молоко"]
        {
            let phrase = parse(text);
            assert_eq!(phrase.named_at, None, "{text}");
        }
        // At the end of the phrase, where a bare pair is already read as a
        // time, the dot spelling still counts.
        assert_eq!(local(&parse("созвон 16.45")), "2026-08-10 16:45");
    }

    #[test]
    fn a_dash_between_numbers_is_a_range_rather_than_a_time() {
        let phrase = parse("матч 14-15");
        assert_eq!(phrase.named_at, None);
        assert_eq!(phrase.title, "Матч 14-15");
    }

    #[test]
    fn an_apostrophe_inside_a_word_does_not_split_it() {
        assert_eq!(parse("call mum's doctor").title, "Call mum's doctor");
    }

    #[test]
    fn a_length_of_time_is_not_an_hour() {
        // «на 20 минут» says how long, not when. Read as an hour it would put
        // the reminder at eight in the evening.
        let phrase = parse("опоздаю на 20 минут");
        assert_eq!(phrase.named_at, None);
        assert_eq!(phrase.title, "Опоздаю на 20 минут");
    }

    #[test]
    fn a_timer_has_nothing_taken_off_it() {
        // «через 20 минут» names the moment the person wants to hear from the
        // app. Subtracting a half-hour lead would ring before they finished
        // asking.
        let phrase = parse("через 20 минут снять с плиты");
        assert_eq!(local(&phrase), "2026-08-10 12:20");
        assert!(!phrase.named_clock_time);
    }

    #[test]
    fn a_timer_counts_in_words_too() {
        assert_eq!(
            local(&parse("через двадцать минут чай")),
            "2026-08-10 12:20"
        );
        assert_eq!(local(&parse("через две недели отчёт")), "2026-08-24 12:00");
    }

    #[test]
    fn an_hour_a_country_skips_is_moved_on_rather_than_refused() {
        // Moscow has no DST any more, so this is asked of a zone that does:
        // Chile puts its clocks forward at midnight, and 00:30 does not exist
        // on 6 September 2026.
        let zone = chrono_tz::America::Santiago;
        let now = FixedClock::at_local(zone, 2026, 9, 5, 12, 0).now();
        let phrase = parse_phrase("завтра в 00:30 стирка", now, zone, fallback()).expect("parses");
        let at = phrase
            .named_at
            .expect("a time was named")
            .to_zoned(zone)
            .expect("representable");
        assert_eq!(at.format("%Y-%m-%d %H:%M").to_string(), "2026-09-06 01:30");
    }
}
