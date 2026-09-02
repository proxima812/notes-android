//! The one place the app talks to the internet.
//!
//! Everything about that conversation is bounded on purpose: a short timeout, a
//! cap on how much of a page is read, a refusal to follow more than a couple of
//! redirects, and no cookies, no compression negotiation beyond what `reqwest`
//! does by itself, and nothing sent that would identify the person. A note is
//! private; reading the title of a link should not make it less so.

use std::io::Read as _;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::domain::links::{FetchedIcon, FetchedPage, LinkReader};
use crate::error::{AppError, AppResult, PlatformError};

/// Long enough for a slow mobile connection, short enough that a dead host does
/// not hold a worker thread while somebody keeps typing.
const TIMEOUT: Duration = Duration::from_secs(8);

/// How much of a page is read before the rest is dropped. The title and the
/// icon are in the `<head>`; a megabyte of it would be a page doing something
/// unusual, and reading further would not find them.
const HTML_LIMIT: u64 = 512 * 1024;

/// Same idea for the icon itself. The use case caps it again, so this is only
/// about not pulling a large file down the wire in the first place.
const ICON_LIMIT: u64 = 512 * 1024;

/// A browser's, deliberately. A site that serves a different page to unknown
/// clients would otherwise give a title nobody can recognise — and this string
/// says nothing about the device it came from.
const USER_AGENT: &str =
    "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120 Mobile Safari/537.36";

fn failed(reason: impl Into<String>) -> AppError {
    AppError::Platform(PlatformError::Network {
        reason: reason.into(),
    })
}

pub struct HttpLinkReader {
    client: OnceLock<reqwest::blocking::Client>,
}

impl Default for HttpLinkReader {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpLinkReader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: OnceLock::new(),
        }
    }

    /// The trust roots, and the reason they are spelled out here.
    ///
    /// Left to itself `reqwest` verifies certificates through the *platform*
    /// verifier, which on Android is a JNI call into the system trust store —
    /// and it panics unless the app has handed it a `Context` at startup. This
    /// app never does, so the first `https://` request would take the whole
    /// process down with it. Compiled-in roots need no JNI, no Java, and no
    /// startup ceremony, at the cost of shipping the root list in the binary
    /// and updating it with the app.
    fn tls() -> AppResult<rustls::ClientConfig> {
        let roots = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };

        rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|error| failed(error.to_string()))
        .map(|builder| builder.with_root_certificates(roots).with_no_client_auth())
    }

    /// Built on first use rather than at startup: an app opened to read the
    /// library must not pay for a TLS stack it may never ask a question of.
    fn client(&self) -> AppResult<&reqwest::blocking::Client> {
        if let Some(client) = self.client.get() {
            return Ok(client);
        }

        let client = reqwest::blocking::Client::builder()
            .use_preconfigured_tls(Self::tls()?)
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            .connect_timeout(TIMEOUT)
            // Two hops covers http→https and a trailing-slash redirect. More
            // than that is a chain the app has no reason to follow.
            .redirect(reqwest::redirect::Policy::limited(2))
            .build()
            .map_err(|error| failed(error.to_string()))?;

        Ok(self.client.get_or_init(|| client))
    }

    /// Reads at most `limit` bytes of a response body.
    fn body(response: reqwest::blocking::Response, limit: u64) -> AppResult<Vec<u8>> {
        let mut bytes = Vec::new();
        response
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(|error| failed(error.to_string()))?;
        Ok(bytes)
    }
}

impl LinkReader for HttpLinkReader {
    fn page(&self, url: &str) -> AppResult<FetchedPage> {
        let response = self
            .client()?
            .get(url)
            .header("accept", "text/html,application/xhtml+xml")
            // Titles are what this is for, and a page that has a Russian one
            // usually only says so when asked.
            .header("accept-language", "ru,en;q=0.8")
            .send()
            .map_err(|error| failed(error.to_string()))?;

        if !response.status().is_success() {
            return Err(failed(format!("status {}", response.status().as_u16())));
        }

        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !content_type.is_empty() && !content_type.contains("html") {
            return Err(failed("not a page"));
        }

        // Decoded by the charset the response declares, so a page still served
        // as windows-1251 — and Russian sites still are — reads as words rather
        // than as mojibake.
        let charset = content_type
            .split("charset=")
            .nth(1)
            .map(|rest| rest.trim_matches(['"', ' ', ';']).to_owned());
        let bytes = Self::body(response, HTML_LIMIT)?;
        let html = charset
            .as_deref()
            .and_then(|label| encoding_rs::Encoding::for_label_no_replacement(label.as_bytes()))
            .map_or_else(
                || String::from_utf8_lossy(&bytes).into_owned(),
                |encoding| encoding.decode(&bytes).0.into_owned(),
            );

        Ok(FetchedPage { final_url, html })
    }

    fn icon(&self, url: &str) -> AppResult<FetchedIcon> {
        let response = self
            .client()?
            .get(url)
            .header("accept", "image/*")
            .send()
            .map_err(|error| failed(error.to_string()))?;

        if !response.status().is_success() {
            return Err(failed(format!("status {}", response.status().as_u16())));
        }

        // A site that answers a missing icon with its own HTML 404 page is
        // common enough that trusting the status alone would put a page of
        // markup into an `<img>`.
        let mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or(value).trim().to_owned())
            .unwrap_or_else(|| "image/x-icon".to_owned());
        if !mime.starts_with("image/") {
            return Err(failed("not an image"));
        }

        Ok(FetchedIcon {
            mime,
            bytes: Self::body(response, ICON_LIMIT)?,
        })
    }
}

/// The reader the app runs with.
#[must_use]
pub fn shared() -> Arc<dyn LinkReader> {
    Arc::new(HttpLinkReader::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ignored by default: it needs the internet, and the suite must pass on a
    /// machine that has none. Run it by hand after touching anything about TLS
    /// — it is the only thing that proves the compiled-in roots verify a real
    /// certificate, which is exactly what the platform verifier failed to do.
    #[test]
    #[ignore = "needs the network"]
    fn a_real_site_gives_up_its_title() {
        let reader = HttpLinkReader::new();
        let page = reader.page("https://example.com/").expect("reads the page");
        assert!(crate::domain::links::html::title_of(&page.html).is_some());
    }
}
