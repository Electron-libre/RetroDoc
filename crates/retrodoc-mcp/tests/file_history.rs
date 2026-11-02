//! The history `git_log` gives of a cited file across its renames, asked of
//! `SourceAccess` over a git repository built by the test.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use retrodoc_mcp::source::SourceAccess;

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "Ada Lovelace")
        .env("GIT_AUTHOR_EMAIL", "ada@example.com")
        .env("GIT_COMMITTER_NAME", "Ada Lovelace")
        .env("GIT_COMMITTER_EMAIL", "ada@example.com")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn write(root: &Path, name: &str, content: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn commit_all(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
}

/// Twenty distinct lines, so that a one-line edit keeps the file similar.
fn code(extra: &str) -> String {
    let mut code = String::new();
    for n in 1..=20 {
        let _ = writeln!(code, "invoice line {n}");
    }
    code.push_str(extra);
    code
}

/// `old.rb` committed, renamed to `new.rb` as is, edited, renamed to
/// `app/new.rb` with an edit, edited again; `other.rb` is changed beside it.
fn renamed_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    write(root, "old.rb", &code(""));
    write(root, "other.rb", "other\n");
    commit_all(root, "add invoices");
    git(root, &["mv", "old.rb", "new.rb"]);
    commit_all(root, "rename invoices");
    write(root, "new.rb", &code("a\n"));
    write(root, "other.rb", "other 2\n");
    commit_all(root, "edit invoices");
    std::fs::remove_file(root.join("new.rb")).unwrap();
    write(root, "app/new.rb", &code("a\nb\n"));
    commit_all(root, "move and edit invoices");
    write(root, "app/new.rb", &code("a\nb\nc\n"));
    commit_all(root, "edit invoices again");
    dir
}

fn subjects(log: &str) -> Vec<&str> {
    log.lines()
        .filter(|line| line.starts_with("- `"))
        .map(|line| line.split(" · ").nth(2).unwrap())
        .collect()
}

// spec: FHI-1
#[test]
fn the_history_of_a_renamed_file_goes_on_under_its_previous_names() {
    let dir = renamed_repo();
    let access = SourceAccess::new(dir.path(), ["app/new.rb".to_string()]);
    let log = access.git_log("app/new.rb", 10).unwrap();
    assert_eq!(
        subjects(&log),
        [
            "edit invoices again",
            "move and edit invoices (renamed from `new.rb`)",
            "edit invoices",
            "rename invoices (renamed from `old.rb`)",
            "add invoices",
        ],
        "{log}"
    );
    let short = access.git_log("app/new.rb", 4).unwrap();
    assert_eq!(subjects(&short).len(), 4, "{short}");
    assert_eq!(
        subjects(&short)[3],
        "rename invoices (renamed from `old.rb`)"
    );
}

// spec: FHI-2
#[test]
fn only_the_commits_that_renamed_the_file_are_marked() {
    let dir = renamed_repo();
    let access = SourceAccess::new(dir.path(), ["app/new.rb".to_string()]);
    let commits = access.commits("app/new.rb", 10).unwrap();
    let renamed: Vec<_> = commits
        .iter()
        .map(|commit| commit.renamed_from.as_deref())
        .collect();
    assert_eq!(
        renamed,
        [None, Some("new.rb"), None, Some("old.rb"), None],
        "{commits:?}"
    );
}

// spec: FHI-3
#[test]
fn a_copy_starts_its_own_history() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    write(root, "a.rb", &code(""));
    commit_all(root, "add a");
    write(root, "a.rb", &code("a\n"));
    commit_all(root, "edit a");
    write(root, "b.rb", &code("a\n"));
    commit_all(root, "copy a to b");
    let access = SourceAccess::new(root, ["b.rb".to_string()]);
    let log = access.git_log("b.rb", 10).unwrap();
    assert_eq!(subjects(&log), ["copy a to b"], "{log}");
}

// spec: FHI-4
#[test]
fn a_previous_name_is_not_readable() {
    let dir = renamed_repo();
    let access = SourceAccess::new(dir.path(), ["app/new.rb".to_string()]);
    for path in ["old.rb", "new.rb"] {
        let log = access.git_log(path, 5).unwrap_err();
        assert!(log.contains("not cited"), "{log}");
        let read = access.read_source(path, None, None).unwrap_err();
        assert!(read.contains("not cited"), "{read}");
    }
}

// spec: FHI-5
#[test]
fn a_merge_is_not_a_commit_of_the_history() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    write(root, "a.rb", &code(""));
    commit_all(root, "add a");
    git(root, &["checkout", "-q", "-b", "side"]);
    git(root, &["mv", "a.rb", "b.rb"]);
    commit_all(root, "rename a on the side");
    git(root, &["checkout", "-q", "main"]);
    write(root, "c.rb", "c\n");
    commit_all(root, "add c");
    git(
        root,
        &["merge", "-q", "--no-ff", "-m", "merge side", "side"],
    );
    let access = SourceAccess::new(root, ["b.rb".to_string()]);
    let log = access.git_log("b.rb", 10).unwrap();
    assert_eq!(
        subjects(&log),
        ["rename a on the side (renamed from `a.rb`)", "add a"],
        "{log}"
    );
}
