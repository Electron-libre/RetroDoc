#!/usr/bin/env rust-script
//! Behavior test for `require_code_review.rs`: runs `record` and `check` against a throwaway git repository.
//! Usage: `just test-harness` (or `rust-script .claude/hooks/test_require_code_review.rs`).
//!
//! ```cargo
//! [dependencies]
//! serde_json = "1"
//! ```

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Exit code and stderr of the hook in `mode` for `event`, with `project` as the project directory.
fn run(hook: &Path, mode: &str, project: &Path, event: &serde_json::Value) -> (i32, String) {
    let mut child = Command::new("rust-script")
        .arg(hook)
        .arg(mode)
        .env("CLAUDE_PROJECT_DIR", project)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rust-script must be installed");
    child.stdin.take().unwrap().write_all(event.to_string().as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn review_done(hook: &Path, project: &Path, skill: &str) {
    let event = serde_json::json!({
        "tool_name": "Skill", "cwd": project, "tool_input": {"skill": skill}, "tool_response": {"success": true}
    });
    assert_eq!(run(hook, "record", project, &event).0, 0);
}

fn commit_attempt(hook: &Path, project: &Path, command: &str) -> (i32, String) {
    let event = serde_json::json!({"tool_name": "Bash", "cwd": project, "tool_input": {"command": command}});
    run(hook, "check", project, &event)
}

fn sh(project: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .current_dir(project)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed");
}

// spec: PAR-9
/// A review recorded in one worktree must not cover another: the marker and the waiver live in the git directory
/// of each worktree.
fn git_dir_of(dir: &Path) -> std::path::PathBuf {
    let out = Command::new("git").args(["rev-parse", "--absolute-git-dir"]).current_dir(dir).output().unwrap();
    Path::new(String::from_utf8_lossy(&out.stdout).trim()).to_path_buf()
}

// spec: REV-15
fn worktree_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let root = std::env::temp_dir().join(format!("reviewtest-wt-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let main = root.join("main");
    let linked = root.join("linked");
    fs::create_dir_all(&main).unwrap();
    sh(&main, &["init", "-q", "."]);
    fs::write(main.join("a.txt"), "one\n").unwrap();
    sh(&main, &["add", "-A"]);
    sh(&main, &["commit", "-qm", "init"]);
    sh(&main, &["worktree", "add", "-q", "-b", "other", linked.to_str().unwrap()]);
    let commit = "git commit -am x";
    fs::write(main.join("a.txt"), "two\n").unwrap();
    fs::write(linked.join("a.txt"), "two\n").unwrap();

    review_done(hook, &main, "code-review");
    let mut out = vec![
        ("a review in the main worktree covers it", commit_attempt(hook, &main, commit).0 == 0),
        ("the same diff in a linked worktree is still unreviewed", commit_attempt(hook, &linked, commit).0 == 2),
    ];
    review_done(hook, &linked, "code-review");
    out.push(("a review in a linked worktree covers it", commit_attempt(hook, &linked, commit).0 == 0));

    // the path the instructions give the user, `git rev-parse --git-path`, is the one the hook reads from a linked worktree
    let name = ["claude-review", "waived"].join("-");
    let given = Command::new("git").args(["rev-parse", "--git-path", &name]).current_dir(&linked).output().unwrap();
    let given = linked.join(String::from_utf8_lossy(&given.stdout).trim());
    out.push(("the given path is not the waiver of the main worktree", given != git_dir_of(&main).join(&name)));
    let waiver = git_dir_of(&linked).join("claude-review-waived");
    out.push(("the given path is the waiver of the linked worktree", given.parent().unwrap().canonicalize().unwrap() == waiver.parent().unwrap().canonicalize().unwrap()));
    fs::write(&given, "").unwrap();
    out.push(("a waiver of the linked worktree lets its commit through", commit_attempt(hook, &linked, commit).0 == 0));
    out.push(("the waiver stays while no commit happened", waiver.exists()));
    sh(&linked, &["commit", "-qam", "x"]);
    fs::write(linked.join("new.txt"), "x\n").unwrap();
    let main_waiver = git_dir_of(&main).join("claude-review-waived");
    fs::write(&main_waiver, "").unwrap();
    out.push(("the waiver of the main worktree does not cover the linked one", commit_attempt(hook, &linked, commit).0 == 2));
    out.push(("the commit made consumed the waiver of the linked worktree", !waiver.exists()));
    out.push(("the linked worktree leaves the main waiver alone", main_waiver.exists()));
    let _ = fs::remove_dir_all(&root);
    out
}

/// Two repositories side by side: `current` reviewed and unchanged, `other` with a change nobody reviewed.
/// Returns the shell directory (`current`).
fn target_fixture(hook: &Path, name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("reviewtest-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (current, other) = (root.join("current"), root.join("other"));
    for repo in [&current, &other] {
        fs::create_dir_all(repo).unwrap();
        sh(repo, &["init", "-q", "."]);
        fs::write(repo.join("a.txt"), "one\n").unwrap();
        sh(repo, &["add", "-A"]);
        sh(repo, &["commit", "-qm", "init"]);
        fs::write(repo.join("a.txt"), "two\n").unwrap();
    }
    review_done(hook, &current, "code-review");
    (current, other)
}

/// The commands of `commands` run from `shell` must all be refused.
fn all_refused(hook: &Path, shell: &Path, commands: &[&'static str]) -> Vec<(&'static str, bool)> {
    commands.iter().map(|c| (*c, commit_attempt(hook, shell, c).0 == 2)).collect()
}

/// Exit code and stderr of `native` run in `cwd` with `env` added, as git runs a `pre-commit` hook: no event on stdin.
fn native_run(hook: &Path, cwd: &Path, env: &[(&str, &Path)]) -> (i32, String) {
    let out = Command::new("rust-script")
        .arg(hook)
        .arg("native")
        .current_dir(cwd)
        .envs(env.iter().map(|(k, v)| (*k, *v)))
        .env_remove("CLAUDE_PROJECT_DIR")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("rust-script must be installed");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stderr).into_owned())
}

// spec: REV-9, REV-10
fn native_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, other) = target_fixture(hook, "rev9");
    let (code, stderr) = native_run(hook, &other, &[]);
    let mut out = vec![
        ("native refuses an unreviewed repository with a message naming the review", code != 0 && stderr.contains("code-review")),
        ("native lets through a reviewed repository", native_run(hook, &current, &[]).0 == 0),
    ];
    // git resolved the repository: the text of the command is not involved
    let (git_dir, tree) = (other.join(".git"), other.clone());
    out.push(("native judges the repository named by GIT_DIR and GIT_WORK_TREE", native_run(hook, &current, &[("GIT_DIR", &git_dir), ("GIT_WORK_TREE", &tree)]).0 != 0));
    review_done(hook, &other, "code-review");
    fs::write(current.join("new.txt"), "x\n").unwrap();
    out.push(("native lets through a reviewed GIT_DIR while the current directory is unreviewed", native_run(hook, &current, &[("GIT_DIR", &git_dir), ("GIT_WORK_TREE", &tree)]).0 == 0));
    out.push(("native refuses the unreviewed current directory", native_run(hook, &current, &[]).0 != 0));
    let waiver = current.join(".git").join(["claude-review", "waived"].join("-"));
    fs::write(&waiver, "").unwrap();
    out.push(("native honours the waiver", native_run(hook, &current, &[]).0 == 0));
    fs::remove_file(&waiver).unwrap();
    fs::remove_file(current.join("new.txt")).unwrap();
    fs::write(current.join("a.txt"), "one\n").unwrap();
    out.push(("native lets through a repository with nothing to commit", native_run(hook, &current, &[]).0 == 0));
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-9, REV-10
/// The hook as git runs it: installed as `pre-commit`, it refuses a real commit nobody reviewed and lets the reviewed one through.
fn native_hook_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    use std::os::unix::fs::PermissionsExt;
    let (current, _other) = target_fixture(hook, "rev9git");
    let pre_commit = current.join(".git/hooks/pre-commit");
    fs::write(&pre_commit, format!("#!/bin/sh\nexec rust-script '{}' native\n", hook.display())).unwrap();
    fs::set_permissions(&pre_commit, fs::Permissions::from_mode(0o755)).unwrap();
    let commit = |args: &[&str]| {
        Command::new("git")
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args(args)
            .current_dir(&current)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    };
    let commits = || {
        let log = Command::new("git").args(["rev-list", "--count", "HEAD"]).current_dir(&current).output().unwrap();
        String::from_utf8_lossy(&log.stdout).trim().parse::<u32>().unwrap()
    };
    let before = commits();
    let reviewed = commit(&["commit", "-qam", "x"]) && commits() == before + 1;
    fs::write(current.join("a.txt"), "three\n").unwrap();
    let refused = !commit(&["commit", "-qam", "y"]);
    let out = vec![
        ("git runs the native hook and a reviewed commit -a passes", reviewed),
        ("git refuses through the native hook a commit nobody reviewed", refused),
        ("the refused commit left the history alone", commits() == before + 1),
    ];
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-11
fn native_worktree_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let root = std::env::temp_dir().join(format!("reviewtest-rev11-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (main, linked) = (root.join("main"), root.join("linked"));
    fs::create_dir_all(&main).unwrap();
    sh(&main, &["init", "-q", "."]);
    fs::write(main.join("a.txt"), "one\n").unwrap();
    sh(&main, &["add", "-A"]);
    sh(&main, &["commit", "-qm", "init"]);
    sh(&main, &["worktree", "add", "-q", "-b", "other", linked.to_str().unwrap()]);
    fs::write(main.join("a.txt"), "two\n").unwrap();
    fs::write(linked.join("a.txt"), "two\n").unwrap();
    review_done(hook, &linked, "code-review");
    let linked_git = git_dir_of(&linked);
    let out = vec![
        ("native lets through a reviewed linked worktree", native_run(hook, &linked, &[("GIT_DIR", &linked_git)]).0 == 0),
        ("native refuses the unreviewed main worktree", native_run(hook, &main, &[]).0 != 0),
    ];
    let _ = fs::remove_dir_all(&root);
    out
}

// spec: REV-12
/// What skips the native hook is refused on a commit line even when the repository is reviewed; the same words in a
/// message, or in a command that is not a commit, are not.
fn skipped_hook_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, _other) = target_fixture(hook, "rev12");
    let refused = [
        "git commit -n -m x",
        "git commit --no-verify -m x",
        "git commit -m x --no-verify",
        "git commit --no-ver -m x",
        "git commit -nm x",
        "git commit -anm x",
        "git commit -m x -n",
        "git -C . commit -n -m x",
        "git commit -m 'a b' --no-verify",
        "git -c core.hooksPath=/dev/null commit -m x",
        "git -c core.hookspath=/dev/null commit -m x",
        "git --config-env=core.hooksPath=H commit -m x",
        "git config core.hooksPath /dev/null",
        "git config --local core.hooksPath /dev/null",
        "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.hooksPath GIT_CONFIG_VALUE_0=/x git commit -m x",
        "git commit -m x && git commit -n -m y",
        "git commit -m a; git commit --no-verify -m b",
        "git config --get a && git -c core.hooksPath=/x commit -m x",
        "git -c core.hooksPath=/x commit -m x; echo --get",
        "git commit -m x --trailer 'a: b' -n",
    ];
    let allowed = [
        "git commit -am x",
        "git commit -m 'fix -n'",
        "git commit -m \"fix --no-verify\"",
        "git commit -mn",
        "git commit -m x -- -n",
        "git commit -m \"document core.hooksPath\"",
        "git commit --message='core.hooksPath' -a",
        "git config --get core.hooksPath",
        "git config core.hooksPath",
        "git config --unset core.hooksPath",
        "git log --grep core.hooksPath",
        "git commit -m x --trailer -n",
        "git commit -F - <<EOF\ncore.hooksPath is documented here\nEOF",
        "git log -n 5 --grep commit",
        "git status -n",
    ];
    let mut out: Vec<(&'static str, bool)> = refused
        .iter()
        .map(|c| (*c, { let (code, stderr) = commit_attempt(hook, &current, c); code == 2 && stderr.contains("native") }))
        .collect();
    out.extend(allowed.iter().map(|c| (*c, commit_attempt(hook, &current, c).0 == 0)));
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-1
/// The decision is the one of the repository the commit targets, whichever of the two is reviewed.
fn target_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, other) = target_fixture(hook, "rev1");
    let mut out = vec![
        ("a commit in the reviewed shell repository passes", commit_attempt(hook, &current, "git commit -am x").0 == 0),
        ("a commit aimed at an unreviewed repository is refused", commit_attempt(hook, &current, "git -C ../other commit -am x").0 == 2),
    ];
    review_done(hook, &other, "code-review");
    fs::write(current.join("new.txt"), "x\n").unwrap();
    out.push(("a commit aimed at a reviewed repository passes while the shell one is not", commit_attempt(hook, &current, "git -C ../other commit -am x").0 == 0));
    out.push(("a commit in the unreviewed shell repository is refused", commit_attempt(hook, &current, "git commit -am x").0 == 2));
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-16
/// A `cd` before the commit moves the shell directory the commit is judged from, relative or absolute, whatever the
/// separator; a subshell gives it back; a target the text does not give is refused.
fn cd_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, other) = target_fixture(hook, "rev16");
    let absolute: &'static str = Box::leak(format!("cd {} && git commit -am x", other.display()).into_boxed_str());
    let quoted: &'static str = Box::leak(format!("cd '{}' && git commit -am x", other.display()).into_boxed_str());
    let mut out = all_refused(
        hook,
        &current,
        &[
            "cd ../other && git commit -am x",
            "cd ../other; git commit -am x",
            "cd ../other\ngit commit -am x",
            "(cd ../other && git commit -am x)",
            "cd .. && cd other && git commit -am x",
            "cd .. && git -C other commit -am x",
            "cd ../other && git -C . commit -am x",
            "cd .. && git --work-tree other commit -am x",
            "pushd ../other && git commit -am x",
            "cd -- ../other && git commit -am x",
            "cd ../other && git status && git commit -am x",
            absolute,
            quoted,
            "cd $X && git commit -am x",
            "cd \"$(mktemp -d)\" && git commit -am x",
            "cd ~/other && git commit -am x",
            "cd - && git commit -am x",
            "cd && git commit -am x",
            "if cd ../other; then git commit -am x; fi",
            "builtin cd ../other && git commit -am x",
            "popd && git commit -am x",
            "pushd +1 && git commit -am x",
        ],
    );
    for (name, command) in [
        ("a cd inside a subshell is undone at its end", "(cd ../other && true); git commit -am x"),
        ("a cd of a commit-free line leaves the shell repository judged", "cd ../other && git status; cd ../current && git commit -am x"),
        ("a cd back to the reviewed repository passes", "cd ../other && cd ../current && git commit -am x"),
        ("a cd run in the background does not move the shell", "cd ../other & git commit -am x"),
        ("a cd of a pipeline does not move the shell", "cd ../other | cat; git commit -am x"),
        ("a cd in a backtick substitution does not move the shell", "d=`cd ../other && pwd`; git commit -am x"),
        ("an unreadable cd of a finished subshell is forgotten", "(cd $X && true); git commit -am x"),
        ("a word cd in a message is no cd", "git commit -m 'cd ../other' -a"),
    ] {
        out.push((name, commit_attempt(hook, &current, command).0 == 0));
    }
    review_done(hook, &other, "code-review");
    fs::write(current.join("new.txt"), "x\n").unwrap();
    out.push(("a cd to a reviewed repository passes while the shell one is not", commit_attempt(hook, &current, "cd ../other && git commit -am x").0 == 0));
    out.push(("a commit without cd is still refused in the unreviewed shell repository", commit_attempt(hook, &current, "git commit -am x").0 == 2));
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-2
/// Several `-C` accumulate: `-C .. -C other` is `../other`.
fn accumulated_c_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, _) = target_fixture(hook, "rev2");
    let out = all_refused(hook, &current, &["git -C .. -C other commit -am x", "git -C .. -C 'other' commit -am x"]);
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-3
/// `--work-tree` names the repository, else `--git-dir` (its parent when it is a `.git`, itself otherwise);
/// the work tree wins.
fn work_tree_and_git_dir_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, _) = target_fixture(hook, "rev3");
    let out = all_refused(
        hook,
        &current,
        &[
            "git --work-tree ../other commit -am x",
            "git --git-dir ../other/.git commit -am x",
            "git --git-dir ../other commit -am x",
            "git --git-dir ../current/.git --work-tree ../other commit -am x",
            "git --work-tree ../other --git-dir ../current/.git commit -am x",
        ],
    );
    let mut out = out;
    // a linked worktree's git directory lies inside the `.git` of its repository
    sh(&current.parent().unwrap().join("other"), &["worktree", "add", "-q", "-b", "wt", "../wt"]);
    out.push(("a git directory inside .git targets its repository", commit_attempt(hook, &current, "git --git-dir ../other/.git/worktrees/wt commit -am x").0 == 2));
    // the reviewed repository as the target, the unreviewed one only named by the other option
    out.push(("a reviewed work tree wins over an unreviewed git directory", commit_attempt(hook, &current, "git --git-dir ../other/.git --work-tree ../current commit -am x").0 == 0));
    out.push(("a reviewed git directory passes", commit_attempt(hook, &current, "git --git-dir ../current/.git commit -am x").0 == 0));
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-4
/// Two words or `=value`, quoted or not.
fn option_forms_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, _) = target_fixture(hook, "rev4");
    // an unreviewed repository whose name holds a space: a quoted value spans two words of the command
    let spaced = current.parent().unwrap().join("my other");
    fs::create_dir_all(&spaced).unwrap();
    sh(&spaced, &["init", "-q", "."]);
    fs::write(spaced.join("a.txt"), "one\n").unwrap();
    sh(&spaced, &["add", "-A"]);
    sh(&spaced, &["commit", "-qm", "init"]);
    fs::write(spaced.join("a.txt"), "two\n").unwrap();
    let out = all_refused(
        hook,
        &current,
        &[
            "git --work-tree=../other commit -am x",
            "git --git-dir=../other/.git commit -am x",
            "git --work-tree '../other' commit -am x",
            "git --work-tree=\"../other\" commit -am x",
            "git --git-dir \"../other/.git\" commit -am x",
            "git --git-dir='../other/.git' --work-tree='../other' commit -am x",
            "git --work-tree=\"../my other\" commit -am x",
            "git --git-dir='../my other/.git' commit -am x",
            "git --work-tree \"../my other\" commit -am x",
        ],
    );
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-5
/// A relative option is taken from the directory the `-C` lead to, before or after them on the line, as `git` does
/// (the other tests: from the shell directory, `../other`).
fn relative_option_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, _) = target_fixture(hook, "rev5");
    let out = all_refused(
        hook,
        &current,
        &[
            "git -C ../other --git-dir .git commit -am x",
            "git --git-dir .git -C ../other commit -am x",
            "git -C .. --work-tree other commit -am x",
            "git --work-tree other -C .. commit -am x",
            "git --work-tree other -C . -C .. commit -am x",
        ],
    );
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-6
/// `--git-dir` alone: the files come from the shell directory, the history is the one of the git directory.
fn git_dir_alone_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, other) = target_fixture(hook, "rev6");
    let alone = "git --git-dir ../other/.git commit -am x";
    let mut out = vec![("an unreviewed git directory is refused", commit_attempt(hook, &current, alone).0 == 2)];
    review_done(hook, &other, "code-review");
    out.push(("both repositories reviewed pass", commit_attempt(hook, &current, alone).0 == 0));
    fs::write(current.join("new.txt"), "x\n").unwrap();
    out.push(("an unreviewed shell directory is refused", commit_attempt(hook, &current, alone).0 == 2));
    out.push(("with --work-tree only the work tree counts", commit_attempt(hook, &current, "git --git-dir ../other/.git --work-tree ../other commit -am x").0 == 0));
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-7
/// `GIT_DIR` and `GIT_WORK_TREE` written in front of `git` name the repository as the options do, which win over them.
fn environment_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, _) = target_fixture(hook, "rev7");
    let mut out = all_refused(
        hook,
        &current,
        &[
            "GIT_DIR=../other/.git GIT_WORK_TREE=../other git commit -am x",
            "GIT_WORK_TREE=../other git commit -am x",
            "GIT_DIR=../other/.git git commit -am x",
            "GIT_WORK_TREE='../other' git commit -am x",
            "env GIT_WORK_TREE=../other git commit -am x",
            "env -i GIT_DIR=../other/.git git commit -am x",
            "FOO=1 GIT_WORK_TREE=../other git commit -am x",
            "GIT_WORK_TREE=../other sudo git commit -am x",
            "GIT_DIR=../other/.git nohup git commit -am x",
            "export GIT_DIR=../other/.git GIT_WORK_TREE=../other; git commit -am x",
            "export GIT_WORK_TREE=../other && git commit -am x",
            "export FOO=1 GIT_WORK_TREE=../other; echo hi; git commit -am x",
            "GIT_WORK_TREE=../other; export GIT_WORK_TREE; git commit -am x",
            "declare -x GIT_WORK_TREE=../other; git commit -am x",
            "export GIT_WORK_TREE=../other\ngit commit -am x",
            "git \\\n  -C ../other commit -am x",
            "export GIT_WORK_TREE=../a; GIT_WORK_TREE=../other; git commit -am x",
        ],
    );
    // `unset` takes the export back; a plain assignment is no environment of `git`
    for (name, command) in [
        ("an unset takes the export back", "export GIT_WORK_TREE=../other; unset GIT_WORK_TREE; git commit -am x"),
        ("a shell variable that is not exported is no environment of git", "GIT_WORK_TREE=../other; git commit -am x"),
        ("env -u hides an exported variable", "export GIT_WORK_TREE=../other; env -u GIT_WORK_TREE git commit -am x"),
        ("an unset hides a variable exported afterwards by name", "GIT_WORK_TREE=../other; unset GIT_WORK_TREE; export GIT_WORK_TREE; git commit -am x"),
    ] {
        out.push((name, commit_attempt(hook, &current, command).0 == 0));
    }
    out.push(("an option wins over the variable", commit_attempt(hook, &current, "GIT_WORK_TREE=../other git --work-tree ../current commit -am x").0 == 0));
    out.push(("a variable naming the reviewed repository passes", commit_attempt(hook, &current, "GIT_WORK_TREE=../current git commit -am x").0 == 0));
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-7
/// A value that cannot be read from the text (`$X`, `$(…)`, backticks) is refused whatever the reviews; a literal one is not.
fn undeterminable_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, other) = target_fixture(hook, "rev7b");
    review_done(hook, &other, "code-review");
    let mut out = all_refused(
        hook,
        &current,
        &[
            "GIT_DIR=$X git commit -am x",
            "GIT_WORK_TREE=\"$HOME/o\" git commit -am x",
            "GIT_WORK_TREE=${X}/o git commit -am x",
            "export GIT_WORK_TREE=$(pwd)/../other; git commit -am x",
            "env GIT_DIR=`pwd`/x git commit -am x",
            "GIT_DIR=\"../other/.git\"$Y git commit -am x",
        ],
    );
    for (name, command) in [
        ("a single-quoted dollar is a literal value", "GIT_WORK_TREE='$X' git commit -am x"),
        ("a dollar elsewhere in the line is no environment", "GIT_WORK_TREE=../other git commit -m \"$msg\""),
        ("another variable is not read", "MY_GIT_DIR=$X git commit -am x"),
        ("an unset is not refused", "unset GIT_DIR; git commit -am x"),
        ("a dollar in single quotes inside the word is literal", "GIT_DIR=/a/'b$c'/x git commit -am x"),
        ("an escaped dollar is literal", "GIT_DIR=/a/\\$b git commit -am x"),
    ] {
        out.push((name, commit_attempt(hook, &current, command).0 == 0));
    }
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-8
/// An option whose value cannot be read from the text names a repository the hook cannot see: refused whatever the
/// reviews; a literal value, or a `$` elsewhere in the line, is not.
fn unreadable_option_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, other) = target_fixture(hook, "rev8");
    review_done(hook, &other, "code-review");
    let mut out = all_refused(
        hook,
        &current,
        &[
            "git --git-dir=$X commit -am x",
            "git --git-dir $X commit -am x",
            "git --git-dir=\"$HOME/o/.git\" commit -am x",
            "git --work-tree=$X commit -am x",
            "git --work-tree ${X} commit -am x",
            "git -C \"$(mktemp -d)\" commit -am x",
            "git -C $X commit -am x",
            "git -C `pwd`/../other commit -am x",
            "git -C ../other --git-dir=$X commit -am x",
            "git -C \"$X\" -C ../other commit -am x",
        ],
    );
    for (name, command) in [
        ("a single-quoted dollar in --git-dir is a literal value", "git --git-dir='$X' commit -am x"),
        ("a single-quoted dollar in -C is a literal value", "git -C '$X' commit -am x"),
        ("a path without dollar passes", "git --git-dir=../other/.git --work-tree ../other commit -am x"),
        ("a dollar in the message is no repository", "git -C ../other commit -m \"$msg\""),
        ("a dollar in a later option is no repository", "git --work-tree ../other commit -am x --author=\"$USER\""),
    ] {
        out.push((name, commit_attempt(hook, &current, command).0 == 0));
    }
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

// spec: REV-8
/// fish sets the environment with `set -x NAME value`, without a `NAME=value` word: read as the assignment, or refused when
/// the value cannot be read; `set -e` takes it back.
fn fish_set_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let (current, other) = target_fixture(hook, "rev8b");
    let mut out = all_refused(
        hook,
        &current,
        &[
            "set -x GIT_DIR $X; git commit -am x",
            "set --export GIT_WORK_TREE (mktemp -d); git commit -am x",
            "set -gx GIT_DIR \"$HOME/o/.git\"; git commit -am x",
            "set -Ux GIT_WORK_TREE $X\ngit commit -am x",
            "set -x GIT_DIR ../other/.git; git commit -am x",
            "set --export --global GIT_WORK_TREE '../other'; git commit -am x",
            "set -x GIT_WORK_TREE ../a; set -x GIT_WORK_TREE ../other; git commit -am x",
        ],
    );
    // `other` is still unreviewed: only the erase lets these through
    for (name, command) in [
        ("set -e takes the export back", "set -x GIT_WORK_TREE ../other; set -e GIT_WORK_TREE; git commit -am x"),
        ("set --erase takes the export back", "set -gx GIT_DIR ../other/.git; set --erase GIT_DIR; git commit -am x"),
    ] {
        out.push((name, commit_attempt(hook, &current, command).0 == 0));
    }
    review_done(hook, &other, "code-review");
    for (name, command) in [
        ("a fish export naming the reviewed repository passes", "set -x GIT_WORK_TREE ../other; git commit -am x"),
        ("a local fish variable is no environment of git", "set GIT_DIR $X; git commit -am x"),
        ("a single-quoted dollar is a literal value", "set -x GIT_WORK_TREE '$X'; git commit -am x"),
        ("another variable is not read", "set -x MY_GIT_DIR $X; git commit -am x"),
        ("a dollar elsewhere in the line is no environment", "set -x FOO $X; git commit -m \"$msg\""),
        ("a dollar after a literal value is no part of it", "set -x GIT_WORK_TREE ../other; git commit -m \"$msg\""),
        ("a bracket after a literal value is no part of it", "set -x GIT_WORK_TREE ../other; git commit -m \"(wip)\""),
        ("a shell set before the environment does not swallow it", "set -e && GIT_WORK_TREE=../other git commit -am x"),
    ] {
        out.push((name, commit_attempt(hook, &current, command).0 == 0));
    }
    let _ = fs::remove_dir_all(current.parent().unwrap());
    out
}

/// Runs `mode` for `event` with a `git` that only logs its arguments, and returns the logged calls (`None` when the
/// hook itself failed, so that "no call" never means "did not run").
fn git_calls(hook: &Path, mode: &str, event: &serde_json::Value) -> Option<Vec<String>> {
    let root = std::env::temp_dir().join(format!("reviewtest-calls-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let log = root.join("calls.log");
    let fake = bin.join("git");
    fs::write(&fake, format!("#!/bin/sh\necho \"$*\" >> '{}'\n", log.display())).unwrap();
    fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    let mut child = Command::new("rust-script")
        .arg(hook)
        .arg(mode)
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("rust-script must be installed");
    child.stdin.take().unwrap().write_all(event.to_string().as_bytes()).unwrap();
    let ran = child.wait().unwrap().success();
    let calls = fs::read_to_string(&log).unwrap_or_default().lines().map(String::from).collect();
    let _ = fs::remove_dir_all(&root);
    ran.then_some(calls)
}

fn bash_calls(hook: &Path, command: &str) -> Option<Vec<String>> {
    git_calls(hook, "check", &serde_json::json!({"tool_name": "Bash", "tool_input": {"command": command}}))
}

/// A command that is not a commit costs no `git` call: the hook runs on every command of the agent.
fn cheap_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let skill = |name: &str| serde_json::json!({"tool_name": "Skill", "tool_input": {"skill": name}, "tool_response": {"success": true}});
    vec![
        ("a command that is not a commit runs no git", bash_calls(hook, "ls -la") == Some(vec![])),
        ("a git command that is not a commit runs no git", bash_calls(hook, "git status") == Some(vec![])),
        ("a commit looks for the top of its work tree", bash_calls(hook, "git commit -m x").is_some_and(|c| c.iter().any(|c| c.contains("--show-toplevel")))),
        ("a skill other than the review runs no git", git_calls(hook, "record", &skill("update-docs")) == Some(vec![])),
        ("the review skill records at the top of its work tree", git_calls(hook, "record", &skill("code-review")).is_some_and(|c| c.iter().any(|c| c.contains("--show-toplevel")))),
    ]
}

/// Untracked files are hashed by `git` itself (their paths on its stdin), never read by the hook and piped to it.
fn streaming_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let root = std::env::temp_dir().join(format!("reviewtest-stream-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(root.join("big.bin"), vec![b'x'; 1 << 20]).unwrap();
    let log = root.join("calls.log");
    // `ls-files` lists the big file; every `hash-object` logs the size of what it is fed
    let fake = bin.join("git");
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in\n  rev-parse) [ \"$2\" = --git-path ] && echo \"$3\" ;;\n  ls-files) printf 'big.bin\\0' ;;\n  hash-object) echo \"$* $(wc -c)\" >> '{}'; echo 0000000000000000000000000000000000000000 ;;\nesac\n",
        log.display()
    );
    fs::write(&fake, script).unwrap();
    fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    let event = serde_json::json!({"tool_name": "Skill", "cwd": root, "tool_input": {"skill": "code-review"}, "tool_response": {"success": true}});
    let mut child = Command::new("rust-script")
        .arg(hook)
        .arg("record")
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("rust-script must be installed");
    child.stdin.take().unwrap().write_all(event.to_string().as_bytes()).unwrap();
    let ran = child.wait().unwrap().success();
    let fed: Vec<usize> = fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.rsplit(' ').next()?.parse().ok())
        .collect();
    let _ = fs::remove_dir_all(&root);
    vec![
        ("the review ran with the fake git", ran && !fed.is_empty()),
        ("no content of an untracked file is piped to git", fed.iter().all(|n| *n < 1 << 16)),
    ]
}

/// The marker still follows the content of an untracked file, and only that.
fn untracked_content_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let root = std::env::temp_dir().join(format!("reviewtest-untracked-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("sub")).unwrap();
    sh(&root, &["init", "-q", "."]);
    fs::write(root.join("a.txt"), "one\n").unwrap();
    sh(&root, &["add", "-A"]);
    sh(&root, &["commit", "-qm", "init"]);
    fs::write(root.join("new.txt"), "aaaa\n").unwrap();
    fs::write(root.join("odd\nname.txt"), "aaaa\n").unwrap();
    fs::create_dir_all(root.join("nested")).unwrap();
    sh(&root.join("nested"), &["init", "-q", "."]);
    let locked = root.join("locked.txt");
    fs::write(&locked, "secret\n").unwrap();
    fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
    let marker = |hook: &Path| {
        review_done(hook, &root, "code-review");
        let text = fs::read_to_string(root.join(".git/claude-code-review.json")).unwrap();
        serde_json::from_str::<serde_json::Value>(&text).unwrap()["hash"].as_str().unwrap().to_string()
    };
    let first = marker(hook);
    let mut out = vec![("the marker is stable while nothing changes", marker(hook) == first)];
    fs::write(root.join("new.txt"), "bbbb\n").unwrap();
    let second = marker(hook);
    out.push(("a content change of the same size changes the marker", second != first));
    fs::write(root.join("odd\nname.txt"), "bbbb\n").unwrap();
    out.push(("a file with a newline in its name is covered", marker(hook) != second));
    // a nested repository and an unreadable file do not stop the other files from being hashed
    fs::write(root.join("new.txt"), "cccc\n").unwrap();
    out.push(("a nested repository and an unreadable file leave the other files covered", marker(hook) != second));
    fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
    let _ = fs::remove_dir_all(&root);
    out
}

// spec: REV-13
/// A command that writes the waiver is refused, with the path the user creates; the refusal does not depend on the review.
fn waiver_write_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let project = std::env::temp_dir().join(format!("reviewtest-ww-{}", std::process::id()));
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).unwrap();
    sh(&project, &["init", "-q", "."]);
    let mut out = Vec::new();
    for (name, command) in [
        ("refuses a redirection into the waiver", "echo > .git/claude-review-waived"),
        ("refuses a redirection with a quoted target", "echo x >> \".git/claude-review-waived\""),
        ("refuses tee on the waiver", "echo x | tee .git/claude-review-waived"),
        ("refuses a copy onto the waiver", "sudo cp a .git/claude-review-waived"),
        ("refuses a shell that names the waiver", "sh -c 'touch .git/claude-review-waived'"),
        ("refuses a heredoc fed to a shell", "bash <<'EOF'\ntouch .git/claude-review-waived\nEOF"),
        ("refuses a substitution inside double quotes", "echo \"$(touch .git/claude-review-waived)\""),
        ("refuses a backtick inside double quotes", "echo \"`touch .git/claude-review-waived`\""),
        ("refuses a quote closed by an escape", "echo \"\\\"\"; touch .git/claude-review-waived"),
        ("refuses a command in braces", "{ touch .git/claude-review-waived; }"),
        ("refuses a command after a keyword", "if true; then touch .git/claude-review-waived; fi"),
        ("refuses a wrapper with options", "sudo -u me touch .git/claude-review-waived"),
        ("refuses find -exec", "find . -exec touch .git/claude-review-waived {} +"),
        ("refuses a continued line", "touch \\\n .git/claude-review-waived"),
        ("refuses after a quoted heredoc marker", "echo \"use <<EOF\"\ntouch .git/claude-review-waived"),
        ("refuses after a comment with an apostrophe", "# it's the waiver\ntouch .git/claude-review-waived"),
        ("refuses an unbalanced quote", "echo 'x; touch .git/claude-review-waived"),
        ("refuses an edit in place", "sed -i s/a/b/ .git/claude-review-waived"),
        ("refuses a copy onto the waiver after two heredocs", "cat <<A <<B\na\nA\nb\nB\ncp x .git/claude-review-waived"),
        ("refuses the waiver created after a commit message", "git commit -qm 'a b'; touch .git/claude-review-waived"),
    ] {
        out.push((name, commit_attempt(hook, &project, command).0 == 2));
    }
    let (_, message) = commit_attempt(hook, &project, "touch .git/claude-review-waived");
    out.push(("the refusal gives the path of the waiver", message.contains(".git/claude-review-waived")));
    let _ = fs::remove_dir_all(&project);
    out
}

// spec: REV-14
/// A command that only names the waiver is not refused for it (a clean repository has nothing to review).
fn waiver_mention_checks(hook: &Path) -> Vec<(&'static str, bool)> {
    let project = std::env::temp_dir().join(format!("reviewtest-wm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).unwrap();
    sh(&project, &["init", "-q", "."]);
    fs::write(project.join("c.txt"), "x\n").unwrap();
    sh(&project, &["add", "-A"]);
    sh(&project, &["commit", "-qm", "init"]);
    let mut out = Vec::new();
    for (name, command) in [
        ("lets through a read with sed", "sed -n 1p .git/claude-review-waived"),
        ("lets through a copy of the waiver", "cp .git/claude-review-waived /tmp/x"),
        ("lets through a message with backticks in a quoted heredoc", "git commit -q -F - <<'EOF'\ndocs: `touch .git/claude-review-waived`\nEOF"),
        ("lets through a grep for the waiver", "grep -rn claude-review-waived specs files"),
        ("lets through a commit message that explains the waiver", "git commit -qm 'run touch .git/claude-review-waived; then tee it'"),
        ("lets through a commit message that quotes a redirection into the waiver", "git commit -qm \"echo > .git/claude-review-waived\""),
        ("lets through a commit message in a heredoc", "git commit -q -F - <<'EOF'\ndocs: touch .git/claude-review-waived\nEOF"),
    ] {
        out.push((name, commit_attempt(hook, &project, command).0 == 0));
    }
    let _ = fs::remove_dir_all(&project);
    out
}

fn main() {
    let here = fs::canonicalize(Path::new(file!()).parent().unwrap_or(Path::new("."))).unwrap();
    let hook = here.join("require_code_review.rs");
    let project = std::env::temp_dir().join(format!("reviewtest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).unwrap();
    sh(&project, &["init", "-q", "."]);
    fs::write(project.join("a.txt"), "one\n").unwrap();
    sh(&project, &["add", "-A"]);
    sh(&project, &["commit", "-qm", "init"]);

    let mut failed = false;
    let mut check = |name: &str, ok: bool| {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    };
    let commit = "git add -A && git commit -m \"feat: x\"";

    check("lets through a commit with nothing changed", commit_attempt(&hook, &project, commit).0 == 0);

    fs::write(project.join("a.txt"), "two\n").unwrap();
    let (code, stderr) = commit_attempt(&hook, &project, commit);
    check("refuses a commit without any review", code == 2 && stderr.contains("code-review"));
    check("lets through a command that is not a commit", commit_attempt(&hook, &project, "git status").0 == 0);
    check("lets through git log mentioning commit", commit_attempt(&hook, &project, "git log --grep commit").0 == 0);

    review_done(&hook, &project, "simplify");
    check("ignores the end of another skill", commit_attempt(&hook, &project, commit).0 == 2);

    review_done(&hook, &project, "code-review");
    check("lets through a commit of the reviewed diff", commit_attempt(&hook, &project, commit).0 == 0);

    fs::write(project.join("a.txt"), "two, fixed\n").unwrap();
    check("accepts a fix made after the review in a reviewed file", commit_attempt(&hook, &project, commit).0 == 0);
    fs::write(project.join("a.txt"), "two\n").unwrap();

    fs::write(project.join("new.txt"), "x\n").unwrap();
    check("refuses a file the review never saw", commit_attempt(&hook, &project, commit).0 == 2);

    fs::remove_file(project.join("new.txt")).unwrap();
    check("accepts the reviewed diff again", commit_attempt(&hook, &project, commit).0 == 0);

    sh(&project, &["commit", "-aqm", "feat: reviewed"]);
    fs::write(project.join("a.txt"), "three\n").unwrap();
    check("refuses a review made before the last commit", commit_attempt(&hook, &project, commit).0 == 2);

    fs::create_dir_all(project.join("deep")).unwrap();
    let other = project.join("sub");
    fs::create_dir_all(&other).unwrap();
    sh(&other, &["init", "-q", "."]);
    fs::write(other.join("b.txt"), "x\n").unwrap();
    let relative = serde_json::json!({"tool_name": "Bash", "cwd": project.join("deep"), "tool_input": {"command": "git -C ../sub commit -am x"}});
    check("resolves -C from the shell directory", run(&hook, "check", &project, &relative).0 == 2);
    let from_deep = serde_json::json!({"tool_name": "Bash", "cwd": project.join("deep"), "tool_input": {"command": "git commit -am x"}});
    check("sees the whole work tree from a subdirectory", run(&hook, "check", &project, &from_deep).0 == 2);
    check("sees a commit ended by a semicolon", commit_attempt(&hook, &project, "git add .;git commit;").0 == 2);
    check("looks at the repository named by -C", commit_attempt(&hook, &project, "git -C sub commit -am x").0 == 2);
    check("sees a commit in brackets", commit_attempt(&hook, &project, "(git commit -m x)").0 == 2);
    for option in ["--git-dir", "--work-tree", "--namespace", "--exec-path", "--super-prefix"] {
        let value = match option { "--git-dir" => "sub/.git", "--work-tree" => "sub", _ => "/x" };
        let spaced = format!("git {option} {value} commit -m x");
        check(&format!("sees a commit after {option} given as two words"), commit_attempt(&hook, &project, &spaced).0 == 2);
    }
    check("sees a commit after a quoted option value", commit_attempt(&hook, &project, "git --namespace 'a b' commit -m x").0 == 2);
    check("sees a commit after --namespace=value", commit_attempt(&hook, &project, "git --namespace=n commit -m x").0 == 2);
    check("sees a commit after -c with a quoted value", commit_attempt(&hook, &project, "git -c 'user.name=A B' commit -m x").0 == 2);
    check("looks at the repository named by a quoted -C", commit_attempt(&hook, &project, "git -C 'sub' commit -am x").0 == 2);
    check("ignores a path named commit after an option value", commit_attempt(&hook, &project, "git --git-dir /x log -- commit").0 == 0);
    check("ignores a path named commit", commit_attempt(&hook, &project, "git log -- commit").0 == 0);

    review_done(&hook, &project, "code-review");
    check("lets through the reviewed diff of the next change", commit_attempt(&hook, &project, commit).0 == 0);
    fs::write(project.join("é.txt"), "x\n").unwrap();
    check("refuses a new non-ASCII file", commit_attempt(&hook, &project, commit).0 == 2);
    fs::remove_file(project.join("é.txt")).unwrap();

    let failed_review = serde_json::json!({
        "tool_name": "Skill", "cwd": project, "tool_input": {"skill": "code-review"}, "tool_response": {"success": false}
    });
    fs::write(project.join("f.txt"), "z\n").unwrap();
    run(&hook, "record", &project, &failed_review);
    check("does not record a failed review", commit_attempt(&hook, &project, commit).0 == 2);

    let esc = project.join("esc");
    fs::create_dir_all(esc.join("issues")).unwrap();
    sh(&esc, &["init", "-q", "."]);
    fs::write(esc.join("issues/i.md"), "one\n").unwrap();
    fs::write(esc.join("code.txt"), "one\n").unwrap();
    sh(&esc, &["add", "-A"]);
    sh(&esc, &["commit", "-qm", "init"]);
    let esc_commit = "git -C esc commit -am x";
    fs::write(esc.join("issues/i.md"), "two\n").unwrap();
    check("lets through a commit that only touches issues", commit_attempt(&hook, &project, esc_commit).0 == 0);
    fs::write(esc.join("issues/j.md"), "new\n").unwrap();
    check("lets through a new issue file", commit_attempt(&hook, &project, esc_commit).0 == 0);
    sh(&esc, &["mv", "code.txt", "issues/code.txt"]);
    check("refuses a code file renamed into issues", commit_attempt(&hook, &project, esc_commit).0 == 2);
    sh(&esc, &["mv", "issues/code.txt", "code.txt"]);
    fs::write(esc.join("code.txt"), "two\n").unwrap();
    check("refuses issues mixed with code", commit_attempt(&hook, &project, esc_commit).0 == 2);
    fs::write(esc.join("docs.md"), "doc\n").unwrap();
    fs::write(esc.join("code.txt"), "one\n").unwrap();
    check("refuses documentation", commit_attempt(&hook, &project, esc_commit).0 == 2);
    let waiver = esc.join(".git/claude-review-waived");
    check("refuses a command that creates the waiver", commit_attempt(&hook, &project, "touch esc/.git/claude-review-waived").0 == 2);
    fs::write(&waiver, "").unwrap();
    check("lets through a commit the user waived", commit_attempt(&hook, &project, esc_commit).0 == 0);
    check("keeps the waiver while no commit happened", waiver.exists() && commit_attempt(&hook, &project, esc_commit).0 == 0);
    sh(&esc, &["add", "-A"]);
    sh(&esc, &["commit", "-qm", "waived"]);
    fs::write(esc.join("docs.md"), "doc two\n").unwrap();
    check("consumes the waiver once a commit happened", commit_attempt(&hook, &project, esc_commit).0 == 2 && !waiver.exists());

    // a waiver spent by a commit is gone at the next check, so the user's new one is not mistaken for it
    fs::write(&waiver, "").unwrap();
    check("lets through the commit of a waiver created again", commit_attempt(&hook, &project, esc_commit).0 == 0);
    sh(&esc, &["add", "-A"]);
    sh(&esc, &["commit", "-qm", "waived again"]);
    fs::write(esc.join("issues/k.md"), "new\n").unwrap();
    check("an issues-only check drops the waiver spent by a commit", commit_attempt(&hook, &project, esc_commit).0 == 0 && !waiver.exists());

    let fresh = project.join("fresh");
    fs::create_dir_all(&fresh).unwrap();
    sh(&fresh, &["init", "-q", "."]);
    fs::write(fresh.join("c.txt"), "x\n").unwrap();
    check("refuses the first commit of a repository", commit_attempt(&hook, &project, "git -C fresh commit -m x").0 == 2);
    let fresh_waiver = fresh.join(".git/claude-review-waived");
    fs::write(&fresh_waiver, "").unwrap();
    check("lets through the first commit the user waived", commit_attempt(&hook, &project, "git -C fresh commit -m x").0 == 0);
    check("stamps the waiver of a repository without commit", fs::read_to_string(&fresh_waiver).unwrap().trim() == "no-head");
    fs::remove_file(&fresh_waiver).unwrap();
    review_done(&hook, &fresh, "code-review");
    check("lets through the reviewed first commit", commit_attempt(&hook, &project, "git -C fresh commit -m x").0 == 0);

    for checks in [waiver_write_checks, waiver_mention_checks, worktree_checks, target_checks, accumulated_c_checks, cd_checks, work_tree_and_git_dir_checks, option_forms_checks, relative_option_checks, git_dir_alone_checks, environment_checks, undeterminable_checks, unreadable_option_checks, fish_set_checks, native_checks, native_hook_checks, native_worktree_checks, skipped_hook_checks, cheap_checks, streaming_checks, untracked_content_checks] {
        for (name, ok) in checks(&hook) {
            check(name, ok);
        }
    }

    let _ = fs::remove_dir_all(&project);
    std::process::exit(i32::from(failed));
}
