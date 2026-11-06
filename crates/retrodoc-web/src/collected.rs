//! The page of a collected Markdown doc (and the narrative of a use case, WEB-118): the doc rendered to HTML with its raw
//! HTML shown as text and its links of an unsafe scheme dropped (a doc of the
//! repository is not trusted more than the LLM output), each heading carrying
//! the anchor the links of the item pages point to. A narrative shown inside its page keeps
//! the first two rules, its headings set under the section that holds it, with no anchor, and
//! its relative links led where they lead from its Markdown page (`NarrativeLinks`).

use pulldown_cmark::{html, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use retrodoc_mcp::ExistingDoc;
use retrodoc_render::{anchor, use_case_path};

use crate::review::Place;
use crate::view::{find_feature, url_path, Site};

pub struct CollectedPage {
    /// From the repository root.
    pub path: String,
    /// The first heading, else the path.
    pub title: String,
    /// The doc rendered, safe to insert as is.
    pub html: String,
}

/// The page of the collected doc at `path`, `None` when no collected doc is there.
#[must_use]
pub fn collected_page(collected: &[ExistingDoc], path: &str) -> Option<CollectedPage> {
    let doc = collected
        .iter()
        .find(|d| d.path.to_string_lossy().replace('\\', "/") == path)?;
    let content = doc.content.replace("\r\n", "\n");
    let (html, title) = render(without_front_matter(&content), Flavour::Doc);
    Some(CollectedPage {
        path: path.to_string(),
        title: title.unwrap_or_else(|| path.to_string()),
        html,
    })
}

/// The HTML of `markdown`, rendered as a collected doc is: raw HTML as text, unsafe links dropped.
pub(crate) fn markdown_html(markdown: &str) -> String {
    render(markdown, Flavour::Doc).0
}

/// Where the relative links of a narrative lead (WEB-123): from the Markdown page of its use
/// case to the collected docs.
pub(crate) struct NarrativeLinks<'a> {
    /// The dirs of the Markdown page, from the repository root; `None` for a section with no
    /// narrative or one the run lacks.
    page_dir: Option<Vec<String>>,
    collected: &'a [ExistingDoc],
}

impl<'a> NarrativeLinks<'a> {
    /// The links of the narrative of the section at `place` of the run `site` serves.
    pub(crate) fn of(site: &'a Site, place: &Place) -> Self {
        let page_dir = match place {
            Place::UseCase {
                domain,
                feature,
                slug,
            } => site.docs().and_then(|docs| {
                let owner = find_feature(docs, domain, feature)?;
                let use_case = docs.use_cases_of(feature).find(|u| &u.slug == slug)?;
                let page = site.links().docs_dir.join(use_case_path(owner, use_case));
                let mut dirs: Vec<String> = Vec::new();
                for part in page.parent()?.components() {
                    match part {
                        std::path::Component::Normal(name) => {
                            dirs.push(name.to_string_lossy().into_owned());
                        }
                        std::path::Component::CurDir => {}
                        _ => return None,
                    }
                }
                Some(dirs)
            }),
            _ => None,
        };
        Self {
            page_dir,
            collected: site.collected(),
        }
    }

    /// Where the relative `url` leads in the interface: the page of the collected doc it resolves
    /// to, its fragment kept; `None` when no collected doc is there.
    fn resolve(&self, url: &str) -> Option<String> {
        let (path, fragment) = url
            .split_once('#')
            .map_or((url, None), |(path, fragment)| (path, Some(fragment)));
        let path = path.split_once('?').map_or(path, |(path, _)| path);
        if path.starts_with('/') {
            return None;
        }
        let mut target = self.page_dir.clone()?;
        for part in path.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    target.pop()?;
                }
                name => target.push(percent_decoded(name)),
            }
        }
        let target = target.join("/");
        self.collected
            .iter()
            .any(|d| d.path.to_string_lossy().replace('\\', "/") == target)
            .then(|| match fragment {
                Some(fragment) => format!("/docs/{}#{fragment}", url_path(&target)),
                None => format!("/docs/{}", url_path(&target)),
            })
    }
}

/// The narrative of a use case rendered as its page and the preview of its edit show it (WEB-118),
/// its line ends and surrounding whitespace cleaned as a save cleans them (WEB-94), its headings
/// under the `h2` of its section with no anchor (WEB-122), its relative links resolved by `links`
/// (WEB-123); `None` when it is blank.
pub(crate) fn narrative_html(narrative: &str, links: &NarrativeLinks) -> Option<String> {
    let narrative = narrative.replace("\r\n", "\n");
    let narrative = narrative.trim();
    (!narrative.is_empty()).then(|| render(narrative, Flavour::Narrative(links)).0)
}

/// What the Markdown is rendered as.
#[derive(Clone, Copy)]
enum Flavour<'a> {
    /// A page of its own: each heading at its level, with its anchor, its relative links kept.
    Doc,
    /// A section inside a page: each heading two levels down, with no anchor (WEB-122), its
    /// relative links resolved (WEB-123).
    Narrative(&'a NarrativeLinks<'a>),
}

/// The HTML of `markdown` and the text of its first heading.
fn render(markdown: &str, flavour: Flavour) -> (String, Option<String>) {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut events = Vec::new();
    let mut title = None;
    // A link or an image dropped for its scheme: its end is dropped too.
    let mut dropped: Vec<TagEnd> = Vec::new();
    for (event, range) in Parser::new_ext(markdown, options).into_offset_iter() {
        let event = match event {
            Event::Start(Tag::Heading {
                level,
                classes,
                attrs,
                ..
            }) => {
                let text = heading_text(&markdown[range]);
                if title.is_none() && !text.is_empty() {
                    title = Some(text.clone());
                }
                let (level, id) = match flavour {
                    Flavour::Doc => (level, Some(CowStr::from(anchor(&text)))),
                    Flavour::Narrative(_) => (two_levels_down(level), None),
                };
                Event::Start(Tag::Heading {
                    level,
                    id,
                    classes,
                    attrs,
                })
            }
            Event::End(TagEnd::Heading(level)) if matches!(flavour, Flavour::Narrative(_)) => {
                Event::End(TagEnd::Heading(two_levels_down(level)))
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let Some(dest_url) = led(&dest_url, flavour) else {
                    dropped.push(TagEnd::Link);
                    continue;
                };
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let Some(dest_url) = led(&dest_url, flavour) else {
                    dropped.push(TagEnd::Image);
                    continue;
                };
                Event::Start(Tag::Image {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
            Event::End(end) if dropped.last() == Some(&end) => {
                dropped.pop();
                continue;
            }
            Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(raw),
            event => event,
        };
        events.push(event);
    }
    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    (out, title)
}

/// `segment` with its `%XX` escapes decoded, as a browser asks for the file; an escape that is
/// not one is kept as written.
fn percent_decoded(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| segment.get(at + 1..at + 3))
            .flatten()
            .filter(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        if let Some(byte) = escaped {
            decoded.push(byte);
            at += 3;
        } else {
            decoded.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Where a link or an image of `url` leads, `None` when it is shown as its text alone: an unsafe
/// scheme (WEB-50), or a relative URL of a narrative that leads to no collected doc (WEB-123).
fn led<'a>(url: &CowStr<'a>, flavour: Flavour) -> Option<CowStr<'a>> {
    match (scheme(url), flavour) {
        (Some(scheme), _) => safe_scheme(&scheme).then(|| url.clone()),
        (None, Flavour::Narrative(links)) if !url.starts_with('#') => {
            links.resolve(url).map(CowStr::from)
        }
        (None, _) => Some(url.clone()),
    }
}

/// `level` two levels down, `h6` at most: under the `h2` of the section of a narrative.
fn two_levels_down(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 => HeadingLevel::H3,
        HeadingLevel::H2 => HeadingLevel::H4,
        HeadingLevel::H3 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    }
}

/// `content` without its YAML front matter, by the rule the doc sections are
/// read with (`retrodoc-ingest`, `signals`), so both see the same headings.
fn without_front_matter(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("---\n") else {
        return content;
    };
    rest.split_once("\n---\n")
        .map_or(content, |(_, after)| after)
}

/// The text of a heading as the doc sections name it: its first line without
/// the `#` that open it (`specs/docs-reconciliation.md`).
fn heading_text(source: &str) -> String {
    let line = source.lines().next().unwrap_or_default().trim();
    line.trim_start_matches('#').trim().to_string()
}

/// A relative URL, or one of the schemes `http`, `https` and `mailto`. The
/// blanks and control characters a browser ignores are ignored too, so
/// `java\tscript:` counts as `javascript:`.
#[cfg(test)]
fn safe_url(url: &str) -> bool {
    scheme(url).is_none_or(|scheme| safe_scheme(&scheme))
}

fn safe_scheme(scheme: &str) -> bool {
    ["http", "https", "mailto"]
        .iter()
        .any(|s| scheme.eq_ignore_ascii_case(s))
}

/// The scheme of `url` as a browser reads it, `None` for a relative URL.
fn scheme(url: &str) -> Option<String> {
    let url: String = url
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && !c.is_control())
        .collect();
    let scheme = &url[..url.find(':')?];
    let is_scheme = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    is_scheme.then(|| scheme.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: WEB-50 (internal: the variants a browser reads as a scheme)
    #[test]
    fn the_scheme_is_read_as_a_browser_reads_it() {
        for unsafe_url in [
            "javascript:x",
            "JAVASCRIPT:x",
            "java\tscript:x",
            "data:x",
            "vbscript:x",
        ] {
            assert!(!safe_url(unsafe_url), "{unsafe_url}");
        }
        for safe in [
            "other.md",
            "../a.md#b",
            "#top",
            "/x?a=b:c",
            "https://e.com",
            "MAILTO:a@b",
        ] {
            assert!(safe_url(safe), "{safe}");
        }
    }
}
