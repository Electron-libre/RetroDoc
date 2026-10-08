//! Translation files: the words the product shows to its users, which are
//! often the best business vocabulary a repository has. The files and their
//! format come from the [`SourceMap`]; YAML, JSON, Java properties and
//! gettext catalogs are read.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::walker::FileEntry;

use super::sources::{language_code, path_language, SourceFormat, SourceKind, SourceMap};
use super::{Signal, SignalKind};

/// Lines kept per translation file.
const MAX_LINES_PER_FILE: usize = 60;
/// Characters kept of each translated text.
const MAX_VALUE_CHARS: usize = 160;

/// Language, file and `key: text` lines of one translation file; the
/// language is empty when nothing tells it.
type Parsed = (String, String, Vec<(String, String)>);

/// One signal per translation file the map selects, for a single language:
/// English when present, otherwise the language of most files (files whose
/// language is unknown are kept). Each line is `key.path: text`.
#[must_use]
pub fn i18n_texts(repo_root: &Path, files: &[FileEntry], sources: &SourceMap) -> Vec<Signal> {
    let mut parsed: Vec<Parsed> = Vec::new();
    for (file, format) in sources.files(SourceKind::I18n, files) {
        let Ok(bytes) = std::fs::read(repo_root.join(&file.path)) else {
            continue;
        };
        let content = String::from_utf8_lossy(&bytes);
        let origin = file.path.to_string_lossy().replace('\\', "/");
        let found = match format {
            SourceFormat::Yaml | SourceFormat::Json => structured(&file.path, &content),
            SourceFormat::Properties => vec![(
                path_language(&file.path).unwrap_or_default(),
                properties(&content),
            )],
            SourceFormat::Po => vec![po(&file.path, &content)],
            _ => continue,
        };
        for (lang, lines) in found {
            if !lines.is_empty() {
                parsed.push((lang, origin.clone(), lines));
            }
        }
    }
    let language = chosen_language(&parsed);
    parsed
        .into_iter()
        .filter(|(lang, ..)| lang.is_empty() || Some(lang) == language.as_ref())
        .map(|(_, origin, lines)| {
            let mut text = String::new();
            for (key, value) in lines.iter().take(MAX_LINES_PER_FILE) {
                let _ = writeln!(text, "{key}: {value}");
            }
            Signal {
                kind: SignalKind::I18n,
                origin,
                text: text.trim_end().to_string(),
            }
        })
        .collect()
}

fn chosen_language(parsed: &[Parsed]) -> Option<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (lang, ..) in parsed.iter().filter(|(l, ..)| !l.is_empty()) {
        *counts.entry(lang).or_default() += 1;
    }
    if counts.contains_key("en") {
        return Some("en".to_string());
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(lang, _)| lang.to_string())
}

/// YAML and JSON catalogs: either `{lang: {tree}}` (one or several
/// languages in the file) or a bare tree whose language is in the path.
fn structured(path: &Path, content: &str) -> Vec<(String, Vec<(String, String)>)> {
    // JSON is read as YAML, of which it is a subset.
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(content) else {
        tracing::debug!("skipping unparseable catalog {}", path.display());
        return Vec::new();
    };
    let Some(root) = value.as_mapping() else {
        return Vec::new();
    };
    // `{lang: {tree}}` only when every root key is a language holding a tree:
    // `{"no": "No", "yes": "Yes"}` is a bare catalog.
    let by_language: Option<Vec<_>> = root
        .iter()
        .map(|(key, tree)| {
            let lang = language_code(key.as_str()?, false)?;
            tree.is_mapping().then_some((lang, tree))
        })
        .collect();
    let trees: Vec<(String, &serde_yaml::Value)> = match by_language {
        Some(found) if !found.is_empty() => found,
        _ => vec![(path_language(path).unwrap_or_default(), &value)],
    };
    trees
        .into_iter()
        .map(|(lang, tree)| {
            let mut lines = Vec::new();
            flatten(tree, &mut Vec::new(), &mut lines);
            (lang, lines)
        })
        .collect()
}

fn flatten(value: &serde_yaml::Value, path: &mut Vec<String>, out: &mut Vec<(String, String)>) {
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (key, child) in map {
                let Some(key) = key.as_str().map(str::to_string).or_else(|| scalar(key)) else {
                    continue;
                };
                path.push(key);
                flatten(child, path, out);
                path.pop();
            }
        }
        serde_yaml::Value::String(text) => {
            if let Some(text) = clean(text) {
                if !path.is_empty() {
                    out.push((path.join("."), text));
                }
            }
        }
        _ => {}
    }
}

fn scalar(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Whitespace collapsed, capped; `None` when nothing is left.
fn clean(text: &str) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then(|| text.chars().take(MAX_VALUE_CHARS).collect())
}

/// `key=value` and `key: value` lines of a Java properties file.
fn properties(content: &str) -> Vec<(String, String)> {
    content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with(['#', '!']))
        .filter_map(|line| {
            let (key, value) = line.split_once(['=', ':'])?;
            Some((key.trim().to_string(), clean(value)?))
        })
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

/// A gettext catalog: each `msgid` is the key, its `msgstr` (or the msgid
/// itself when not translated yet) the text. The language is the `Language:`
/// header, else the one of the path.
fn po(path: &Path, content: &str) -> (String, Vec<(String, String)>) {
    #[derive(PartialEq)]
    enum Field {
        None,
        Id,
        Str,
    }
    let (mut id, mut text, mut field) = (String::new(), String::new(), Field::None);
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut flush = |id: &mut String, text: &mut String| {
        if !id.is_empty() || !text.is_empty() {
            entries.push((std::mem::take(id), std::mem::take(text)));
        }
    };
    for line in content.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("msgid ") {
            flush(&mut id, &mut text);
            id = po_string(rest);
            field = Field::Id;
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            text = po_string(rest);
            field = Field::Str;
        } else if line.starts_with('"') {
            match field {
                Field::Id => id.push_str(&po_string(line)),
                Field::Str => text.push_str(&po_string(line)),
                Field::None => {}
            }
        }
    }
    flush(&mut id, &mut text);
    let mut language = path_language(path).unwrap_or_default();
    let mut lines = Vec::new();
    for (id, text) in entries {
        if id.is_empty() {
            // The header entry.
            if let Some(code) = text
                .lines()
                .find_map(|l| l.strip_prefix("Language:"))
                .and_then(|l| language_code(l.trim(), true))
            {
                language = code;
            }
        } else if let Some(shown) = clean(if text.is_empty() { &id } else { &text }) {
            lines.push((clean(&id).unwrap_or_default(), shown));
        }
    }
    (language, lines)
}

/// The text inside the quotes of a `po` string, `\n` and `\"` undone.
fn po_string(text: &str) -> String {
    let text = text.trim();
    let inner = text
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text);
    inner.replace("\\n", "\n").replace("\\\"", "\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::FileKind;
    use std::fs;
    use std::path::PathBuf;

    fn run(files: &[(&str, &str, SourceFormat)]) -> Vec<Signal> {
        let dir = tempfile::tempdir().unwrap();
        let mut entries = Vec::new();
        let mut sources = SourceMap::default();
        for (path, content, format) in files {
            let abs = dir.path().join(path);
            fs::create_dir_all(abs.parent().unwrap()).unwrap();
            fs::write(abs, content).unwrap();
            entries.push(FileEntry {
                path: PathBuf::from(path),
                kind: FileKind::Other,
                size_bytes: 1,
            });
            sources.rules.push(super::super::sources::SourceRule {
                kind: SourceKind::I18n,
                glob: (*path).to_string(),
                format: *format,
            });
        }
        i18n_texts(dir.path(), &entries, &sources)
    }

    #[test]
    fn flattens_a_yaml_catalog() {
        let signals = run(&[(
            "config/locales/en.yml",
            "en:\n  contracts:\n    sign:\n      title: \"Sign the\n        contract\"\n      count: 3\n",
            SourceFormat::Yaml,
        )]);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].kind, SignalKind::I18n);
        assert_eq!(signals[0].origin, "config/locales/en.yml");
        assert_eq!(signals[0].text, "contracts.sign.title: Sign the contract");
    }

    #[test]
    fn reads_json_with_the_language_in_the_path_or_at_the_root() {
        let signals = run(&[
            (
                "public/locales/en/translation.json",
                r#"{"cart":{"title":"Cart"}}"#,
                SourceFormat::Json,
            ),
            (
                "public/locales/fr/translation.json",
                r#"{"cart":{"title":"Panier"}}"#,
                SourceFormat::Json,
            ),
            (
                "i18n/all.json",
                r#"{"en":{"home":"Home"},"fr":{"home":"Accueil"}}"#,
                SourceFormat::Json,
            ),
        ]);
        let texts: Vec<_> = signals.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["cart.title: Cart", "home: Home"]);
    }

    #[test]
    fn reads_java_properties() {
        let signals = run(&[
            (
                "res/messages_fr.properties",
                "# c\ncart.title=Panier\nbad line\nhome: Accueil\n",
                SourceFormat::Properties,
            ),
            (
                "res/messages_en.properties",
                "cart.title=Cart\n",
                SourceFormat::Properties,
            ),
        ]);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].text, "cart.title: Cart");
    }

    #[test]
    fn reads_gettext_catalogs() {
        let signals = run(&[(
            "locale/xx/messages.po",
            "msgid \"\"\nmsgstr \"\"\n\"Language: fr\\n\"\n\nmsgid \"Add to cart\"\nmsgstr \"Ajouter au panier\"\n\nmsgid \"Pay\"\nmsgstr \"\"\n\nmsgid \"Long \"\n\"text\"\nmsgstr \"Texte \"\n\"long\"\n",
            SourceFormat::Po,
        )]);
        assert_eq!(
            signals[0].text,
            "Add to cart: Ajouter au panier\nPay: Pay\nLong text: Texte long"
        );
    }

    #[test]
    fn keeps_the_most_frequent_language_when_there_is_no_english() {
        let signals = run(&[
            ("l/a.fr.yml", "fr:\n  a: Un\n", SourceFormat::Yaml),
            ("l/b.fr.yml", "fr:\n  b: Deux\n", SourceFormat::Yaml),
            ("l/c.de.yml", "de:\n  c: Drei\n", SourceFormat::Yaml),
        ]);
        assert_eq!(signals.len(), 2);
    }

    #[test]
    fn a_catalog_with_language_looking_keys_is_not_split() {
        let signals = run(&[(
            "locales/answers.json",
            r#"{"no":"No","yes":"Yes"}"#,
            SourceFormat::Json,
        )]);
        assert_eq!(signals[0].text, "no: No\nyes: Yes");
    }

    #[test]
    fn bad_catalogs_give_nothing() {
        let signals = run(&[("locales/bad.yml", "en: [unclosed\n", SourceFormat::Yaml)]);
        assert_eq!(signals, vec![]);
    }
}
