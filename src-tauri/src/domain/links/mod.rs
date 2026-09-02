//! What a pasted address turns out to be.
//!
//! This is the one part of the app that leaves the device. The rule it keeps is
//! narrow: only the address the user pasted is contacted, and only for its own
//! title and its own icon. Nothing is sent anywhere else, and no third-party
//! favicon service is asked — that would hand every domain in someone's notes
//! to a company that was never part of the deal.

pub mod html;

use crate::error::AppResult;

/// A page as the app remembers it.
///
/// `title` and `icon` are independently optional: plenty of pages have one and
/// not the other, and a missing icon is no reason to throw away a good title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPreview {
    /// The address this was looked up under — the normalised form, not
    /// necessarily the text the user typed.
    pub url: String,
    pub title: Option<String>,
    /// A `data:` URL, so the icon survives being offline and needs no second
    /// store on disk beside the database.
    pub icon: Option<String>,
    /// Whether the last attempt reached the site at all. A row with `ok = false`
    /// is what stops a host that is down from being asked on every keystroke.
    pub ok: bool,
    pub fetched_at: i64,
}

impl LinkPreview {
    /// A preview that says nothing, for an address that could not be read.
    #[must_use]
    pub fn unreachable(url: String, fetched_at: i64) -> Self {
        Self {
            url,
            title: None,
            icon: None,
            ok: false,
            fetched_at,
        }
    }

    /// Whether this is worth showing. A row that reached the site and found
    /// neither a title nor an icon is a real answer, but not a useful one.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.icon.is_none()
    }
}

pub trait LinkPreviewRepository: Send + Sync {
    /// # Errors
    /// Fails on a database error.
    fn read(&self, url: &str) -> AppResult<Option<LinkPreview>>;

    /// # Errors
    /// Fails on a database error.
    fn write(&self, preview: &LinkPreview) -> AppResult<()>;

    /// Drops every remembered page. Offered because a cache of the sites
    /// someone has pasted is a record of what they read, and they are entitled
    /// to end it without deleting the notes.
    ///
    /// # Errors
    /// Fails on a database error.
    fn clear(&self) -> AppResult<()>;
}

/// A page, fetched.
pub struct FetchedPage {
    /// Where the page came from after redirects. Relative icon addresses are
    /// resolved against this rather than against what was asked for.
    pub final_url: String,
    pub html: String,
}

/// An icon, fetched.
pub struct FetchedIcon {
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// The network, as the use case needs it.
///
/// A port rather than a direct call so the fetching rules can be tested without
/// a network, and so the one place that talks to the outside world is a single
/// named implementation somebody can read end to end.
pub trait LinkReader: Send + Sync {
    /// # Errors
    /// Fails when the address cannot be reached, answers with an error status,
    /// or does not return HTML.
    fn page(&self, url: &str) -> AppResult<FetchedPage>;

    /// # Errors
    /// Fails when the icon cannot be reached or is not an image.
    fn icon(&self, url: &str) -> AppResult<FetchedIcon>;
}
