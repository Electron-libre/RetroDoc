//! Repo walk respecting `.gitignore` (+ additional patterns from
//! `retrodoc.toml`), rough classification of encountered files.

use std::path::{Path, PathBuf};

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;

use crate::error::IngestError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Markdown,
    Source,
    /// Source code that is a test (by directory or file name): kept out of
    /// the functional pipeline, where test scaffolding and expected-failure
    /// fixtures would otherwise be documented as product behavior.
    Test,
    Other,
}

/// Languages whose test files are named `FooTest` or `FooTests`.
const CAPITAL_TEST_SUFFIX: &[&str] =
    &["java", "kt", "kts", "groovy", "cs", "php", "scala", "swift"];

/// Directory names that mark everything below them as tests.
const TEST_DIRS: &[&str] = &["tests", "test", "spec", "specs", "__tests__", "e2e"];

impl FileKind {
    fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("md" | "mdx") => FileKind::Markdown,
            _ if is_build_script(path) => FileKind::Other,
            Some(
                "rs" | "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" | "py" | "go"
                | "java" | "kt" | "kts" | "rb" | "php" | "c" | "h" | "cc" | "cpp" | "hpp" | "cs"
                | "swift" | "scala" | "sql",
            ) => {
                if is_test_path(path) {
                    FileKind::Test
                } else {
                    FileKind::Source
                }
            }
            _ => FileKind::Other,
        }
    }
}

/// Turns the [`FileKind::Other`] files whose extension is in `extensions`
/// (with or without the dot, case-insensitive) into [`FileKind::Source`], or
/// [`FileKind::Test`] by the usual test-path heuristic. The built-in list of
/// [`FileKind::from_path`] only knows common languages; this lets the stack
/// identification add the others. Returns the number of files promoted.
pub fn promote_to_source(files: &mut [FileEntry], extensions: &[String]) -> usize {
    let wanted: Vec<String> = extensions
        .iter()
        .map(|e| e.trim().trim_start_matches('.').to_lowercase())
        .filter(|e| !e.is_empty())
        .collect();
    let sbt = is_sbt_build(files);
    let mut promoted = 0;
    for file in files.iter_mut().filter(|f| {
        f.kind == FileKind::Other && !is_build_script(&f.path) && !(sbt && in_sbt_project(&f.path))
    }) {
        let matches = file
            .path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| wanted.contains(&e.to_lowercase()));
        if matches {
            file.kind = if is_test_path(&file.path) {
                FileKind::Test
            } else {
                FileKind::Source
            };
            promoted += 1;
        }
    }
    promoted
}

/// A build script written in a language of the reference set
/// (`build.gradle.kts`, `settings.gradle.kts`, `Package.swift`): a manifest,
/// not code of the application.
fn is_build_script(path: &Path) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy())
        .is_some_and(|n| n.ends_with(".gradle.kts") || n == "Package.swift")
}

/// An sbt build: a `build.sbt` at the root, whose `project/` directory holds
/// build code (`Dependencies.scala`), not code of the application.
fn is_sbt_build(files: &[FileEntry]) -> bool {
    files.iter().any(|f| f.path == Path::new("build.sbt"))
}

fn in_sbt_project(path: &Path) -> bool {
    path.starts_with("project")
}

/// Heuristic test detection on a repo-relative path: a test directory
/// anywhere in it, or a conventional test file name (`foo_test.go`,
/// `foo.spec.ts`, `test_foo.py`, `foo_spec.rb`, `FooTest.java`,
/// `FooTests.cs`).
fn is_test_path(path: &Path) -> bool {
    let dirs: Vec<String> = path
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let in_test_dir = dirs.iter().enumerate().any(|(i, dir)| {
        TEST_DIRS.contains(&dir.to_lowercase().as_str())
            || is_test_project_dir(dir)
            || (i > 0 && dirs[i - 1] == "src" && is_test_source_set(dir))
    });
    if in_test_dir {
        return true;
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = stem.to_lowercase();
    ["_test", "_spec", ".test", ".spec"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
        || lower.starts_with("test_")
        || (has_capital_test_suffix(&stem) && uses_capital_test_suffix(path))
}

/// A .NET test project (`Shop.Tests`, `Shop.UnitTests`, `ShopIntegrationTests`):
/// the capital `Tests` after a word or a dot.
fn is_test_project_dir(dir: &str) -> bool {
    dir.strip_suffix("Tests")
        .and_then(|w| w.chars().last())
        .is_some_and(|c| c.is_alphanumeric() || c == '.')
}

/// A Gradle test source set under `src/`: `integrationTest`, `functionalTest`,
/// `testFixtures`.
fn is_test_source_set(dir: &str) -> bool {
    dir == "testFixtures"
        || dir
            .strip_suffix("Test")
            .and_then(|w| w.chars().last())
            .is_some_and(char::is_alphanumeric)
}

fn uses_capital_test_suffix(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| CAPITAL_TEST_SUFFIX.contains(&e.as_str()))
}

/// `OrderTest`, `OrderTests`: the capital `Test` after a word, so `Contest`
/// and a bare `Test` are not tests.
fn has_capital_test_suffix(stem: &str) -> bool {
    let word = stem
        .strip_suffix("Tests")
        .or_else(|| stem.strip_suffix("Test"));
    word.and_then(|w| w.chars().last())
        .is_some_and(char::is_alphanumeric)
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Path relative to the repo root.
    pub path: PathBuf,
    pub kind: FileKind,
    pub size_bytes: u64,
}

/// Walks `repo_root` respecting `.gitignore`, `.ignore`, and the extra
/// patterns supplied (gitignore syntax, e.g. `"*.lock"`, `"vendor/"`).
///
/// # Errors
///
/// Returns an error if `repo_root` doesn't exist, an exclusion pattern is
/// invalid, or walking the repo fails.
pub fn walk_repo(repo_root: &Path, extra_ignore: &[String]) -> Result<Vec<FileEntry>, IngestError> {
    if !repo_root.exists() {
        return Err(IngestError::InvalidRepo(repo_root.to_path_buf()));
    }

    let mut overrides = OverrideBuilder::new(repo_root);
    for pattern in extra_ignore {
        // An "!pattern" override excludes; that's the reverse of raw
        // gitignore syntax, so we prefix it ourselves with "!" to expose
        // plain gitignore syntax on the user config side.
        let negated = format!("!{pattern}");
        overrides
            .add(&negated)
            .map_err(|source| IngestError::InvalidIgnorePattern {
                pattern: pattern.clone(),
                source,
            })?;
    }
    // Always ignore RetroDoc's own internal metadata.
    overrides
        .add("!.retrodoc/")
        .map_err(|source| IngestError::InvalidIgnorePattern {
            pattern: ".retrodoc/".to_string(),
            source,
        })?;
    let overrides = overrides
        .build()
        .map_err(|source| IngestError::InvalidIgnorePattern {
            pattern: "<build>".to_string(),
            source,
        })?;

    let walker = WalkBuilder::new(repo_root)
        .hidden(false) // we want to see .github/, .gitlab-ci.yml etc.; only .git is excluded below
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .overrides(overrides)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();

    let mut entries = Vec::new();
    for result in walker {
        let entry = result?;
        let path = entry.path();
        let is_file = entry.file_type().is_some_and(|t| t.is_file());
        if !is_file {
            continue;
        }
        let relative = path.strip_prefix(repo_root).unwrap_or(path).to_path_buf();
        let size_bytes = entry.metadata().map_or(0, |m| m.len());
        entries.push(FileEntry {
            kind: FileKind::from_path(&relative),
            path: relative,
            size_bytes,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    if is_sbt_build(&entries) {
        for entry in entries
            .iter_mut()
            .filter(|e| in_sbt_project(&e.path) && e.kind != FileKind::Markdown)
        {
            entry.kind = FileKind::Other;
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn walk_respects_gitignore_and_extra_ignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // `.gitignore` is only applied by the `ignore` crate inside a real
        // git repo (`require_git` defaults to true): initialize one here.
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .unwrap();
        fs::write(root.join(".gitignore"), "ignored_by_git.txt\n").unwrap();
        fs::write(root.join("ignored_by_git.txt"), "x").unwrap();
        fs::write(root.join("ignored_by_config.lock"), "x").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("README.md"), "# hi").unwrap();

        let entries = walk_repo(root, &["*.lock".to_string()]).unwrap();
        let paths: Vec<_> = entries
            .iter()
            .map(|e| e.path.to_string_lossy().to_string())
            .collect();

        assert!(paths.contains(&"main.rs".to_string()));
        assert!(paths.contains(&"README.md".to_string()));
        assert!(!paths.contains(&"ignored_by_git.txt".to_string()));
        assert!(!paths.contains(&"ignored_by_config.lock".to_string()));
    }

    #[test]
    fn classifies_file_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("lib.rs"), "").unwrap();
        fs::write(root.join("notes.md"), "").unwrap();
        fs::write(root.join("data.bin"), "").unwrap();

        let entries = walk_repo(root, &[]).unwrap();
        let kind_of = |name: &str| {
            entries
                .iter()
                .find(|e| e.path == Path::new(name))
                .map(|e| e.kind)
        };
        assert_eq!(kind_of("lib.rs"), Some(FileKind::Source));
        assert_eq!(kind_of("notes.md"), Some(FileKind::Markdown));
        assert_eq!(kind_of("data.bin"), Some(FileKind::Other));
    }

    #[test]
    fn promotes_listed_extensions_to_source() {
        let entry = |path: &str, kind| FileEntry {
            path: PathBuf::from(path),
            kind,
            size_bytes: 0,
        };
        let mut files = vec![
            entry("lib/a.ex", FileKind::Other),
            entry("test/a_test.ex", FileKind::Other),
            entry("web/b.EXS", FileKind::Other),
            entry("app/show.html.erb", FileKind::Other),
            entry("lib/c.rs", FileKind::Source),
            entry("build.gradle.kts", FileKind::Other),
        ];
        let promoted = promote_to_source(
            &mut files,
            &[".ex".into(), "exs".into(), "kts".into(), " ".into()],
        );
        let kinds: Vec<_> = files.iter().map(|f| f.kind).collect();
        assert_eq!(promoted, 3);
        assert_eq!(
            kinds,
            vec![
                FileKind::Source,
                FileKind::Test,
                FileKind::Source,
                FileKind::Other,
                FileKind::Source,
                FileKind::Other
            ]
        );
    }

    // spec: LNG-14 (internal: the roles call that promotes extensions needs an LLM through the binary)
    #[test]
    fn the_sbt_project_directory_is_not_promoted() {
        let entry = |path: &str| FileEntry {
            path: PathBuf::from(path),
            kind: FileKind::Other,
            size_bytes: 0,
        };
        let mut sbt = vec![
            entry("build.sbt"),
            entry("project/Build.scala"),
            entry("app/A.scala"),
        ];
        assert_eq!(promote_to_source(&mut sbt, &["scala".into()]), 1);
        assert_eq!(sbt[1].kind, FileKind::Other);
        let mut plain = vec![entry("project/Build.scala")];
        assert_eq!(promote_to_source(&mut plain, &["scala".into()]), 1);
    }

    #[test]
    fn classifies_tests_apart_from_source() {
        for test in [
            "tests/api.rs",
            "tests/trybuild/fail/x.rs",
            "src/__tests__/a.ts",
            "spec/models/user_spec.rb",
            "pkg/handler_test.go",
            "web/app.spec.ts",
            "tools/test_parser.py",
            "src/main/java/shop/OrderTest.java",
            "Shop/OrderTests.cs",
            "Shop.Tests/Fakes/FakeClock.cs",
            "ShopTests/Order.swift",
            "core/src/integrationTest/kotlin/OrderIT.kt",
            "src/testFixtures/java/OrderBuilder.java",
        ] {
            assert_eq!(
                FileKind::from_path(Path::new(test)),
                FileKind::Test,
                "{test}"
            );
        }
        for source in [
            "src/lib.rs",
            "src/testing_utils.rs",
            "app/latest.py",
            "src/contest/a.rs",
            "src/main/java/shop/Contest.java",
            "src/main/java/shop/Test.java",
            "src/main/kotlin/Order.kts",
            "src/components/SpeedTest.tsx",
            "src/main/java/shop/Contests/Draw.java",
            "src/Tests.Api/Order.cs",
            "lib/integrationTest/a.kt",
        ] {
            assert_eq!(
                FileKind::from_path(Path::new(source)),
                FileKind::Source,
                "{source}"
            );
        }
        for build_script in ["build.gradle.kts", "Package.swift"] {
            assert_eq!(
                FileKind::from_path(Path::new(build_script)),
                FileKind::Other,
                "{build_script}"
            );
        }
        // Non-code files under a test dir stay non-code.
        assert_eq!(
            FileKind::from_path(Path::new("tests/test_api.yml")),
            FileKind::Other
        );
    }
}
