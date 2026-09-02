//! Turning a pasted address into a title and an icon.
//!
//! The rules about *when* the network is touched live here rather than in the
//! reader: a cache that is consulted first, a short memory for failures, and a
//! long one for answers. The reader itself only knows how to fetch.

use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use url::Url;

use crate::domain::clock::SharedClock;
use crate::domain::links::{html, FetchedPage, LinkPreview, LinkPreviewRepository, LinkReader};
use crate::error::AppResult;

/// How long an answer is trusted. Titles change, but rarely, and a month keeps
/// an old note from re-reading its whole address book every time it is opened.
const FRESH_MILLIS: i64 = 30 * 24 * 60 * 60 * 1_000;

/// How long a failure is remembered. Short, because the usual cause is that the
/// phone had no signal at that moment, and that is over long before a month.
const RETRY_MILLIS: i64 = 6 * 60 * 60 * 1_000;

/// Largest icon kept, before base64. Anything above this is a picture rather
/// than an icon, and every byte here lands in the database and in the backup.
const ICON_LIMIT: usize = 128 * 1024;

pub struct LinkUseCases {
    previews: Arc<dyn LinkPreviewRepository>,
    reader: Arc<dyn LinkReader>,
    clock: SharedClock,
}

/// The address as the cache knows it: no fragment, lowercase host, and http
/// only. `None` for anything the app must not fetch — a `mailto:`, a `tel:`, or
/// text that is not an address at all.
///
/// Dropping the fragment is what makes two links to different parts of one page
/// share a row; keeping the query is what stops two different pages from doing
/// so.
#[must_use]
fn normalise(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.host()?;
    url.set_fragment(None);
    Some(url.to_string())
}

impl LinkUseCases {
    #[must_use]
    pub fn new(
        previews: Arc<dyn LinkPreviewRepository>,
        reader: Arc<dyn LinkReader>,
        clock: SharedClock,
    ) -> Self {
        Self {
            previews,
            reader,
            clock,
        }
    }

    /// What is known about an address, reading it if what is known is old.
    ///
    /// `None` means the address is not one the app fetches at all, which the
    /// screen shows as the plain link it already had.
    ///
    /// # Errors
    /// Fails only on a database error. A site that cannot be reached is an
    /// answer, not an error: it is cached as a failure and reported as a
    /// preview with nothing in it.
    pub fn preview(&self, raw: &str) -> AppResult<Option<LinkPreview>> {
        let Some(url) = normalise(raw) else {
            return Ok(None);
        };
        let now = self.clock.now().as_millis();

        let known = self.previews.read(&url)?;
        if let Some(known) = known.as_ref() {
            let age = now.saturating_sub(known.fetched_at);
            let limit = if known.ok && !known.is_empty() {
                FRESH_MILLIS
            } else {
                RETRY_MILLIS
            };
            if age < limit {
                return Ok(Some(known.clone()));
            }
        }

        let fresh = match self.read(url, now) {
            // Losing signal must not lose a title that was already known: the
            // old answer is kept and shown, marked stale so it is tried again
            // in hours rather than in a month.
            failed if !failed.ok => match known {
                Some(old) if old.ok && !old.is_empty() => LinkPreview {
                    ok: false,
                    fetched_at: now,
                    ..old
                },
                _ => failed,
            },
            read => read,
        };
        self.previews.write(&fresh)?;
        Ok(Some(fresh))
    }

    /// Forgets every page. See [`LinkPreviewRepository::clear`].
    ///
    /// # Errors
    /// Fails on a database error.
    pub fn forget_all(&self) -> AppResult<()> {
        self.previews.clear()
    }

    /// One trip to the network. Never returns an error: whatever went wrong,
    /// the answer to show is "nothing about this page".
    fn read(&self, url: String, now: i64) -> LinkPreview {
        let Ok(page) = self.reader.page(&url) else {
            tracing::debug!("a link preview could not be read");
            return LinkPreview::unreachable(url, now);
        };

        let title = html::title_of(&page.html);
        let icon = self.read_icon(&page);

        LinkPreview {
            url,
            title,
            icon,
            ok: true,
            fetched_at: now,
        }
    }

    /// The icon the page declares, or the one at `/favicon.ico`, as a data URL.
    ///
    /// The fallback is worth the second request: a great many sites still ship
    /// an icon at the well-known path and say nothing about it in their `head`.
    fn read_icon(&self, page: &FetchedPage) -> Option<String> {
        let base = Url::parse(&page.final_url).ok()?;
        let declared = html::icon_href_of(&page.html).and_then(|href| base.join(&href).ok());
        let fallback = base.join("/favicon.ico").ok();

        for candidate in [declared, fallback].into_iter().flatten() {
            let Ok(icon) = self.reader.icon(candidate.as_str()) else {
                continue;
            };
            if icon.bytes.is_empty() || icon.bytes.len() > ICON_LIMIT {
                continue;
            }
            return Some(format!(
                "data:{};base64,{}",
                icon.mime,
                STANDARD.encode(&icon.bytes)
            ));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::domain::clock::{FixedClock, Timestamp};
    use crate::domain::links::FetchedIcon;
    use crate::error::{AppError, PlatformError};
    use crate::infrastructure::sqlite::{Database, SqliteLinkPreviewRepository};

    /// A network that answers from a script, and counts how often it was asked.
    struct FakeReader {
        html: Option<&'static str>,
        icon: Option<&'static [u8]>,
        pages: AtomicUsize,
        icons: AtomicUsize,
    }

    impl FakeReader {
        fn new(html: Option<&'static str>, icon: Option<&'static [u8]>) -> Arc<Self> {
            Arc::new(Self {
                html,
                icon,
                pages: AtomicUsize::new(0),
                icons: AtomicUsize::new(0),
            })
        }
    }

    fn offline() -> AppError {
        AppError::Platform(PlatformError::PluginCall {
            reason: "no network".to_owned(),
        })
    }

    impl LinkReader for FakeReader {
        fn page(&self, url: &str) -> AppResult<FetchedPage> {
            self.pages.fetch_add(1, Ordering::Relaxed);
            self.html.map_or_else(
                || Err(offline()),
                |html| {
                    Ok(FetchedPage {
                        final_url: url.to_owned(),
                        html: html.to_owned(),
                    })
                },
            )
        }

        fn icon(&self, _url: &str) -> AppResult<FetchedIcon> {
            self.icons.fetch_add(1, Ordering::Relaxed);
            self.icon.map_or_else(
                || Err(offline()),
                |bytes| {
                    Ok(FetchedIcon {
                        mime: "image/png".to_owned(),
                        bytes: bytes.to_vec(),
                    })
                },
            )
        }
    }

    fn use_cases(reader: Arc<FakeReader>, at: i64) -> LinkUseCases {
        let database = Arc::new(Database::open_in_memory(0).expect("opens"));
        LinkUseCases::new(
            Arc::new(SqliteLinkPreviewRepository::new(database)),
            reader,
            Arc::new(FixedClock::new(Timestamp::from_millis(at))),
        )
    }

    const PAGE: &str = r#"<head><title>WorkOS — Your app, Enterprise Ready.</title>
                          <link rel="icon" href="/icon.png"></head>"#;

    #[test]
    fn a_page_gives_up_its_title_and_its_icon() {
        let links = use_cases(FakeReader::new(Some(PAGE), Some(b"png-bytes")), 1_000);

        let preview = links
            .preview("https://workos.com/pricing")
            .expect("no database error")
            .expect("an http address is fetched");

        assert_eq!(
            preview.title.as_deref(),
            Some("WorkOS — Your app, Enterprise Ready.")
        );
        assert_eq!(
            preview.icon.as_deref(),
            Some("data:image/png;base64,cG5nLWJ5dGVz")
        );
        assert!(preview.ok);
    }

    /// The whole point of the cache: a note full of links, opened twice, must
    /// not be a second trip to every one of those sites.
    #[test]
    fn a_second_look_at_the_same_address_asks_nobody() {
        let reader = FakeReader::new(Some(PAGE), Some(b"png-bytes"));
        let links = use_cases(Arc::clone(&reader), 1_000);

        links.preview("https://workos.com/").expect("reads");
        links.preview("https://workos.com/").expect("reads again");

        assert_eq!(reader.pages.load(Ordering::Relaxed), 1);
    }

    /// The fragment is which part of a page you were looking at, not which page
    /// it is, so both links share one row and one request.
    #[test]
    fn two_links_differing_only_by_fragment_are_one_page() {
        let reader = FakeReader::new(Some(PAGE), Some(b"png-bytes"));
        let links = use_cases(Arc::clone(&reader), 1_000);

        links.preview("https://workos.com/docs#sso").expect("reads");
        links
            .preview("https://workos.com/docs#scim")
            .expect("reads");

        assert_eq!(reader.pages.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_site_that_cannot_be_reached_is_an_empty_preview_rather_than_an_error() {
        let links = use_cases(FakeReader::new(None, None), 1_000);

        let preview = links
            .preview("https://offline.example/")
            .expect("not a database error")
            .expect("still an http address");

        assert!(!preview.ok);
        assert!(preview.is_empty());
    }

    /// A title with no icon beside it is most of the value, and must not be
    /// thrown away because the icon 404s.
    #[test]
    fn a_page_whose_icon_is_missing_keeps_its_title() {
        let links = use_cases(FakeReader::new(Some(PAGE), None), 1_000);

        let preview = links
            .preview("https://workos.com/")
            .expect("reads")
            .expect("fetched");

        assert!(preview.title.is_some());
        assert_eq!(preview.icon, None);
        assert!(preview.ok, "the page itself was read");
    }

    /// A site that declares no icon still usually has one at the old path.
    #[test]
    fn a_page_that_declares_no_icon_falls_back_to_the_well_known_path() {
        let reader = FakeReader::new(Some("<title>Plain</title>"), Some(b"ico"));
        let links = use_cases(Arc::clone(&reader), 1_000);

        let preview = links
            .preview("https://plain.example/deep/page")
            .expect("reads")
            .expect("fetched");

        assert!(preview.icon.is_some());
        assert_eq!(reader.icons.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn an_icon_too_large_to_be_an_icon_is_left_alone() {
        const HUGE: &[u8] = &[0; ICON_LIMIT + 1];
        let links = use_cases(FakeReader::new(Some(PAGE), Some(HUGE)), 1_000);

        let preview = links
            .preview("https://workos.com/")
            .expect("reads")
            .expect("fetched");

        assert_eq!(preview.icon, None);
    }

    /// Addresses the app has no business fetching are answered without a trip.
    #[test]
    fn only_http_addresses_are_ever_fetched() {
        let reader = FakeReader::new(Some(PAGE), Some(b"png"));
        let links = use_cases(Arc::clone(&reader), 1_000);

        for address in [
            "mailto:someone@example.com",
            "tel:+79990000000",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "not an address at all",
        ] {
            assert_eq!(links.preview(address).expect("no error"), None, "{address}");
        }
        assert_eq!(reader.pages.load(Ordering::Relaxed), 0);
    }

    /// Going through a tunnel is not a reason to forget what a link was called.
    #[test]
    fn a_title_already_known_survives_a_failed_re_read() {
        let database = Arc::new(Database::open_in_memory(0).expect("opens"));
        let previews = Arc::new(SqliteLinkPreviewRepository::new(Arc::clone(&database)));

        let online = LinkUseCases::new(
            Arc::clone(&previews) as Arc<dyn LinkPreviewRepository>,
            FakeReader::new(Some(PAGE), Some(b"png")),
            Arc::new(FixedClock::new(Timestamp::from_millis(1_000))),
        );
        online.preview("https://workos.com/").expect("reads");

        // A month later, with the network gone.
        let offline = LinkUseCases::new(
            previews as Arc<dyn LinkPreviewRepository>,
            FakeReader::new(None, None),
            Arc::new(FixedClock::new(Timestamp::from_millis(
                1_000 + FRESH_MILLIS + 1,
            ))),
        );
        let preview = offline
            .preview("https://workos.com/")
            .expect("reads")
            .expect("fetched");

        assert_eq!(
            preview.title.as_deref(),
            Some("WorkOS — Your app, Enterprise Ready.")
        );
        assert!(!preview.ok, "but it is known to be stale");
    }

    #[test]
    fn forgetting_empties_the_cache() {
        let reader = FakeReader::new(Some(PAGE), Some(b"png"));
        let links = use_cases(Arc::clone(&reader), 1_000);

        links.preview("https://workos.com/").expect("reads");
        links.forget_all().expect("forgets");
        links.preview("https://workos.com/").expect("reads again");

        assert_eq!(reader.pages.load(Ordering::Relaxed), 2);
    }
}
