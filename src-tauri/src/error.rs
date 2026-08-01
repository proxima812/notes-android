//! Error taxonomy for the whole core.
//!
//! Two rules hold everywhere below:
//!
//! 1. **Nothing internal crosses the bridge.** `AppError` carries a Russian
//!    sentence and a stable code; the underlying `rusqlite::Error`, io path or
//!    crypto failure is logged locally and dropped from the DTO. A user must
//!    never see a SQL statement, and note content must never reach a log.
//! 2. **Every variant is actionable or explanatory.** If the UI can do
//!    something about a failure (open an Android settings screen, free disk
//!    space, re-enter a password), the code says which.

use serde::Serialize;
use std::collections::BTreeMap;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database: {0}")]
    Database(#[from] DatabaseError),

    #[error("validation: {0}")]
    Validation(#[from] ValidationError),

    #[error("reminder: {0}")]
    Reminder(#[from] ReminderError),

    #[error("notification: {0}")]
    Notification(#[from] NotificationError),

    #[error("filesystem: {0}")]
    FileSystem(#[from] FileSystemError),

    #[error("encryption: {0}")]
    Encryption(#[from] EncryptionError),

    #[error("backup: {0}")]
    Backup(#[from] BackupError),

    #[error("import: {0}")]
    Import(#[from] ImportError),

    #[error("platform: {0}")]
    Platform(#[from] PlatformError),
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("failed to open the database")]
    Open(#[source] rusqlite::Error),

    #[error("migration {version} failed")]
    Migration {
        version: i64,
        #[source]
        source: rusqlite::Error,
    },

    #[error("schema is newer ({found}) than this build supports ({supported})")]
    SchemaTooNew { found: i64, supported: i64 },

    #[error("query failed")]
    Query(#[source] rusqlite::Error),

    #[error("{entity} {id} not found")]
    NotFound { entity: &'static str, id: String },

    #[error("{entity} {id} was modified by another change")]
    Conflict { entity: &'static str, id: String },

    #[error("the database is locked by another operation")]
    Busy,

    #[error("integrity check failed")]
    Corrupt,
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("field `{field}` is required")]
    Required { field: &'static str },

    #[error("field `{field}` is longer than {max} characters")]
    TooLong { field: &'static str, max: usize },

    #[error("field `{field}` has an unsupported value")]
    Invalid { field: &'static str },

    #[error("`{value}` is not a known time zone")]
    UnknownTimeZone { value: String },

    #[error("the scheduled time is in the past")]
    TimeInPast,

    #[error("making {child} a child of {parent} would create a cycle")]
    CyclicHierarchy { child: String, parent: String },
}

#[derive(Debug, Error)]
pub enum ReminderError {
    #[error("recurrence rule is not valid")]
    InvalidRecurrence { reason: String },

    #[error("the recurrence produced no future occurrence")]
    NoFutureOccurrence,

    #[error("could not parse the reminder text")]
    UnparsableText,

    #[error("the reminder has already ended")]
    Ended,
}

#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("exact alarms are not permitted")]
    ExactAlarmDenied,

    #[error("notifications are not permitted")]
    NotificationsDenied,

    #[error("the platform refused to schedule the alarm")]
    ScheduleFailed { reason: String },

    #[error("the notification channel could not be created")]
    ChannelUnavailable,
}

#[derive(Debug, Error)]
pub enum FileSystemError {
    #[error("file not found")]
    NotFound,

    #[error("not enough free space: {needed} bytes required")]
    OutOfSpace { needed: u64 },

    #[error("permission denied")]
    PermissionDenied,

    #[error("io failure")]
    Io(#[source] std::io::Error),

    #[error("the path escapes the application directory")]
    PathEscape,

    #[error("the file is larger than the {max} byte limit")]
    TooLarge { max: u64 },
}

#[derive(Debug, Error)]
pub enum EncryptionError {
    #[error("the password is wrong or the data is damaged")]
    DecryptionFailed,

    #[error("key derivation failed")]
    KeyDerivation,

    #[error("the key material is unavailable")]
    KeyUnavailable,

    #[error("unsupported encryption format version {version}")]
    UnsupportedFormat { version: u32 },
}

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("the archive is damaged")]
    Corrupt,

    #[error("the checksum does not match")]
    ChecksumMismatch,

    #[error("the archive is missing `{entry}`")]
    MissingEntry { entry: String },

    #[error("the archive was produced by a newer version ({found})")]
    UnsupportedVersion { found: u32 },

    #[error("the archive could not be written")]
    WriteFailed,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("the file format is not supported")]
    UnsupportedFormat,

    #[error("the file could not be parsed at line {line}")]
    Malformed { line: usize },

    #[error("{count} identifiers already exist")]
    IdConflict { count: usize },

    #[error("the file contains no importable entries")]
    Empty,
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("this capability is not available on the current platform")]
    Unsupported { capability: &'static str },

    #[error("the Android plugin call failed")]
    PluginCall { reason: String },

    #[error("the application data directory is unavailable")]
    DataDirUnavailable,
}

// ------------------------------------------------------------------- DTO --

/// Wire representation of [`AppError`]. Matches `AppErrorDto` in TypeScript.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub kind: &'static str,
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<BTreeMap<String, String>>,
}

fn details(pairs: &[(&str, String)]) -> Option<BTreeMap<String, String>> {
    if pairs.is_empty() {
        return None;
    }
    Some(
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    )
}

impl AppError {
    /// Broad category, used by the UI to pick an icon and a recovery hint.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Database(DatabaseError::NotFound { .. }) => "not_found",
            Self::Database(DatabaseError::Conflict { .. }) => "conflict",
            Self::Database(_) => "database",
            Self::Validation(_) => "validation",
            Self::Reminder(_) => "reminder",
            Self::Notification(_) => "notification",
            Self::FileSystem(_) => "file_system",
            Self::Encryption(_) => "encryption",
            Self::Backup(_) => "backup",
            Self::Import(_) => "import",
            Self::Platform(_) => "platform",
        }
    }

    /// Stable identifier the UI may branch on. Never changes for a given case.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(error) => match error {
                DatabaseError::Open(_) => "database_open_failed",
                DatabaseError::Migration { .. } => "database_migration_failed",
                DatabaseError::SchemaTooNew { .. } => "database_schema_too_new",
                DatabaseError::Query(_) => "database_query_failed",
                DatabaseError::NotFound { .. } => "not_found",
                DatabaseError::Conflict { .. } => "conflict",
                DatabaseError::Busy => "database_busy",
                DatabaseError::Corrupt => "database_corrupt",
            },
            Self::Validation(error) => match error {
                ValidationError::Required { .. } => "validation_required",
                ValidationError::TooLong { .. } => "validation_too_long",
                ValidationError::Invalid { .. } => "validation_invalid",
                ValidationError::UnknownTimeZone { .. } => "validation_unknown_timezone",
                ValidationError::TimeInPast => "validation_time_in_past",
                ValidationError::CyclicHierarchy { .. } => "validation_cyclic_hierarchy",
            },
            Self::Reminder(error) => match error {
                ReminderError::InvalidRecurrence { .. } => "reminder_invalid_recurrence",
                ReminderError::NoFutureOccurrence => "reminder_no_future_occurrence",
                ReminderError::UnparsableText => "reminder_unparsable_text",
                ReminderError::Ended => "reminder_ended",
            },
            Self::Notification(error) => match error {
                NotificationError::ExactAlarmDenied => "exact_alarm_permission_denied",
                NotificationError::NotificationsDenied => "notification_permission_denied",
                NotificationError::ScheduleFailed { .. } => "notification_schedule_failed",
                NotificationError::ChannelUnavailable => "notification_channel_unavailable",
            },
            Self::FileSystem(error) => match error {
                FileSystemError::NotFound => "file_not_found",
                FileSystemError::OutOfSpace { .. } => "file_out_of_space",
                FileSystemError::PermissionDenied => "storage_permission_denied",
                FileSystemError::Io(_) => "file_io_failed",
                FileSystemError::PathEscape => "file_path_escape",
                FileSystemError::TooLarge { .. } => "file_too_large",
            },
            Self::Encryption(error) => match error {
                EncryptionError::DecryptionFailed => "encryption_decryption_failed",
                EncryptionError::KeyDerivation => "encryption_key_derivation_failed",
                EncryptionError::KeyUnavailable => "encryption_key_unavailable",
                EncryptionError::UnsupportedFormat { .. } => "encryption_unsupported_format",
            },
            Self::Backup(error) => match error {
                BackupError::Corrupt => "backup_corrupt",
                BackupError::ChecksumMismatch => "backup_checksum_mismatch",
                BackupError::MissingEntry { .. } => "backup_missing_entry",
                BackupError::UnsupportedVersion { .. } => "backup_unsupported_version",
                BackupError::WriteFailed => "backup_write_failed",
            },
            Self::Import(error) => match error {
                ImportError::UnsupportedFormat => "import_unsupported_format",
                ImportError::Malformed { .. } => "import_malformed",
                ImportError::IdConflict { .. } => "import_id_conflict",
                ImportError::Empty => "import_empty",
            },
            Self::Platform(error) => match error {
                PlatformError::Unsupported { .. } => "platform_unsupported",
                PlatformError::PluginCall { .. } => "platform_plugin_call_failed",
                PlatformError::DataDirUnavailable => "platform_data_dir_unavailable",
            },
        }
    }

    /// Finished Russian sentence for the user. Deliberately free of file paths,
    /// SQL and identifiers that would mean nothing outside the code.
    pub fn user_message(&self) -> String {
        match self {
            Self::Database(error) => match error {
                DatabaseError::Open(_) => "Не удалось открыть базу данных.".to_owned(),
                DatabaseError::Migration { .. } => {
                    "Не удалось обновить структуру базы данных.".to_owned()
                }
                DatabaseError::SchemaTooNew { .. } => {
                    "База создана более новой версией приложения. Обновите приложение.".to_owned()
                }
                DatabaseError::Query(_) => "Не удалось выполнить операцию с данными.".to_owned(),
                DatabaseError::NotFound { .. } => "Запись больше недоступна.".to_owned(),
                DatabaseError::Conflict { .. } => {
                    "Запись была изменена. Обновите экран и повторите.".to_owned()
                }
                DatabaseError::Busy => "База занята. Повторите через мгновение.".to_owned(),
                DatabaseError::Corrupt => {
                    "База данных повреждена. Восстановите из резервной копии.".to_owned()
                }
            },
            Self::Validation(error) => match error {
                ValidationError::Required { .. } => "Заполните обязательное поле.".to_owned(),
                ValidationError::TooLong { max, .. } => {
                    format!("Значение слишком длинное, максимум {max} символов.")
                }
                ValidationError::Invalid { .. } => "Недопустимое значение поля.".to_owned(),
                ValidationError::UnknownTimeZone { .. } => "Неизвестный часовой пояс.".to_owned(),
                ValidationError::TimeInPast => "Указанное время уже прошло.".to_owned(),
                ValidationError::CyclicHierarchy { .. } => {
                    "Нельзя переместить элемент внутрь самого себя.".to_owned()
                }
            },
            Self::Reminder(error) => match error {
                ReminderError::InvalidRecurrence { .. } => {
                    "Правило повторения некорректно.".to_owned()
                }
                ReminderError::NoFutureOccurrence => {
                    "У этого повторения нет будущих срабатываний.".to_owned()
                }
                ReminderError::UnparsableText => {
                    "Не удалось распознать дату и время. Укажите их вручную.".to_owned()
                }
                ReminderError::Ended => "Напоминание уже завершено.".to_owned(),
            },
            Self::Notification(error) => match error {
                NotificationError::ExactAlarmDenied => {
                    "Разрешите точные напоминания в настройках Android.".to_owned()
                }
                NotificationError::NotificationsDenied => {
                    "Разрешите уведомления в настройках Android.".to_owned()
                }
                NotificationError::ScheduleFailed { .. } => {
                    "Не удалось создать точное напоминание.".to_owned()
                }
                NotificationError::ChannelUnavailable => {
                    "Не удалось настроить канал уведомлений.".to_owned()
                }
            },
            Self::FileSystem(error) => match error {
                FileSystemError::NotFound => "Файл больше недоступен.".to_owned(),
                FileSystemError::OutOfSpace { .. } => "Недостаточно свободного места.".to_owned(),
                FileSystemError::PermissionDenied => "Нет доступа к файлу.".to_owned(),
                FileSystemError::Io(_) => "Не удалось прочитать или записать файл.".to_owned(),
                FileSystemError::PathEscape => "Недопустимый путь к файлу.".to_owned(),
                FileSystemError::TooLarge { .. } => "Файл слишком большой.".to_owned(),
            },
            Self::Encryption(error) => match error {
                EncryptionError::DecryptionFailed => {
                    "Неверный пароль или файл повреждён.".to_owned()
                }
                EncryptionError::KeyDerivation => "Не удалось подготовить ключ.".to_owned(),
                EncryptionError::KeyUnavailable => {
                    "Ключ шифрования недоступен на этом устройстве.".to_owned()
                }
                EncryptionError::UnsupportedFormat { .. } => {
                    "Формат шифрования не поддерживается этой версией.".to_owned()
                }
            },
            Self::Backup(error) => match error {
                BackupError::Corrupt => "Резервная копия повреждена.".to_owned(),
                BackupError::ChecksumMismatch => {
                    "Контрольная сумма резервной копии не совпадает.".to_owned()
                }
                BackupError::MissingEntry { .. } => {
                    "В резервной копии не хватает данных.".to_owned()
                }
                BackupError::UnsupportedVersion { .. } => {
                    "Резервная копия создана более новой версией приложения.".to_owned()
                }
                BackupError::WriteFailed => "Не удалось записать резервную копию.".to_owned(),
            },
            Self::Import(error) => match error {
                ImportError::UnsupportedFormat => "Формат файла не поддерживается.".to_owned(),
                ImportError::Malformed { line } => {
                    format!("Файл повреждён: ошибка в строке {line}.")
                }
                ImportError::IdConflict { count } => {
                    format!("{count} записей уже существуют. Выберите способ разрешения.")
                }
                ImportError::Empty => "В файле нет данных для импорта.".to_owned(),
            },
            Self::Platform(error) => match error {
                PlatformError::Unsupported { .. } => {
                    "Эта возможность недоступна на текущем устройстве.".to_owned()
                }
                PlatformError::PluginCall { .. } => "Системный компонент не ответил.".to_owned(),
                PlatformError::DataDirUnavailable => "Хранилище приложения недоступно.".to_owned(),
            },
        }
    }

    /// Small structured facts the UI may need. Never includes user content.
    fn detail_pairs(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Validation(ValidationError::Required { field })
            | Self::Validation(ValidationError::Invalid { field }) => {
                vec![("field", (*field).to_owned())]
            }
            Self::Validation(ValidationError::TooLong { field, max }) => {
                vec![("field", (*field).to_owned()), ("max", max.to_string())]
            }
            Self::FileSystem(FileSystemError::OutOfSpace { needed }) => {
                vec![("needed", needed.to_string())]
            }
            Self::FileSystem(FileSystemError::TooLarge { max }) => {
                vec![("max", max.to_string())]
            }
            Self::Import(ImportError::IdConflict { count }) => {
                vec![("count", count.to_string())]
            }
            Self::Database(DatabaseError::SchemaTooNew { found, supported }) => vec![
                ("found", found.to_string()),
                ("supported", supported.to_string()),
            ],
            _ => Vec::new(),
        }
    }

    pub fn to_dto(&self) -> AppErrorDto {
        AppErrorDto {
            kind: self.kind(),
            code: self.code(),
            message: self.user_message(),
            details: details(&self.detail_pairs()),
        }
    }
}

/// `rusqlite` failures are mapped here so that call sites can use `?` without
/// leaking the driver type into the domain layer.
impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        match error {
            rusqlite::Error::SqliteFailure(inner, _)
                if inner.code == rusqlite::ErrorCode::DatabaseBusy
                    || inner.code == rusqlite::ErrorCode::DatabaseLocked =>
            {
                Self::Database(DatabaseError::Busy)
            }
            rusqlite::Error::SqliteFailure(inner, _)
                if inner.code == rusqlite::ErrorCode::DatabaseCorrupt =>
            {
                Self::Database(DatabaseError::Corrupt)
            }
            other => Self::Database(DatabaseError::Query(other)),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => Self::FileSystem(FileSystemError::NotFound),
            std::io::ErrorKind::PermissionDenied => {
                Self::FileSystem(FileSystemError::PermissionDenied)
            }
            _ => Self::FileSystem(FileSystemError::Io(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_is_reported_as_its_own_kind() {
        let error = AppError::Database(DatabaseError::NotFound {
            entity: "note",
            id: "abc".to_owned(),
        });
        assert_eq!(error.kind(), "not_found");
        assert_eq!(error.code(), "not_found");
    }

    #[test]
    fn exact_alarm_denial_tells_the_user_what_to_do() {
        let error = AppError::Notification(NotificationError::ExactAlarmDenied);
        assert_eq!(error.code(), "exact_alarm_permission_denied");
        assert!(error.user_message().contains("настройках Android"));
    }

    #[test]
    fn the_dto_never_carries_the_internal_message() {
        // `Display` for the internal error mentions the driver; the DTO must not.
        let error = AppError::Database(DatabaseError::Open(rusqlite::Error::InvalidQuery));
        let dto = error.to_dto();
        assert_eq!(dto.message, "Не удалось открыть базу данных.");
        assert!(!dto.message.contains("rusqlite"));
        assert!(dto.details.is_none());
    }

    #[test]
    fn validation_details_name_the_offending_field() {
        let error = AppError::Validation(ValidationError::TooLong {
            field: "title",
            max: 200,
        });
        let dto = error.to_dto();
        let details = dto.details.expect("details are present");
        assert_eq!(details.get("field").map(String::as_str), Some("title"));
        assert_eq!(details.get("max").map(String::as_str), Some("200"));
        assert!(dto.message.contains("200"));
    }

    #[test]
    fn a_busy_database_is_distinguished_from_a_query_failure() {
        let busy = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseBusy,
                extended_code: 5,
            },
            None,
        );
        assert_eq!(AppError::from(busy).code(), "database_busy");
    }

    #[test]
    fn a_missing_file_maps_to_a_user_facing_message() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let error = AppError::from(io);
        assert_eq!(error.code(), "file_not_found");
        assert_eq!(error.user_message(), "Файл больше недоступен.");
    }
}
