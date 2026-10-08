//! The data model as the repository declares it: tables and columns of the
//! schema file, and the names of the migrations, which tell how the product
//! grew.

use std::fmt::Write as _;
use std::path::Path;

use crate::walker::FileEntry;

use super::sources::{SourceFormat, SourceKind, SourceMap};
use super::{Signal, SignalKind};

/// Tables kept per schema file.
const MAX_TABLES: usize = 150;
/// Columns kept per table.
const MAX_COLUMNS: usize = 25;
/// Migration names kept (the newest).
const MAX_MIGRATIONS: usize = 150;

/// One signal per schema file the map selects (SQL DDL or an
/// `ActiveRecord` schema): its tables with their column names.
#[must_use]
pub fn schema_tables(repo_root: &Path, files: &[FileEntry], sources: &SourceMap) -> Vec<Signal> {
    let mut signals = Vec::new();
    for (file, format) in sources.files(SourceKind::Schema, files) {
        let parse = match format {
            SourceFormat::SqlDdl => parse_sql_ddl,
            SourceFormat::RailsSchema => parse_schema_rb,
            _ => continue,
        };
        let Ok(bytes) = std::fs::read(repo_root.join(&file.path)) else {
            continue;
        };
        let tables = parse(&String::from_utf8_lossy(&bytes));
        if tables.is_empty() {
            continue;
        }
        let mut text = String::new();
        for (table, columns) in tables.iter().take(MAX_TABLES) {
            let kept: Vec<&str> = columns
                .iter()
                .take(MAX_COLUMNS)
                .map(String::as_str)
                .collect();
            let _ = writeln!(text, "{table}: {}", kept.join(", "));
        }
        if tables.len() > MAX_TABLES {
            let _ = writeln!(text, "… {} more tables", tables.len() - MAX_TABLES);
        }
        signals.push(Signal {
            kind: SignalKind::Schema,
            origin: file.path.to_string_lossy().replace('\\', "/"),
            text: text.trim_end().to_string(),
        });
    }
    signals
}

/// One signal naming the migrations the map selects in words (`add copy
/// pending to annexes`), oldest first (the newest ones kept). The names are
/// sorted as written: the timestamp or sequence number leads them.
#[must_use]
pub fn migration_names(files: &[FileEntry], sources: &SourceMap) -> Option<Signal> {
    let mut names: Vec<(String, String)> = sources
        .files(SourceKind::Migrations, files)
        .into_iter()
        .filter_map(|(file, _)| {
            let stem = file.path.file_stem()?.to_string_lossy().into_owned();
            let sentence = in_words(&stem);
            (!sentence.is_empty() && sentence != "init").then_some((stem, sentence))
        })
        .collect();
    if names.is_empty() {
        return None;
    }
    names.sort_by_key(|(stem, _)| version_order(stem));
    let skipped = names.len().saturating_sub(MAX_MIGRATIONS);
    let mut text: String = names
        .iter()
        .skip(skipped)
        .map(|(_, sentence)| sentence.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if skipped > 0 {
        let _ = write!(text, "\n… {skipped} older migrations");
    }
    Some(Signal {
        kind: SignalKind::Migrations,
        origin: "migrations".to_string(),
        text,
    })
}

/// Sort key of a migration: its leading version as a number (`V10` after
/// `V2`), those without one last, then the name.
fn version_order(stem: &str) -> (u128, String) {
    let digits: String = stem
        .trim_start_matches(['V', 'v'])
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (digits.parse().unwrap_or(u128::MAX), stem.to_string())
}

/// `20260423_add_copy_pending` / `V2__add_orders` / `ab12cd34_add_users`
/// become `add copy pending` / `add orders` / `add users`: the version
/// token (digits, or hex with a digit in it) leads and is dropped.
fn in_words(stem: &str) -> String {
    let stem = stem
        .strip_prefix(['V', 'v'])
        .filter(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        .unwrap_or(stem);
    let mut words: Vec<&str> = stem.split(['_', '-']).filter(|w| !w.is_empty()).collect();
    while words.first().is_some_and(|w| {
        w.chars().all(|c| c.is_ascii_hexdigit() || c == '.')
            && w.chars().any(|c| c.is_ascii_digit())
    }) {
        words.remove(0);
    }
    words.join(" ")
}

type Tables = Vec<(String, Vec<String>)>;

/// `create_table "users", force: :cascade do |t|` then `t.string "email"`.
fn parse_schema_rb(content: &str) -> Tables {
    let mut tables: Tables = Vec::new();
    let mut open = false;
    for line in content.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("create_table ") {
            if let Some(name) = quoted_or_symbol(rest) {
                tables.push((name, Vec::new()));
                open = true;
            }
        } else if line == "end" {
            open = false;
        } else if open && line.starts_with("t.") {
            // `t.index [...]` and `t.check_constraint` are not columns.
            let Some((kind, rest)) = line[2..].split_once(' ') else {
                continue;
            };
            if matches!(
                kind,
                "index" | "check_constraint" | "foreign_key" | "exclusion_constraint"
            ) {
                continue;
            }
            if let (Some(name), Some((_, columns))) = (quoted_or_symbol(rest), tables.last_mut()) {
                columns.push(name);
            }
        }
    }
    tables
}

/// The first `"name"`, `'name'` or `:name` of a Ruby call.
fn quoted_or_symbol(text: &str) -> Option<String> {
    let text = text.trim_start_matches('(').trim();
    if let Some(symbol) = text.strip_prefix(':') {
        let name: String = symbol
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        return (!name.is_empty()).then_some(name);
    }
    let quote = text.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let body = &text[1..];
    body.find(quote).map(|end| body[..end].to_string())
}

/// `CREATE TABLE schema.users (` then one column per line up to `);`.
fn parse_sql_ddl(content: &str) -> Tables {
    const NOT_COLUMNS: &[&str] = &[
        "CONSTRAINT",
        "PRIMARY",
        "FOREIGN",
        "UNIQUE",
        "CHECK",
        "EXCLUDE",
        "LIKE",
    ];
    let mut tables: Tables = Vec::new();
    let mut open = false;
    for line in content.lines().map(str::trim) {
        let upper = line.to_uppercase();
        if upper.starts_with("CREATE TABLE ") || upper.starts_with("CREATE UNLOGGED TABLE ") {
            let after = &line[upper.find("TABLE ").unwrap_or(0) + "TABLE ".len()..];
            let after = after.strip_prefix("IF NOT EXISTS ").unwrap_or(after);
            let name = after
                .split(|c: char| c == '(' || c.is_whitespace())
                .next()
                .unwrap_or_default()
                .replace(['"', '`', '[', ']'], "");
            if !name.is_empty() {
                tables.push((name, Vec::new()));
                open = !line.contains(");");
            }
        } else if open && line.starts_with(')') {
            open = false;
        } else if open {
            let first = line.split_whitespace().next().unwrap_or_default();
            if !first.is_empty()
                && !NOT_COLUMNS.contains(&first.to_uppercase().as_str())
                && !first.starts_with("--")
            {
                if let Some((_, columns)) = tables.last_mut() {
                    columns.push(first.trim_matches(['"', '`', ',', '[', ']']).to_string());
                }
            }
        }
    }
    tables
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::sources::SourceRule;
    use crate::walker::FileKind;
    use std::fs;
    use std::path::PathBuf;

    fn entry(path: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            kind: FileKind::Other,
            size_bytes: 1,
        }
    }

    fn map(kind: SourceKind, glob: &str, format: SourceFormat) -> SourceMap {
        SourceMap {
            rules: vec![SourceRule {
                kind,
                glob: glob.to_string(),
                format,
            }],
        }
    }

    fn schema_of(path: &str, content: &str, format: SourceFormat) -> Vec<Signal> {
        let dir = tempfile::tempdir().unwrap();
        let abs = dir.path().join(path);
        fs::create_dir_all(abs.parent().unwrap()).unwrap();
        fs::write(abs, content).unwrap();
        schema_tables(
            dir.path(),
            &[entry(path)],
            &map(SourceKind::Schema, path, format),
        )
    }

    #[test]
    fn reads_an_active_record_schema() {
        let signals = schema_of(
            "db/schema.rb",
            "ActiveRecord::Schema.define do\n  create_table \"contracts\", force: :cascade do |t|\n    t.string \"title\"\n    t.bigint \"company_id\", null: false\n    t.index [\"company_id\"], name: \"idx\"\n  end\n  create_table :signatures do |t|\n    t.datetime :signed_at\n  end\n  add_foreign_key \"contracts\", \"companies\"\nend\n",
            SourceFormat::RailsSchema,
        );
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].kind, SignalKind::Schema);
        assert_eq!(signals[0].origin, "db/schema.rb");
        assert_eq!(
            signals[0].text,
            "contracts: title, company_id\nsignatures: signed_at"
        );
    }

    #[test]
    fn reads_sql_ddl_wherever_it_lives() {
        let signals = schema_of(
            "any/where/model.sql",
            "CREATE TABLE public.orders (\n    id bigint NOT NULL,\n    \"total\" numeric(10,2),\n    `note` text,\n    CONSTRAINT positive CHECK ((total > 0))\n);\n\nCREATE TABLE IF NOT EXISTS `audit`.log (id int);\nCREATE INDEX idx ON public.orders (id);\n",
            SourceFormat::SqlDdl,
        );
        assert_eq!(
            signals[0].text,
            "public.orders: id, total, note\naudit.log:"
        );
    }

    #[test]
    fn files_outside_the_map_and_empty_schemas_give_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("schema.rb"), "# nothing\n").unwrap();
        fs::write(dir.path().join("other.sql"), "CREATE TABLE t (id int);").unwrap();
        let sources = map(SourceKind::Schema, "schema.rb", SourceFormat::RailsSchema);
        assert_eq!(
            schema_tables(
                dir.path(),
                &[entry("schema.rb"), entry("other.sql")],
                &sources
            ),
            vec![]
        );
    }

    #[test]
    fn migrations_are_named_in_words_oldest_first() {
        let sources = map(SourceKind::Migrations, "**/*", SourceFormat::FileNames);
        let signal = migration_names(
            &[
                entry("db/migrate/20260429130500_add_lookup_index_to_companies.rb"),
                entry("db/migrate/20260423000000_add_copy_pending_to_annexes.rb"),
                entry("db/migration/V2__create_orders.sql"),
                entry("db/migration/V10__create_invoices.sql"),
                entry("alembic/versions/ab12cd34_add_users.py"),
                entry("api/migrations/0001_initial.py"),
                entry("api/migrations/__init__.py"),
            ],
            &sources,
        )
        .unwrap();
        assert_eq!(signal.kind, SignalKind::Migrations);
        assert_eq!(
            signal.text,
            "initial\ncreate orders\ncreate invoices\nadd copy pending to annexes\nadd lookup index to companies\nadd users"
        );
        assert!(migration_names(&[], &sources).is_none());
    }
}
