//! The page of a collected Markdown doc: the doc rendered to HTML with its raw
//! HTML shown as text and its links of an unsafe scheme dropped (a doc of the
//! repository is not trusted more than the LLM output), each heading carrying
//! the anchor the links of the item pages point to.

use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag, TagEnd};
use retrodoc_mcp::ExistingDoc;
use retrodoc_render::anchor;

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
    let (html, title) = render(without_front_matter(&content));
    Some(CollectedPage {
        path: path.to_string(),
        title: title.unwrap_or_else(|| path.to_string()),
        html,
    })
}

/// The HTML of `markdown`, rendered as a collected doc is: raw HTML as text, unsafe links dropped.
pub(crate) fn markdown_html(markdown: &str) -> String {
    render(markdown).0
}

/// The HTML of `markdown` and the text of its first heading.
fn render(markdown: &str) -> (String, Option<String>) {
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
                Event::Start(Tag::Heading {
                    level,
                    id: Some(CowStr::from(anchor(&text))),
                    classes,
                    attrs,
                })
            }
            Event::Start(Tag::Link { ref dest_url, .. }) if !safe_url(dest_url) => {
                dropped.push(TagEnd::Link);
                continue;
            }
            Event::Start(Tag::Image { ref dest_url, .. }) if !safe_url(dest_url) => {
                dropped.push(TagEnd::Image);
                continue;
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
fn safe_url(url: &str) -> bool {
    let url: String = url
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && !c.is_control())
        .collect();
    let Some(colon) = url.find(':') else {
        return true;
    };
    let scheme = &url[..colon];
    let is_scheme = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    !is_scheme
        || ["http", "https", "mailto"]
            .iter()
            .any(|s| scheme.eq_ignore_ascii_case(s))
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
