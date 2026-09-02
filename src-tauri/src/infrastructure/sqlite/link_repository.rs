//! The remembered-pages table.

use std::sync::Arc;

use rusqlite::{params, OptionalExtension as _};

use crate::domain::links::{LinkPreview, LinkPreviewRepository};
use crate::error::{AppError, AppResult};

use super::Database;

pub struct SqliteLinkPreviewRepository {
    database: Arc<Database>,
}

impl SqliteLinkPreviewRepository {
    #[must_use]
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }
}

impl LinkPreviewRepository for SqliteLinkPreviewRepository {
    fn read(&self, url: &str) -> AppResult<Option<LinkPreview>> {
        self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT url, title, icon, ok, fetched_at
                       FROM link_previews
                      WHERE url = ?1",
                    [url],
                    |row| {
                        Ok(LinkPreview {
                            url: row.get(0)?,
                            title: row.get(1)?,
                            icon: row.get(2)?,
                            ok: row.get::<_, i64>(3)? == 1,
                            fetched_at: row.get(4)?,
                        })
                    },
                )
                .optional()
                .map_err(AppError::from)
        })
    }

    fn write(&self, preview: &LinkPreview) -> AppResult<()> {
        self.database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO link_previews (url, title, icon, ok, fetched_at)
                          VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT (url) DO UPDATE
                            SET title = excluded.title,
                                icon = excluded.icon,
                                ok = excluded.ok,
                                fetched_at = excluded.fetched_at",
                    params![
                        preview.url,
                        preview.title,
                        preview.icon,
                        i64::from(preview.ok),
                        preview.fetched_at,
                    ],
                )
                .map(|_| ())
                .map_err(AppError::from)
        })
    }

    fn clear(&self) -> AppResult<()> {
        self.database.with_connection(|connection| {
            connection
                .execute("DELETE FROM link_previews", [])
                .map(|_| ())
                .map_err(AppError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> SqliteLinkPreviewRepository {
        let database = Arc::new(Database::open_in_memory(0).expect("opens"));
        SqliteLinkPreviewRepository::new(database)
    }

    fn preview() -> LinkPreview {
        LinkPreview {
            url: "https://workos.com/".to_owned(),
            title: Some("WorkOS".to_owned()),
            icon: Some("data:image/png;base64,AAA".to_owned()),
            ok: true,
            fetched_at: 1_000,
        }
    }

    #[test]
    fn an_address_that_was_never_read_is_absent() {
        assert_eq!(fixture().read("https://example.com/").expect("reads"), None);
    }

    #[test]
    fn what_was_written_comes_back_whole() {
        let repository = fixture();
        repository.write(&preview()).expect("writes");
        assert_eq!(
            repository.read("https://workos.com/").expect("reads"),
            Some(preview())
        );
    }

    /// Re-reading a page must replace what was known about it rather than fail:
    /// sites are renamed, and the cache has to be able to follow.
    #[test]
    fn reading_the_same_address_again_replaces_the_row() {
        let repository = fixture();
        repository.write(&preview()).expect("writes");
        let renamed = LinkPreview {
            title: Some("WorkOS — Enterprise Ready".to_owned()),
            fetched_at: 2_000,
            ..preview()
        };
        repository.write(&renamed).expect("writes again");

        assert_eq!(
            repository.read("https://workos.com/").expect("reads"),
            Some(renamed)
        );
    }

    #[test]
    fn a_failed_read_is_remembered_as_a_failure() {
        let repository = fixture();
        repository
            .write(&LinkPreview::unreachable(
                "https://offline.example/".to_owned(),
                5_000,
            ))
            .expect("writes");

        let found = repository
            .read("https://offline.example/")
            .expect("reads")
            .expect("is there");
        assert!(!found.ok);
        assert!(found.is_empty());
    }

    #[test]
    fn clearing_leaves_nothing_behind() {
        let repository = fixture();
        repository.write(&preview()).expect("writes");
        repository.clear().expect("clears");
        assert_eq!(repository.read("https://workos.com/").expect("reads"), None);
    }
}
