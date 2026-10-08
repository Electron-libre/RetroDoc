//! Which files hold the schema, the migrations and the translations of a
//! repository, and in which format. No framework location is written in the
//! readers: they know formats, and this map says where to apply them. It is
//! first guessed from the content of the files ([`SourceMap::sniff`], no
//! LLM); the pipeline can replace it with an inferred, hand-editable one.

use std::collections::HashSet;
use std::path::Path;

use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Serialize};

use crate::walker::FileEntry;

/// What a source tells about the product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Tables and columns.
    Schema,
    /// The history of schema changes, known by the names of its files.
    Migrations,
    /// Translated texts.
    I18n,
}

/// How to read a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    /// SQL `CREATE TABLE` statements.
    SqlDdl,
    /// `ActiveRecord::Schema` `create_table` blocks.
    RailsSchema,
    /// Only the file names matter.
    FileNames,
    Yaml,
    Json,
    /// Java-style `key=value` lines.
    Properties,
    /// Gettext catalogs.
    Po,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRule {
    pub kind: SourceKind,
    /// Glob relative to the repo root (`db/**/*.sql`).
    pub glob: String,
    pub format: SourceFormat,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMap {
    pub rules: Vec<SourceRule>,
}

/// Escapes the glob characters of a path, so a rule can name exactly one file.
#[must_use]
pub fn escape_glob(path: &str) -> String {
    globset::escape(path)
}

/// Files bigger than this are not read to be sniffed.
const MAX_SNIFFED_BYTES: u64 = 2_000_000;

/// ISO 639-1 language codes, to tell `messages_fr.json` from `my_settings.json`.
const LANGUAGES: &str =
    "aa ab af ak am an ar as av ay az ba be bg bh bi bm bn bo br bs ca ce ch co \
cr cs cu cv cy da de dv dz ee el en eo es et eu fa ff fi fj fo fr fy ga gd gl gn gu gv ha he hi ho \
hr ht hu hy hz ia id ie ig ii ik io is it iu ja jv ka kg ki kj kk kl km kn ko kr ks ku kv kw ky la \
lb lg li ln lo lt lu lv mg mh mi mk ml mn mr ms mt my na nb nd ne ng nl nn no nr nv ny oc oj om or \
os pa pi pl ps pt qu rm rn ro ru rw sa sc sd se sg si sk sl sm sn so sq sr ss st su sv sw ta te tg \
th ti tk tl tn to tr ts tt tw ty ug uk ur uz ve vi vo wa wo xh yi yo za zh zu";

/// Words that name a place for translations.
const TRANSLATION_WORDS: &[&str] = &[
    "locale",
    "locales",
    "i18n",
    "l10n",
    "lang",
    "langs",
    "languages",
    "translations",
    "messages",
    "intl",
];

impl SourceMap {
    /// The files of `kind` the rules select, each once, with the format of
    /// the first rule that matches it. A glob that doesn't compile is skipped
    /// with a warning.
    #[must_use]
    pub fn files<'a>(
        &self,
        kind: SourceKind,
        files: &'a [FileEntry],
    ) -> Vec<(&'a FileEntry, SourceFormat)> {
        let matchers: Vec<(GlobMatcher, SourceFormat)> = self
            .rules
            .iter()
            .filter(|r| r.kind == kind)
            .filter_map(|r| match Glob::new(&r.glob) {
                Ok(glob) => Some((glob.compile_matcher(), r.format)),
                Err(error) => {
                    tracing::warn!("ignoring source glob {:?}: {error}", r.glob);
                    None
                }
            })
            .collect();
        files
            .iter()
            .filter_map(|file| {
                matchers
                    .iter()
                    .find(|(matcher, _)| matcher.is_match(&file.path))
                    .map(|(_, format)| (file, *format))
            })
            .collect()
    }

    /// Guesses the sources from the files and their content, whatever the
    /// stack: migrations are the files of a folder named `migrat*`, a
    /// Flyway-style `V<n>__name` file or an Alembic revision; the schema is
    /// SQL that creates tables or an `ActiveRecord::Schema`; translations
    /// are gettext catalogs and YAML, JSON or properties files that sit under
    /// a language code or a translation folder. One rule per file.
    #[must_use]
    pub fn sniff(repo_root: &Path, files: &[FileEntry]) -> Self {
        let mut rules = Vec::new();
        for file in files {
            if let Some((kind, format)) = sniff_file(repo_root, file) {
                rules.push(SourceRule {
                    kind,
                    glob: globset::escape(&file.path.to_string_lossy().replace('\\', "/")),
                    format,
                });
            }
        }
        SourceMap { rules }
    }
}

fn sniff_file(repo_root: &Path, file: &FileEntry) -> Option<(SourceKind, SourceFormat)> {
    let name = file.path.file_name()?.to_string_lossy().into_owned();
    let ext = file
        .path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let read = || -> Option<String> {
        (file.size_bytes <= MAX_SNIFFED_BYTES)
            .then(|| std::fs::read(repo_root.join(&file.path)).ok())
            .flatten()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    };

    if is_migration(file, &name, &ext, &read) {
        return Some((SourceKind::Migrations, SourceFormat::FileNames));
    }
    let stem = name.split('.').next().unwrap_or_default().to_lowercase();
    match ext.as_str() {
        "sql" if read()?.to_lowercase().contains("create table") => {
            Some((SourceKind::Schema, SourceFormat::SqlDdl))
        }
        "rb" if stem.contains("schema") && read()?.contains("ActiveRecord::Schema") => {
            Some((SourceKind::Schema, SourceFormat::RailsSchema))
        }
        "po" => Some((SourceKind::I18n, SourceFormat::Po)),
        "yml" | "yaml" => {
            let located = path_language(&file.path).is_some() && has_translation_word(&file.path);
            (located || first_yaml_key_is_language(&read()?))
                .then_some((SourceKind::I18n, SourceFormat::Yaml))
        }
        "json" => {
            let located = path_language(&file.path).is_some() || has_translation_word(&file.path);
            (located && only_string_leaves(&read()?))
                .then_some((SourceKind::I18n, SourceFormat::Json))
        }
        "properties" => (path_language(&file.path).is_some() || has_translation_word(&file.path))
            .then_some((SourceKind::I18n, SourceFormat::Properties)),
        _ => None,
    }
}

fn is_migration(
    file: &FileEntry,
    name: &str,
    ext: &str,
    read: &dyn Fn() -> Option<String>,
) -> bool {
    if name.ends_with(".Designer.cs")
        || name.ends_with("ModelSnapshot.cs")
        || matches!(
            ext,
            "md" | "mdx" | "txt" | "rst" | "adoc" | "gitkeep" | "keep"
        )
    {
        return false;
    }
    let in_migration_dir = file.path.parent().is_some_and(|p| {
        p.components().any(|c| {
            c.as_os_str()
                .to_string_lossy()
                .to_lowercase()
                .starts_with("migrat")
        })
    });
    let flyway = name.strip_prefix(['V', 'v']).is_some_and(|rest| {
        rest.split_once("__").is_some_and(|(n, _)| {
            !n.is_empty()
                && n.chars()
                    .all(|c| c.is_ascii_digit() || c == '_' || c == '.')
        })
    });
    let alembic = ext == "py"
        && file
            .path
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|d| d == "versions")
        && read().is_some_and(|c| c.contains("down_revision"));
    (in_migration_dir || flyway || alembic) && name != "__init__.py"
}

/// The language of a code such as `fr`, `pt-BR`, `zh_Hans` or `es-419`
/// (lower case, region dropped); `None` for any other text. With `lenient`,
/// a lower-case region (`pt-br`) is accepted too.
#[must_use]
pub fn language_code(text: &str, lenient: bool) -> Option<String> {
    static CODES: std::sync::OnceLock<HashSet<&'static str>> = std::sync::OnceLock::new();
    let codes = CODES.get_or_init(|| LANGUAGES.split_whitespace().collect());
    let (head, region) = text.split_once(['-', '_']).unwrap_or((text, ""));
    let head = head.to_lowercase();
    let region_ok = region.is_empty()
        || (region.len() == 2
            && region
                .chars()
                .all(|c| c.is_ascii_uppercase() || (lenient && c.is_ascii_lowercase())))
        || (region.len() == 3 && region.chars().all(|c| c.is_ascii_digit()))
        || (region.len() == 4
            && region
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
            && region.chars().skip(1).all(|c| c.is_ascii_lowercase()));
    (codes.contains(head.as_str()) && region_ok).then_some(head)
}

/// The language code a path is under: its file name is it (`fr.yml`) or ends
/// with it in a translation path (`messages_fr.properties`), or a folder is
/// named after it (`locales/pt-BR/app.json`). Lower case, region dropped.
#[must_use]
pub fn path_language(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    if let Some(code) = language_code(&stem, true) {
        return Some(code);
    }
    // `app.fr.yml`, `messages_fr.properties`: a last word that is also a code
    // (`go_to`) says nothing unless the path is about translations.
    let last_token = stem.rsplit(['.', '_']).next().unwrap_or_default();
    if has_translation_word(path) {
        if let Some(code) = language_code(last_token, true) {
            return Some(code);
        }
    }
    path.parent()?
        .components()
        .rev()
        .find_map(|c| language_code(&c.as_os_str().to_string_lossy(), true))
}

fn has_translation_word(path: &Path) -> bool {
    path.components().any(|c| {
        let part = c.as_os_str().to_string_lossy().to_lowercase();
        part.split(['.', '_', '-'])
            .any(|word| TRANSLATION_WORDS.contains(&word))
    })
}

/// Whether the first top-level key of a YAML text is a language code
/// (`en:`, `pt-BR:`).
fn first_yaml_key_is_language(content: &str) -> bool {
    content
        .lines()
        .find(|l| {
            !l.trim().is_empty() && !l.trim_start().starts_with(['#', '-']) && !l.starts_with(' ')
        })
        .and_then(|line| line.trim().strip_suffix(':'))
        .is_some_and(|key| language_code(key.trim_matches(['"', '\'']), false).is_some())
}

/// A JSON object whose leaves are all strings (a catalog), unlike a
/// manifest or a settings file.
fn only_string_leaves(content: &str) -> bool {
    fn leaves(value: &serde_json::Value, strings: &mut usize) -> bool {
        match value {
            serde_json::Value::String(_) => {
                *strings += 1;
                true
            }
            serde_json::Value::Object(map) => map.values().all(|v| leaves(v, strings)),
            _ => false,
        }
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return false;
    };
    let mut strings = 0;
    value.is_object() && leaves(&value, &mut strings) && strings > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::FileKind;
    use std::fs;
    use std::path::PathBuf;

    fn repo(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<FileEntry>) {
        let dir = tempfile::tempdir().unwrap();
        let mut entries = Vec::new();
        for (path, content) in files {
            let abs = dir.path().join(path);
            fs::create_dir_all(abs.parent().unwrap()).unwrap();
            fs::write(&abs, content).unwrap();
            entries.push(FileEntry {
                path: PathBuf::from(path),
                kind: FileKind::Other,
                size_bytes: content.len() as u64,
            });
        }
        (dir, entries)
    }

    fn sniffed(files: &[(&str, &str)]) -> Vec<(String, SourceKind, SourceFormat)> {
        let (dir, entries) = repo(files);
        SourceMap::sniff(dir.path(), &entries)
            .rules
            .into_iter()
            .map(|r| (r.glob, r.kind, r.format))
            .collect()
    }

    fn has(
        found: &[(String, SourceKind, SourceFormat)],
        path: &str,
        kind: SourceKind,
        format: SourceFormat,
    ) -> bool {
        found
            .iter()
            .any(|(g, k, f)| g == path && *k == kind && *f == format)
    }

    #[test]
    fn recognizes_rails_sources() {
        let found = sniffed(&[
            ("db/schema.rb", "ActiveRecord::Schema.define do\nend\n"),
            ("db/migrate/20260101_add_x.rb", "class AddX; end"),
            ("config/locales/en.yml", "en:\n  a: b\n"),
            ("config/locales/shop/fr.yml", "fr:\n  a: b\n"),
            ("config/database.yml", "production:\n  adapter: pg\n"),
            ("config/locales/app.yml", "to_do:\n  a: b\n"),
            ("db/migrate/README.md", "about migrations"),
        ]);
        assert!(has(
            &found,
            "db/schema.rb",
            SourceKind::Schema,
            SourceFormat::RailsSchema
        ));
        assert!(has(
            &found,
            "db/migrate/20260101_add_x.rb",
            SourceKind::Migrations,
            SourceFormat::FileNames
        ));
        assert!(has(
            &found,
            "config/locales/en.yml",
            SourceKind::I18n,
            SourceFormat::Yaml
        ));
        assert!(has(
            &found,
            "config/locales/shop/fr.yml",
            SourceKind::I18n,
            SourceFormat::Yaml
        ));
        assert_eq!(found.len(), 4);
    }

    #[test]
    fn recognizes_flyway_and_spring_sources() {
        let found = sniffed(&[
            (
                "src/main/resources/db/migration/V1__create_orders.sql",
                "CREATE TABLE orders (id int);",
            ),
            (
                "src/main/resources/messages_fr.properties",
                "cart.title=Panier\n",
            ),
            (
                "src/main/resources/application.properties",
                "server.port=8080\n",
            ),
            ("sql/model.sql", "create table customers (id int);"),
        ]);
        assert!(has(
            &found,
            "src/main/resources/db/migration/V1__create_orders.sql",
            SourceKind::Migrations,
            SourceFormat::FileNames
        ));
        assert!(has(
            &found,
            "src/main/resources/messages_fr.properties",
            SourceKind::I18n,
            SourceFormat::Properties
        ));
        assert!(has(
            &found,
            "sql/model.sql",
            SourceKind::Schema,
            SourceFormat::SqlDdl
        ));
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn recognizes_python_and_js_sources() {
        let found = sniffed(&[
            (
                "alembic/versions/ab12cd34_add_users.py",
                "revision = 'ab12cd34'\ndown_revision = None\n",
            ),
            ("shop/migrations/0001_initial.py", "class Migration: pass"),
            ("shop/migrations/__init__.py", ""),
            (
                "locale/fr/LC_MESSAGES/django.po",
                "msgid \"a\"\nmsgstr \"b\"\n",
            ),
            (
                "public/locales/en/translation.json",
                "{\"cart\":{\"title\":\"Cart\"}}",
            ),
            ("package.json", "{\"name\":\"x\",\"version\":\"1\"}"),
            ("src/go_to.json", "{\"a\":\"b\"}"),
            ("tsconfig.json", "{\"compilerOptions\":{\"strict\":true}}"),
        ]);
        assert!(has(
            &found,
            "alembic/versions/ab12cd34_add_users.py",
            SourceKind::Migrations,
            SourceFormat::FileNames
        ));
        assert!(has(
            &found,
            "shop/migrations/0001_initial.py",
            SourceKind::Migrations,
            SourceFormat::FileNames
        ));
        assert!(has(
            &found,
            "locale/fr/LC_MESSAGES/django.po",
            SourceKind::I18n,
            SourceFormat::Po
        ));
        assert!(has(
            &found,
            "public/locales/en/translation.json",
            SourceKind::I18n,
            SourceFormat::Json
        ));
        assert_eq!(found.len(), 4, "{found:?}");
    }

    #[test]
    fn a_map_selects_files_by_glob_with_the_first_matching_format() {
        let (_dir, entries) = repo(&[("a/x.sql", ""), ("a/y.txt", ""), ("b/z.sql", "")]);
        let map = SourceMap {
            rules: vec![
                SourceRule {
                    kind: SourceKind::Schema,
                    glob: "a/**/*.sql".into(),
                    format: SourceFormat::SqlDdl,
                },
                SourceRule {
                    kind: SourceKind::Schema,
                    glob: "**/*.sql".into(),
                    format: SourceFormat::RailsSchema,
                },
                SourceRule {
                    kind: SourceKind::I18n,
                    glob: "[".into(),
                    format: SourceFormat::Json,
                },
            ],
        };
        let found: Vec<_> = map
            .files(SourceKind::Schema, &entries)
            .into_iter()
            .map(|(f, fmt)| (f.path.to_string_lossy().into_owned(), fmt))
            .collect();
        assert_eq!(
            found,
            [
                ("a/x.sql".to_string(), SourceFormat::SqlDdl),
                ("b/z.sql".to_string(), SourceFormat::RailsSchema)
            ]
        );
        assert!(map.files(SourceKind::I18n, &entries).is_empty());
    }

    #[test]
    fn finds_the_language_of_a_path() {
        let lang = |p: &str| path_language(Path::new(p));
        assert_eq!(lang("config/locales/fr.yml"), Some("fr".into()));
        assert_eq!(lang("messages_pt-BR.properties"), Some("pt".into()));
        assert_eq!(lang("locales/app.de.yml"), Some("de".into()));
        assert_eq!(lang("app.de.yml"), None);
        assert_eq!(lang("src/go_to.json"), None);
        assert_eq!(lang("locales/ja/app.json"), Some("ja".into()));
        assert_eq!(lang("src/settings.json"), None);
    }
}
