//! Reading a title and an icon out of a page, without a parser.
//!
//! Everything here is a scan over the raw text rather than a DOM: the app needs
//! two facts out of the `<head>` of an arbitrary page, and a real HTML parser
//! is a large dependency for two facts. The scan is deliberately forgiving —
//! unquoted attributes, uppercase tags, and attributes in any order all read —
//! and every function answers `None` rather than guessing when the page does
//! not say.

/// Longest title kept. A page is free to put a paragraph in its `<title>`; a
/// line in a note is not the place for it.
const TITLE_LIMIT: usize = 120;

/// The icon size the editor draws at, doubled for a dense screen. Nothing is
/// resized to it; it is only what «closest» is measured against.
const WANTED_SIZE: u32 = 48;

/// Case-insensitive `find`, over ASCII only — every tag and attribute name this
/// module looks for is ASCII, and lowercasing the whole page to search it would
/// copy a document to find one word.
fn find_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    let (hay, pin) = (haystack.as_bytes(), needle.as_bytes());
    if pin.is_empty() || hay.len() < pin.len() {
        return None;
    }
    (0..=hay.len() - pin.len()).find(|&start| {
        hay[start..start + pin.len()]
            .iter()
            .zip(pin)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    })
}

/// The value of one attribute inside a single tag's text.
///
/// `tag` is what sits between `<` and `>`, so the search cannot wander into the
/// rest of the document.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let mut rest = tag;
    loop {
        let at = find_ascii_ci(rest, name)?;
        let before_is_boundary = rest[..at]
            .chars()
            .next_back()
            .is_none_or(|char| char.is_whitespace());
        let after = &rest[at + name.len()..];
        let value = after.trim_start();

        // `property` must not match when `name` is `name`, and `data-title`
        // must not match `title`, so both sides are checked for a boundary.
        if before_is_boundary && value.starts_with('=') {
            let value = value[1..].trim_start();
            let mut chars = value.chars();
            return match chars.next() {
                Some(quote @ ('"' | '\'')) => {
                    let inner = &value[1..];
                    inner.find(quote).map(|end| inner[..end].to_owned())
                }
                Some(_) => Some(
                    value
                        .split(|char: char| char.is_whitespace() || char == '>')
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                ),
                None => None,
            };
        }

        rest = &rest[at + name.len()..];
    }
}

/// Every `<name ...>` tag in the document, as the text between the brackets.
fn tags<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let opening = format!("<{name}");
    let mut found = Vec::new();
    let mut offset = 0;

    while let Some(at) = find_ascii_ci(&html[offset..], &opening) {
        let start = offset + at + opening.len();
        // `<linkedin>` is not `<link>`: the name has to end where the tag says.
        let ends_here = html[start..]
            .chars()
            .next()
            .is_some_and(|char| char.is_whitespace() || char == '>' || char == '/');
        let Some(close) = html[start..].find('>') else {
            break;
        };
        if ends_here {
            found.push(&html[start..start + close]);
        }
        offset = start + close;
    }

    found
}

/// Turns the handful of entities that actually appear in titles back into text.
///
/// Not a general decoder: a title is a line of prose, and the numeric forms
/// plus the five named ones cover what prose contains. Anything else is left
/// alone, which shows the entity rather than swallowing the sentence.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(end) = after.find(';').filter(|end| *end <= 10) else {
            out.push('&');
            rest = &after[1..];
            continue;
        };
        let entity = &after[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix('#')
                .and_then(|number| match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => number.parse().ok(),
                })
                .and_then(char::from_u32),
        };

        match decoded {
            Some(char) => {
                out.push(char);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }

    out.push_str(rest);
    out
}

/// One line, trimmed, with runs of whitespace collapsed and a length cap.
///
/// A `<title>` is often laid out across several indented lines in the source,
/// and that indentation is not part of what the page is called.
fn tidy(raw: &str) -> Option<String> {
    let mut title = String::new();
    for word in decode_entities(raw).split_whitespace() {
        if !title.is_empty() {
            title.push(' ');
        }
        title.push_str(word);
    }

    if title.is_empty() {
        return None;
    }
    if title.chars().count() > TITLE_LIMIT {
        title = title.chars().take(TITLE_LIMIT - 1).collect::<String>() + "…";
    }
    Some(title)
}

/// The `content` of the first `<meta>` whose `property` or `name` matches.
fn meta_content(html: &str, key: &str) -> Option<String> {
    tags(html, "meta")
        .into_iter()
        .find(|tag| {
            attribute(tag, "property")
                .or_else(|| attribute(tag, "name"))
                .is_some_and(|found| found.eq_ignore_ascii_case(key))
        })
        .and_then(|tag| attribute(tag, "content"))
}

/// What to call the page.
///
/// `og:title` first, because it is what a site writes for exactly this purpose
/// — a link shown somewhere else — while `<title>` also carries whatever the
/// site appends for search engines.
#[must_use]
pub fn title_of(html: &str) -> Option<String> {
    if let Some(title) = meta_content(html, "og:title").and_then(|raw| tidy(&raw)) {
        return Some(title);
    }

    let at = find_ascii_ci(html, "<title")?;
    let after = &html[at..];
    let open = after.find('>')? + 1;
    let close = find_ascii_ci(&after[open..], "</title>")?;
    tidy(&after[open..open + close])
}

/// How good an icon a `<link rel>` promises, or `None` when it promises none.
///
/// Bigger is better up to a point: a 16px favicon is blurry beside text, and a
/// 512px PWA icon is a large download for a 16px slot. Anything unlabelled
/// scores below a labelled size, because a site that bothered to say is usually
/// the one that has more than one.
fn icon_rank(tag: &str) -> Option<u32> {
    let rel = attribute(tag, "rel")?.to_ascii_lowercase();
    let is_icon = rel
        .split_whitespace()
        .any(|word| word == "icon" || word == "shortcut" || word == "apple-touch-icon");
    if !is_icon {
        return None;
    }

    let size = attribute(tag, "sizes")
        .and_then(|sizes| {
            sizes
                .split(['x', 'X', ' '])
                .filter_map(|part| part.parse::<u32>().ok())
                .max()
        })
        .unwrap_or(0);

    // An SVG has no size and needs none, so it wins outright: one file that is
    // sharp at every size is exactly what this slot wants.
    if attribute(tag, "href").is_some_and(|href| href.to_ascii_lowercase().contains(".svg")) {
        return Some(1_000);
    }
    // Closeness to the size the slot actually wants, so a 16px favicon and a
    // 512px PWA icon both lose to the 32px one between them. A tag that names
    // no size scores below every tag that does, but is still worth having.
    Some(if size == 0 {
        1
    } else {
        2.max(300_u32.saturating_sub(size.abs_diff(WANTED_SIZE)))
    })
}

/// The address of the best icon the page declares, exactly as written.
///
/// Relative — resolving it against the page's own address is the caller's job,
/// because only the caller knows where the page finally came from after
/// redirects.
#[must_use]
pub fn icon_href_of(html: &str) -> Option<String> {
    tags(html, "link")
        .into_iter()
        .filter_map(|tag| icon_rank(tag).map(|rank| (rank, tag)))
        .max_by_key(|(rank, _)| *rank)
        .and_then(|(_, tag)| attribute(tag, "href"))
        .filter(|href| !href.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_open_graph_title_is_preferred_over_the_document_one() {
        let html = r#"<head><meta property="og:title" content="WorkOS — Your app, Enterprise Ready.">
                      <title>WorkOS | Enterprise SSO, SCIM, Audit Logs</title></head>"#;
        assert_eq!(
            title_of(html).as_deref(),
            Some("WorkOS — Your app, Enterprise Ready.")
        );
    }

    #[test]
    fn a_page_without_open_graph_falls_back_to_the_title_tag() {
        let html = "<HTML><HEAD><TITLE>Clerk | Authentication and User Management</TITLE>";
        assert_eq!(
            title_of(html).as_deref(),
            Some("Clerk | Authentication and User Management")
        );
    }

    #[test]
    fn a_title_laid_out_over_several_lines_becomes_one() {
        let html = "<title>\n  Длинный\n  заголовок\n</title>";
        assert_eq!(title_of(html).as_deref(), Some("Длинный заголовок"));
    }

    #[test]
    fn entities_are_read_as_the_characters_they_stand_for() {
        let html = "<title>Tom &amp; Jerry &#8212; &#x41;&nbsp;list</title>";
        assert_eq!(title_of(html).as_deref(), Some("Tom & Jerry — A list"));
    }

    /// An entity nobody decodes must not eat the rest of the sentence.
    #[test]
    fn an_unknown_entity_is_left_as_written() {
        let html = "<title>a &zwnj; b</title>";
        assert_eq!(title_of(html).as_deref(), Some("a &zwnj; b"));
    }

    #[test]
    fn a_page_with_no_title_at_all_has_none() {
        assert_eq!(title_of("<head><meta charset=\"utf-8\"></head>"), None);
        assert_eq!(title_of("<title>   </title>"), None);
    }

    #[test]
    fn a_very_long_title_is_cut_rather_than_stored_whole() {
        let html = format!("<title>{}</title>", "я".repeat(400));
        let title = title_of(&html).expect("has a title");
        assert_eq!(title.chars().count(), TITLE_LIMIT);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn the_largest_declared_icon_under_the_cap_wins() {
        let html = r#"
            <link rel="icon" sizes="16x16" href="/small.png">
            <link rel="icon" sizes="32x32" href="/right.png">
            <link rel="icon" sizes="512x512" href="/huge.png">
        "#;
        assert_eq!(icon_href_of(html).as_deref(), Some("/right.png"));
    }

    #[test]
    fn a_vector_icon_beats_every_bitmap() {
        let html = r#"<link rel="icon" sizes="32x32" href="/a.png">
                      <link rel=icon href=/b.svg type=image/svg+xml>"#;
        assert_eq!(icon_href_of(html).as_deref(), Some("/b.svg"));
    }

    #[test]
    fn a_stylesheet_link_is_not_an_icon() {
        let html = r#"<link rel="stylesheet" href="/app.css">
                      <link rel="preconnect" href="https://cdn.example">"#;
        assert_eq!(icon_href_of(html), None);
    }

    /// `<linkedin>` is not `<link>`, and `data-title` is not `title`.
    #[test]
    fn a_tag_or_attribute_that_merely_starts_the_same_is_not_matched() {
        assert_eq!(icon_href_of("<linkedin rel=icon href=/no.png>"), None);
        assert_eq!(
            icon_href_of(r#"<link data-rel="icon" href="/no.png">"#),
            None
        );
    }
}
