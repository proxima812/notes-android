//! Numbers said out loud.
//!
//! A recogniser writes `15:00` when someone says "пятнадцать ноль ноль", but it
//! writes «в три часа», «полвторого» and «без пятнадцати шесть» exactly as they
//! were said — those are words, not digits, and no amount of formatting turns
//! them into a clock. This module is the dictionary that does.
//!
//! Only the words a clock needs are here: nothing above fifty-nine, no
//! thousands, no arithmetic. A number this table does not know stays a word and
//! ends up in the note's title, which is the safe direction to fail in.
//!
//! Russian nouns decline, and speech arrives declined: «без пятнадцати» is the
//! genitive of «пятнадцать» and «в половине третьего» carries two cases in
//! three words. The oblique forms are listed beside the nominative rather than
//! stripped by rule, because a rule that guesses endings would also accept
//! words that are not numbers at all.

/// A cardinal — «три», «пятнадцать», "twenty" — as far as a clock needs them.
///
/// Tens and units are separate entries; putting «двадцать три» together is the
/// caller's job, since only it can see whether the next word is even part of
/// the same number.
#[must_use]
pub fn cardinal(word: &str) -> Option<u32> {
    let value = match word {
        "ноль" | "нуль" | "zero" => 0,
        // «час» is the hour it names as well as the unit: «в час дня» is one
        // o'clock. «через час» never reaches here — the distance rule takes it
        // first, which is what keeps the two meanings apart.
        "один" | "одна" | "одного" | "час" | "one" => 1,
        "два" | "две" | "двух" | "two" => 2,
        "три" | "трёх" | "трех" | "three" => 3,
        "четыре" | "четырёх" | "четырех" | "four" => 4,
        "пять" | "пяти" | "five" => 5,
        "шесть" | "шести" | "six" => 6,
        "семь" | "семи" | "seven" => 7,
        "восемь" | "восьми" | "eight" => 8,
        "девять" | "девяти" | "nine" => 9,
        "десять" | "десяти" | "ten" => 10,
        "одиннадцать" | "одиннадцати" | "eleven" => 11,
        "двенадцать" | "двенадцати" | "twelve" => 12,
        "тринадцать" | "тринадцати" | "thirteen" => 13,
        "четырнадцать" | "четырнадцати" | "fourteen" => 14,
        "пятнадцать" | "пятнадцати" | "четверть" | "четверти" | "fifteen" | "quarter" => {
            15
        }
        "шестнадцать" | "шестнадцати" | "sixteen" => 16,
        "семнадцать" | "семнадцати" | "seventeen" => 17,
        "восемнадцать" | "восемнадцати" | "eighteen" => 18,
        "девятнадцать" | "девятнадцати" | "nineteen" => 19,
        "двадцать" | "двадцати" | "twenty" => 20,
        "тридцать" | "тридцати" | "thirty" => 30,
        "сорок" | "сорока" | "forty" => 40,
        "пятьдесят" | "пятидесяти" | "fifty" => 50,
        _ => return None,
    };
    Some(value)
}

/// True for the words that can carry units behind them: «двадцать три».
///
/// Fifty is the last one, because a clock never counts past fifty-nine.
#[must_use]
pub fn is_tens(value: u32) -> bool {
    matches!(value, 20 | 30 | 40 | 50)
}

/// An ordinal in the genitive — «второго», «третьего» — which is the case
/// Russian uses for halves: «полвторого» is half of the second hour, 01:30.
#[must_use]
pub fn ordinal(word: &str) -> Option<u32> {
    let value = match word {
        "первого" | "первому" | "first" => 1,
        "второго" | "второму" | "second" => 2,
        "третьего" | "третьему" | "third" => 3,
        "четвёртого" | "четвертого" | "fourth" => 4,
        "пятого" | "fifth" => 5,
        "шестого" | "sixth" => 6,
        "седьмого" | "seventh" => 7,
        "восьмого" | "eighth" => 8,
        "девятого" | "ninth" => 9,
        "десятого" | "tenth" => 10,
        "одиннадцатого" | "eleventh" => 11,
        "двенадцатого" | "twelfth" => 12,
        _ => return None,
    };
    Some(value)
}

/// The hour named by a written-together half: «полвторого» → 2, meaning 01:30.
///
/// Recognisers write this one as a single word about as often as two, and the
/// two-word spelling is handled by the caller — here the prefix is stripped and
/// the rest looked up as an ordinary ordinal.
#[must_use]
pub fn half_of(word: &str) -> Option<u32> {
    // `strip_prefix` works on bytes, and «пол» is six of them in UTF-8; using
    // it rather than slicing by index keeps the boundary correct.
    let rest = word.strip_prefix("пол")?;
    if rest.is_empty() {
        return None;
    }
    ordinal(rest)
}

/// The two hours that have names instead of numbers.
#[must_use]
pub fn named_hour(word: &str) -> Option<(u32, u32)> {
    match word {
        "полдень" | "полудня" | "noon" | "midday" => Some((12, 0)),
        "полночь" | "полуночи" | "midnight" => Some((0, 0)),
        _ => None,
    }
}

/// The rough hour a part of the day stands for, for phrases that name no clock
/// time at all — «завтра утром».
///
/// These are the app's opinion, not a fact: morning starts when you get up.
/// Nine, one, seven and ten are the hours a reminder is most likely to be
/// useful in each stretch, and the person can always move it afterwards.
#[must_use]
pub fn part_of_day(word: &str) -> Option<(u32, u32)> {
    let hour = match word {
        "утро" | "утром" | "утра" | "morning" => 9,
        "день" | "днём" | "днем" | "полдня" | "afternoon" => 13,
        "вечер" | "вечером" | "вечера" | "evening" => 19,
        "ночь" | "ночью" | "ночи" | "night" => 22,
        _ => return None,
    };
    Some((hour, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbers_a_clock_needs_are_all_there() {
        for (word, value) in [
            ("ноль", 0),
            ("три", 3),
            ("двенадцать", 12),
            ("пятнадцать", 15),
            ("двадцать", 20),
            ("сорок", 40),
            ("fifty", 50),
        ] {
            assert_eq!(cardinal(word), Some(value), "{word}");
        }
    }

    #[test]
    fn declined_forms_count_because_that_is_how_they_are_said() {
        // «без пятнадцати шесть», «без двадцати пяти семь».
        assert_eq!(cardinal("пятнадцати"), Some(15));
        assert_eq!(cardinal("двадцати"), Some(20));
        assert_eq!(cardinal("пяти"), Some(5));
        assert_eq!(cardinal("трёх"), Some(3));
    }

    #[test]
    fn a_quarter_is_fifteen_minutes_in_both_languages() {
        assert_eq!(cardinal("четверть"), Some(15));
        assert_eq!(cardinal("четверти"), Some(15));
        assert_eq!(cardinal("quarter"), Some(15));
    }

    #[test]
    fn the_hour_word_is_also_the_number_one() {
        // «в час дня». The other meaning — «через час» — never reaches this
        // table, because the distance rule matches first.
        assert_eq!(cardinal("час"), Some(1));
    }

    #[test]
    fn a_word_that_is_not_a_number_is_not_a_number() {
        for word in ["встреча", "часов", "", "три́", "twentyish"] {
            assert_eq!(cardinal(word), None, "{word}");
        }
    }

    #[test]
    fn only_round_tens_can_carry_units_behind_them() {
        assert!(is_tens(20));
        assert!(is_tens(50));
        assert!(!is_tens(15));
        assert!(!is_tens(60));
    }

    #[test]
    fn halves_written_as_one_word_are_read_as_the_hour_before() {
        // «полвторого» is half of the second hour: 01:30.
        assert_eq!(half_of("полвторого"), Some(2));
        assert_eq!(half_of("полшестого"), Some(6));
        assert_eq!(half_of("полдвенадцатого"), Some(12));
    }

    #[test]
    fn the_half_prefix_alone_is_not_a_time() {
        assert_eq!(half_of("пол"), None);
        assert_eq!(half_of("полка"), None);
        assert_eq!(half_of("получка"), None);
    }

    #[test]
    fn noon_and_midnight_are_hours_with_names() {
        assert_eq!(named_hour("полдень"), Some((12, 0)));
        assert_eq!(named_hour("полночь"), Some((0, 0)));
        assert_eq!(named_hour("midnight"), Some((0, 0)));
        assert_eq!(named_hour("вечер"), None);
    }

    #[test]
    fn each_part_of_the_day_has_one_hour_the_app_stands_behind() {
        assert_eq!(part_of_day("утром"), Some((9, 0)));
        assert_eq!(part_of_day("днём"), Some((13, 0)));
        assert_eq!(part_of_day("вечером"), Some((19, 0)));
        assert_eq!(part_of_day("ночью"), Some((22, 0)));
        assert_eq!(part_of_day("завтра"), None);
    }
}
