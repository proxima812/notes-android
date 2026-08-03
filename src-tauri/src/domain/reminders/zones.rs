//! Keeping the wall clock, not the instant.
//!
//! A reminder is set by pointing at a time on a clock. The instant that time
//! happens to be is a consequence of where the user was standing, and it stops
//! being the right instant the moment they move: someone who asks to be
//! reminded at 09:00 and then flies two zones east still means 09:00, not 07:00.
//!
//! So the stored instant is treated as a rendering of a wall-clock intent in a
//! known zone, and moving to another zone re-renders the same intent there.

use chrono::{LocalResult, NaiveDateTime, TimeZone as _};
use chrono_tz::Tz;

use crate::domain::clock::Timestamp;
use crate::error::{AppError, AppResult, ValidationError};

/// Parses an IANA zone name.
///
/// # Errors
/// Returns [`ValidationError::UnknownTimeZone`] for a name the database of
/// zones does not have.
pub fn parse_zone(name: &str) -> AppResult<Tz> {
    name.parse().map_err(|_| {
        AppError::Validation(ValidationError::UnknownTimeZone {
            value: name.to_owned(),
        })
    })
}

/// The same time on the clock, seen in another zone.
///
/// # Errors
/// Fails only when the stored instant is outside the range `chrono` can
/// represent, which means the row was already corrupt.
pub fn reinterpret(instant: Timestamp, from: Tz, to: Tz) -> AppResult<Timestamp> {
    let wall = instant.to_zoned(from)?.naive_local();
    Ok(resolve(wall, to))
}

/// Turns a wall-clock reading into an instant in `zone`.
///
/// Twice a year a given reading is either ambiguous or does not exist at all,
/// and neither may be allowed to lose the reminder:
///
/// * when the clocks go back, 01:30 happens twice — the first one is used, so
///   the reminder arrives at the earlier of the two moments the user could have
///   meant rather than an hour late;
/// * when the clocks go forward, 02:30 never happens — the instant the gap ends
///   is used, so a reminder set for a time that was skipped still fires that
///   morning instead of silently never arriving.
#[must_use]
pub fn resolve(wall: NaiveDateTime, zone: Tz) -> Timestamp {
    match zone.from_local_datetime(&wall) {
        LocalResult::Single(instant) | LocalResult::Ambiguous(instant, _) => {
            Timestamp::from_utc(instant.to_utc())
        }
        LocalResult::None => {
            // Walk forward a minute at a time until the clock exists again. The
            // largest gap any zone has ever had is under two hours, so this
            // stops almost immediately; the bound is there so a malformed zone
            // cannot spin.
            let mut candidate = wall;
            for _ in 0..MAX_GAP_MINUTES {
                candidate += chrono::Duration::minutes(1);
                if let LocalResult::Single(instant) | LocalResult::Ambiguous(instant, _) =
                    zone.from_local_datetime(&candidate)
                {
                    return Timestamp::from_utc(instant.to_utc());
                }
            }
            // Unreachable for any real zone; falling back to UTC keeps the
            // reminder rather than dropping it.
            Timestamp::from_utc(chrono::Utc.from_utc_datetime(&wall))
        }
    }
}

/// Longest daylight-saving gap to step over. Real ones are at most two hours.
const MAX_GAP_MINUTES: i32 = 180;

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn wall(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .expect("valid date")
            .and_hms_opt(hour, minute, 0)
            .expect("valid time")
    }

    fn reading(instant: Timestamp, zone: Tz) -> String {
        instant
            .to_zoned(zone)
            .expect("representable")
            .format("%Y-%m-%d %H:%M")
            .to_string()
    }

    #[test]
    fn flying_east_keeps_the_time_on_the_clock() {
        // 09:00 set in Almaty, then the phone lands in Moscow.
        let almaty = resolve(wall(2026, 8, 3, 9, 0), chrono_tz::Asia::Almaty);
        let moved = reinterpret(almaty, chrono_tz::Asia::Almaty, chrono_tz::Europe::Moscow)
            .expect("reinterprets");

        assert_eq!(
            reading(moved, chrono_tz::Europe::Moscow),
            "2026-08-03 09:00"
        );
        assert_ne!(
            moved, almaty,
            "the instant has to move for the clock to stay"
        );
    }

    #[test]
    fn staying_put_changes_nothing() {
        let instant = resolve(wall(2026, 8, 3, 9, 0), chrono_tz::Asia::Almaty);
        let same = reinterpret(instant, chrono_tz::Asia::Almaty, chrono_tz::Asia::Almaty)
            .expect("reinterprets");
        assert_eq!(same, instant);
    }

    #[test]
    fn a_time_the_clocks_skipped_still_happens_that_morning() {
        // Central European clocks jump from 02:00 to 03:00 on 29 March 2026, so
        // 02:30 does not exist. The reminder must not vanish with it.
        let instant = resolve(wall(2026, 3, 29, 2, 30), chrono_tz::Europe::Berlin);
        assert_eq!(
            reading(instant, chrono_tz::Europe::Berlin),
            "2026-03-29 03:00"
        );
    }

    #[test]
    fn a_time_the_clocks_repeated_uses_the_first_one() {
        // 25 October 2026: 02:30 happens twice in Berlin.
        let instant = resolve(wall(2026, 10, 25, 2, 30), chrono_tz::Europe::Berlin);
        let offset = chrono::Offset::fix(
            instant
                .to_zoned(chrono_tz::Europe::Berlin)
                .expect("representable")
                .offset(),
        );
        assert_eq!(
            reading(instant, chrono_tz::Europe::Berlin),
            "2026-10-25 02:30"
        );
        assert_eq!(
            offset.local_minus_utc(),
            2 * 3600,
            "summer time is still in force at the earlier of the two"
        );
    }

    #[test]
    fn an_ordinary_time_survives_a_round_trip() {
        let original = wall(2026, 12, 31, 23, 59);
        let instant = resolve(original, chrono_tz::Asia::Almaty);
        assert_eq!(
            reading(instant, chrono_tz::Asia::Almaty),
            "2026-12-31 23:59"
        );
    }

    #[test]
    fn an_unknown_zone_is_a_validation_error() {
        let error = parse_zone("Mars/Olympus").expect_err("must fail");
        assert_eq!(error.code(), "validation_unknown_timezone");
    }

    #[test]
    fn every_zone_the_app_offers_can_be_parsed() {
        for name in ["UTC", "Asia/Almaty", "Europe/Moscow", "America/New_York"] {
            parse_zone(name).expect("known zone");
        }
    }
}
