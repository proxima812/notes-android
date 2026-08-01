//! Typed identifiers.
//!
//! Each entity gets its own newtype so the compiler rejects passing a `TaskId`
//! where a `NoteId` belongs. New ids are UUIDv7: they sort by creation time,
//! which keeps SQLite's B-tree inserts local instead of scattering them the way
//! UUIDv4 does, and gives every table a natural chronological order for free.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::error::{AppError, ValidationError};

macro_rules! define_id {
    ($name:ident, $field:literal) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generates a fresh, time-ordered identifier.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            #[must_use]
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            /// Parses an identifier received from the frontend or a backup.
            ///
            /// # Errors
            /// Returns [`ValidationError::Invalid`] when the text is not a UUID.
            pub fn parse(value: &str) -> Result<Self, AppError> {
                Uuid::parse_str(value)
                    .map(Self)
                    .map_err(|_| AppError::Validation(ValidationError::Invalid { field: $field }))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                // Hyphenated lowercase: the exact form stored in SQLite and
                // expected by the branded types on the TypeScript side.
                write!(formatter, "{}", self.0.as_hyphenated())
            }
        }

        impl rusqlite::ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                Ok(rusqlite::types::ToSqlOutput::from(
                    self.0.as_hyphenated().to_string(),
                ))
            }
        }

        impl rusqlite::types::FromSql for $name {
            fn column_result(
                value: rusqlite::types::ValueRef<'_>,
            ) -> rusqlite::types::FromSqlResult<Self> {
                let text = value.as_str()?;
                Uuid::parse_str(text)
                    .map(Self)
                    .map_err(|error| rusqlite::types::FromSqlError::Other(Box::new(error)))
            }
        }
    };
}

define_id!(NoteId, "note_id");
define_id!(NoteBlockId, "note_block_id");
define_id!(TaskId, "task_id");
define_id!(ReminderId, "reminder_id");
define_id!(ReminderOccurrenceId, "reminder_occurrence_id");
define_id!(AttachmentId, "attachment_id");
define_id!(FolderId, "folder_id");
define_id!(TagId, "tag_id");
define_id!(TemplateId, "template_id");
define_id!(SavedSearchId, "saved_search_id");
define_id!(BackupRecordId, "backup_id");
define_id!(ActivityId, "activity_id");
define_id!(NotificationEventId, "notification_event_id");
define_id!(TaskCompletionId, "task_completion_id");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_unique() {
        let first = NoteId::new();
        let second = NoteId::new();
        assert_ne!(first, second);
    }

    #[test]
    fn v7_ids_sort_in_creation_order() {
        // This is what keeps `ORDER BY id` meaningful and B-tree inserts local.
        let mut ids: Vec<NoteId> = (0..64).map(|_| NoteId::new()).collect();
        let generated = ids.clone();
        ids.sort();
        assert_eq!(ids, generated);
    }

    #[test]
    fn parsing_round_trips_through_display() {
        let id = TaskId::new();
        let parsed = TaskId::parse(&id.to_string()).expect("the id is valid");
        assert_eq!(id, parsed);
    }

    #[test]
    fn parsing_rejects_a_non_uuid() {
        let error = NoteId::parse("note-1").expect_err("a plain word is not an id");
        assert_eq!(error.code(), "validation_invalid");
    }

    #[test]
    fn display_is_hyphenated_lowercase() {
        let id = NoteId::from_uuid(
            Uuid::parse_str("0193B3B2-4D3C-7C9A-8F2E-1A2B3C4D5E6F").expect("valid uuid"),
        );
        assert_eq!(id.to_string(), "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e6f");
    }
}
