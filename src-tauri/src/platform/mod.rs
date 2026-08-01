//! The bridge to Android.
//!
//! Traits here describe what the core needs from the device; the adapters next
//! to them are the only code that talks to a Tauri plugin.

pub mod alarms;
pub mod tauri_alarms;

pub use alarms::{Alarm, AlarmClock, AlarmPermissions};
pub use tauri_alarms::TauriAlarmClock;
