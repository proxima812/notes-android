//! Time.
//!
//! Every instant in the core is a [`Timestamp`]: milliseconds since the Unix
//! epoch, UTC. Wall-clock intent ("every day at 09:00") is stored separately as
//! an IANA zone plus local components, because an absolute instant alone cannot
//! survive a DST transition or the user flying to another country.
//!
//! Nothing calls `Utc::now()` directly. Services take a [`Clock`], so tests can
//! place themselves at an exact instant — which is the only way to assert
//! anything useful about recurrence, quiet hours or snooze.

use chrono::{DateTime, TimeZone as _, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{AppError, ValidationError};

/// Milliseconds since the Unix epoch, UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    #[must_use]
    pub const fn as_millis(self) -> i64 {
        self.0
    }

    /// Converts to a `chrono` instant.
    ///
    /// # Errors
    /// Returns [`ValidationError::Invalid`] when the value is outside the range
    /// `chrono` can represent, which only happens for corrupted input.
    pub fn to_utc(self) -> Result<DateTime<Utc>, AppError> {
        DateTime::from_timestamp_millis(self.0).ok_or(AppError::Validation(
            ValidationError::Invalid { field: "timestamp" },
        ))
    }

    #[must_use]
    pub fn from_utc(value: DateTime<Utc>) -> Self {
        Self(value.timestamp_millis())
    }

    /// Same instant seen from a given zone.
    ///
    /// # Errors
    /// Propagates the failure of [`Self::to_utc`].
    pub fn to_zoned(self, zone: Tz) -> Result<DateTime<Tz>, AppError> {
        Ok(self.to_utc()?.with_timezone(&zone))
    }

    #[must_use]
    pub const fn saturating_add_minutes(self, minutes: i64) -> Self {
        Self(self.0.saturating_add(minutes.saturating_mul(60_000)))
    }

    #[must_use]
    pub const fn saturating_add_millis(self, millis: i64) -> Self {
        Self(self.0.saturating_add(millis))
    }

    #[must_use]
    pub const fn is_before(self, other: Self) -> bool {
        self.0 < other.0
    }
}

/// Source of "now". Injected everywhere instead of calling the OS clock.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// The real device clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_utc(Utc::now())
    }
}

/// A clock frozen at a chosen instant, for tests.
#[derive(Debug, Clone)]
pub struct FixedClock {
    at: Timestamp,
}

impl FixedClock {
    #[must_use]
    pub const fn new(at: Timestamp) -> Self {
        Self { at }
    }

    /// Builds a clock from a local wall-clock reading in a given zone.
    ///
    /// # Panics
    /// Panics when the components do not name a real instant in that zone,
    /// which in a test means the test itself is wrong.
    #[must_use]
    pub fn at_local(zone: Tz, year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Self {
        let local = zone
            .with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .expect("the test instant must be unambiguous in this zone");
        Self::new(Timestamp::from_utc(local.with_timezone(&Utc)))
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.at
    }
}

/// Shared handle used by services.
pub type SharedClock = Arc<dyn Clock>;

/// Parses an IANA zone name coming from the frontend or a stored row.
///
/// # Errors
/// Returns [`ValidationError::UnknownTimeZone`] for anything `chrono-tz` does
/// not recognise, so a corrupted setting cannot silently reschedule everything
/// into UTC.
pub fn parse_zone(name: &str) -> Result<Tz, AppError> {
    name.parse::<Tz>().map_err(|_| {
        AppError::Validation(ValidationError::UnknownTimeZone {
            value: name.to_owned(),
        })
    })
}

impl rusqlite::ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(self.0))
    }
}

impl rusqlite::types::FromSql for Timestamp {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_i64().map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_clock_does_not_move() {
        let clock = FixedClock::new(Timestamp::from_millis(1_700_000_000_000));
        assert_eq!(clock.now(), clock.now());
        assert_eq!(clock.now().as_millis(), 1_700_000_000_000);
    }

    #[test]
    fn local_construction_accounts_for_the_zone_offset() {
        // Moscow is UTC+3 year round, so 12:00 local is 09:00 UTC.
        let clock = FixedClock::at_local(chrono_tz::Europe::Moscow, 2026, 8, 1, 12, 0);
        let utc = clock.now().to_utc().expect("representable");
        assert_eq!(
            utc.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "2026-08-01T09:00:00Z"
        );
    }

    #[test]
    fn a_timestamp_round_trips_through_a_zone() {
        let zone = chrono_tz::Europe::Moscow;
        let clock = FixedClock::at_local(zone, 2026, 8, 1, 9, 30);
        let zoned = clock.now().to_zoned(zone).expect("representable");
        assert_eq!(zoned.format("%H:%M").to_string(), "09:30");
    }

    #[test]
    fn adding_minutes_saturates_instead_of_overflowing() {
        let far_future = Timestamp::from_millis(i64::MAX - 10);
        assert_eq!(
            far_future.saturating_add_minutes(1_000).as_millis(),
            i64::MAX
        );
    }

    #[test]
    fn an_unknown_zone_is_rejected_rather_than_defaulting_to_utc() {
        let error = parse_zone("Middle/Earth").expect_err("not a real zone");
        assert_eq!(error.code(), "validation_unknown_timezone");
    }

    #[test]
    fn known_zones_parse() {
        assert!(parse_zone("Europe/Moscow").is_ok());
        assert!(parse_zone("UTC").is_ok());
        assert!(parse_zone("America/Argentina/Salta").is_ok());
    }
}
