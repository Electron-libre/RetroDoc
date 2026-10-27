//! Code citations of an answer, shared by the passes that show numbered files (`=== [F2] path ===`):
//! `"F2:10-24"`, `"F2:7"`, `"F2"` or a path with the same suffixes, or the `{"path", "start_line",
//! "end_line"}` objects of the older answers, resolved to a file shown to the LLM.

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::use_cases::resolve_cited_path;

#[derive(Debug, Deserialize)]
pub(crate) struct RawCode {
    /// A file number of the prompt (`F2`) or a path.
    pub(crate) path: Option<String>,
    pub(crate) start_line: Option<u32>,
    pub(crate) end_line: Option<u32>,
}

/// The cited code: `"F2:10-24"`, `"F2:7"`, `"F2"` or `"lib/a.rb:3-4"` strings, or the `{"path",
/// "start_line", "end_line"}` objects an older answer (or a model that ignores the shape) gives;
/// anything else is skipped.
pub(crate) fn lenient_code<'de, D>(deserializer: D) -> Result<Vec<RawCode>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let items = match value {
        Some(serde_json::Value::Array(items)) => items,
        Some(single @ (serde_json::Value::String(_) | serde_json::Value::Object(_))) => {
            vec![single]
        }
        _ => Vec::new(),
    };
    Ok(items.iter().filter_map(raw_code_of).collect())
}

fn raw_code_of(value: &serde_json::Value) -> Option<RawCode> {
    match value {
        serde_json::Value::String(text) => Some(parse_citation(text)),
        serde_json::Value::Object(_) => serde_json::from_value(value.clone()).ok(),
        _ => None,
    }
}

/// `file:start-end`, `file:line` or `file`; a suffix after the last `:` that is not lines belongs
/// to the file.
fn parse_citation(text: &str) -> RawCode {
    let text = text.trim();
    if let Some((file, lines)) = text.rsplit_once(':') {
        let (start, end) = lines.split_once('-').unwrap_or((lines, ""));
        let start = start.trim().parse::<u32>().ok();
        let end = end.trim().parse::<u32>().ok();
        if start.is_some() && (end.is_some() || !lines.contains('-')) {
            return RawCode {
                path: Some(file.trim().to_string()),
                start_line: start,
                end_line: end,
            };
        }
    }
    // A slip in the range (`F2:L10-24`) costs the lines, not the file, when the file is a number.
    let file = text.split_once(':').map_or(text, |(file, _)| file);
    RawCode {
        path: Some(
            if file_number(file).is_some() {
                file
            } else {
                text
            }
            .to_string(),
        ),
        start_line: None,
        end_line: None,
    }
}

/// The number of a file as the prompt shows it (`F2`, or `[F2]`), unless it is a path.
fn file_number(reference: &str) -> Option<usize> {
    let digits = reference
        .trim()
        .trim_matches(|c| c == '[' || c == ']')
        .strip_prefix(['F', 'f'])?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The file shown to the LLM that `cited` designates: the one of its number in `numbered` (from 1),
/// else the one its path resolves to among `files`. `None` for an unknown number or path.
pub(crate) fn cited_path(
    cited: &RawCode,
    numbered: &[String],
    files: &BTreeSet<String>,
) -> Option<String> {
    let reference = cited.path.as_deref()?;
    match file_number(reference).filter(|_| !files.contains(reference.trim())) {
        Some(number) => numbered.get(number.checked_sub(1)?).cloned(),
        None => resolve_cited_path(reference, files),
    }
}
