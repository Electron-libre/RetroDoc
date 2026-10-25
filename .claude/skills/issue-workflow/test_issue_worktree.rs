#!/usr/bin/env rust-script
//! Behavior test for `issue_worktree.rs` (`start` and `finish`) against a throwaway git repository.
//! Usage: `just test-harness`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success()
}

fn git_ok(dir: &Path, args: &[&str]) {
    assert!(git(dir, args), "git {args:?} failed");
}

fn branch_exists(repo: &Path, name: &str) -> bool {
    git(repo, &["rev-parse", "--verify", "-q", &format!("refs/heads/{name}")])
}

struct Ctx {
    root: PathBuf,
    repo: PathBuf,
    script: PathBuf,
}

type Checks = Vec<(&'static str, bool)>;

impl Ctx {
    fn run(&self, cwd: &Path, args: &[&str]) -> (bool, String) {
        if !cwd.is_dir() {
            return (false, format!("{} does not exist", cwd.display()));
        }
        let out = Command::new("rust-script").arg(&self.script).args(args).current_dir(cwd).output().expect("rust-script must be installed");
        (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
    }

    fn wt(&self, stem: &str) -> PathBuf {
        self.root.join(format!("repo-{stem}"))
    }
}

// spec: PAR-1
fn start_creates_the_worktree(c: &Ctx) -> Checks {
    let (ok, out) = c.run(&c.repo, &["start", "issues/x.md"]);
    let created = ok && c.wt("x").join("issues/x.md").is_file() && branch_exists(&c.repo, "x");
    let (linked_ok, _) = c.run(&c.wt("x"), &["start", "issues/y.md"]);
    vec![
        ("start creates the sibling worktree on a branch named after the stem", created),
        ("start prints the worktree path", out.contains(c.wt("x").to_str().unwrap())),
        ("start works from a linked worktree and names the sibling of the main one", linked_ok && c.wt("y").is_dir()),
    ]
}

// spec: PAR-2
fn start_refuses_and_changes_nothing(c: &Ctx) -> Checks {
    let before = std::fs::read_dir(&c.root).unwrap().count();
    let (taken, _) = c.run(&c.repo, &["start", "issues/x.md"]);
    let (missing, _) = c.run(&c.repo, &["start", "issues/missing.md"]);
    let (uncommitted, _) = c.run(&c.repo, &["start", "issues/new.md"]);
    std::fs::create_dir_all(c.wt("z")).unwrap();
    let (occupied, _) = c.run(&c.repo, &["start", "issues/z.md"]);
    std::fs::remove_dir_all(c.wt("z")).unwrap();
    git_ok(&c.repo, &["checkout", "-q", "--detach"]);
    let (detached, _) = c.run(&c.repo, &["start", "issues/z.md"]);
    let detached_clean = !branch_exists(&c.repo, "z") && !c.wt("z").exists();
    git_ok(&c.repo, &["checkout", "-q", "trunk"]);
    vec![
        ("start refuses a detached main worktree", !detached && detached_clean),
        ("start refuses an issue whose branch exists", !taken),
        ("start refuses a missing issue file", !missing && !branch_exists(&c.repo, "missing")),
        ("start refuses an issue file the main branch does not hold", !uncommitted && !branch_exists(&c.repo, "new")),
        ("start refuses an existing directory", !occupied && !branch_exists(&c.repo, "z")),
        ("a refusal changes nothing", std::fs::read_dir(&c.root).unwrap().count() == before),
    ]
}

// spec: PAR-6
fn finish_needs_a_merged_branch(c: &Ctx) -> Checks {
    std::fs::write(c.wt("x").join("work.txt"), "work\n").unwrap();
    git_ok(&c.wt("x"), &["add", "-A"]);
    git_ok(&c.wt("x"), &["commit", "-qm", "work", "--no-verify"]);
    let (unmerged, _) = c.run(&c.repo, &["finish", "x"]);
    let kept = c.wt("x").is_dir() && branch_exists(&c.repo, "x");
    git_ok(&c.repo, &["merge", "-q", "--ff-only", "x"]);
    let (merged, _) = c.run(&c.repo, &["finish", "x"]);
    vec![
        ("finish refuses an unmerged branch and keeps both", !unmerged && kept),
        ("finish removes the worktree and the branch once merged", merged && !c.wt("x").exists() && !branch_exists(&c.repo, "x")),
    ]
}

// spec: PAR-7
fn finish_removes_only_a_clean_sibling(c: &Ctx) -> Checks {
    std::fs::write(c.wt("y").join("dirty.txt"), "wip\n").unwrap();
    let (dirty, _) = c.run(&c.repo, &["finish", "y"]);
    let elsewhere = c.root.join("elsewhere");
    git_ok(&c.repo, &["worktree", "add", "-q", "-b", "w", elsewhere.to_str().unwrap()]);
    let (stranger, _) = c.run(&c.repo, &["finish", "w"]);
    vec![
        ("finish refuses a worktree with uncommitted changes", !dirty && c.wt("y").is_dir() && branch_exists(&c.repo, "y")),
        ("finish does not remove a worktree that is not the sibling of the stem", !stranger && elsewhere.is_dir() && branch_exists(&c.repo, "w")),
    ]
}

fn commit_file(wt: &Path, name: &str) {
    std::fs::write(wt.join(name), format!("{name}\n")).unwrap();
    git_ok(wt, &["add", "-A"]);
    git_ok(wt, &["commit", "-qm", name, "--no-verify"]);
}

// spec: PAR-6
fn finish_accepts_a_patch_equivalent_branch(c: &Ctx) -> Checks {
    for stem in ["r", "m"] {
        let (ok, _) = c.run(&c.repo, &["start", &format!("issues/{stem}.md")]);
        assert!(ok);
        commit_file(&c.wt(stem), &format!("{stem}.txt"));
    }
    commit_file(&c.repo, "trunk-moved.txt");
    git_ok(&c.repo, &["cherry-pick", "r"]);
    git_ok(&c.repo, &["merge", "-q", "--squash", "m"]);
    git_ok(&c.repo, &["commit", "-qm", "squash m", "--no-verify"]);
    let (rebased, _) = c.run(&c.repo, &["finish", "r"]);
    let (squashed, _) = c.run(&c.repo, &["finish", "m"]);
    vec![
        ("finish accepts a rebased branch", rebased && !c.wt("r").exists() && !branch_exists(&c.repo, "r")),
        ("finish accepts a branch squashed from one commit", squashed && !c.wt("m").exists() && !branch_exists(&c.repo, "m")),
    ]
}

// spec: PAR-6
fn finish_accepts_a_squash_of_several_commits(c: &Ctx) -> Checks {
    for stem in ["s", "t"] {
        let (ok, _) = c.run(&c.repo, &["start", &format!("issues/{stem}.md")]);
        assert!(ok);
        commit_file(&c.wt(stem), &format!("{stem}1.txt"));
        commit_file(&c.wt(stem), &format!("{stem}2.txt"));
        git_ok(&c.repo, &["merge", "-q", "--squash", stem]);
        git_ok(&c.repo, &["commit", "-qm", &format!("squash {stem}"), "--no-verify"]);
    }
    let (squashed, _) = c.run(&c.repo, &["finish", "s"]);
    // trunk rewrites a squashed file: the content of `t` is no longer all on trunk, so it counts as unmerged.
    std::fs::write(c.repo.join("t1.txt"), "rewritten\n").unwrap();
    git_ok(&c.repo, &["commit", "-qam", "rewrite t1", "--no-verify"]);
    let (rewritten, _) = c.run(&c.repo, &["finish", "t"]);
    vec![
        ("finish accepts a branch squashed from several commits", squashed && !c.wt("s").exists() && !branch_exists(&c.repo, "s")),
        ("finish keeps a squashed branch whose content trunk changed afterwards", !rewritten && c.wt("t").is_dir() && branch_exists(&c.repo, "t")),
    ]
}

// spec: PAR-10
fn finish_forces_work_that_is_not_on_trunk(c: &Ctx) -> Checks {
    let (ok, _) = c.run(&c.repo, &["start", "issues/u.md"]);
    assert!(ok);
    commit_file(&c.wt("u"), "u1.txt");
    let (plain, msg) = c.run(&c.repo, &["finish", "u"]);
    let kept = c.wt("u").is_dir() && branch_exists(&c.repo, "u");
    std::fs::write(c.wt("u").join("dirty.txt"), "wip\n").unwrap();
    let (dirty, _) = c.run(&c.repo, &["finish", "u", "--force"]);
    let dirty_kept = c.wt("u").is_dir() && branch_exists(&c.repo, "u");
    std::fs::remove_file(c.wt("u").join("dirty.txt")).unwrap();
    let (forced, _) = c.run(&c.repo, &["finish", "u", "--force"]);
    vec![
        ("finish refuses work that is not on trunk without --force, names --force and keeps both", !plain && kept && msg.contains("--force")),
        ("finish --force still refuses a worktree with uncommitted changes", !dirty && dirty_kept),
        ("finish --force removes the worktree and the branch", forced && !c.wt("u").exists() && !branch_exists(&c.repo, "u")),
    ]
}

// spec: PAR-11
fn finish_refuses_from_inside_the_worktree(c: &Ctx) -> Checks {
    let (ok, _) = c.run(&c.repo, &["start", "issues/c.md"]);
    assert!(ok);
    let sub = c.wt("c").join("issues");
    let (inside, msg) = c.run(&c.wt("c"), &["finish", "c"]);
    let (below, _) = c.run(&sub, &["finish", "c"]);
    let (forced, _) = c.run(&sub, &["finish", "c", "--force"]);
    let kept = c.wt("c").is_dir() && branch_exists(&c.repo, "c");
    commit_file(&c.wt("c"), "c.txt");
    let (unmerged, unmerged_msg) = c.run(&c.wt("c"), &["finish", "c"]);
    let (outside_unmerged, _) = c.run(&c.repo, &["finish", "c"]);
    git_ok(&c.repo, &["merge", "-q", "--ff-only", "c"]);
    let (outside, _) = c.run(&c.repo, &["finish", "c"]);
    vec![
        ("finish refuses from the worktree to remove and names the main worktree", !inside && msg.contains(c.repo.to_str().unwrap())),
        ("finish refuses from a subdirectory of it, with or without --force", !below && !forced),
        ("a refusal keeps the worktree and the branch", kept),
        ("an unmerged branch gets the same refusal from inside", !unmerged && unmerged_msg.contains("inside") && !outside_unmerged),
        ("finish from the main worktree still removes it", outside && !c.wt("c").exists() && !branch_exists(&c.repo, "c")),
    ]
}

fn main() {
    let script = std::fs::canonicalize(Path::new(file!()).parent().unwrap()).unwrap().join("issue_worktree.rs");
    let root = std::fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("issue_worktree_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let repo = root.join("repo");
    std::fs::create_dir_all(repo.join("issues")).unwrap();
    for s in ["x", "y", "z", "w", "r", "m", "s", "t", "u", "c"] {
        std::fs::write(repo.join(format!("issues/{s}.md")), format!("# {s}\n")).unwrap();
    }
    git_ok(&repo, &["init", "-q", "-b", "trunk", "."]);
    git_ok(&repo, &["add", "-A"]);
    git_ok(&repo, &["commit", "-qm", "init", "--no-verify"]);
    std::fs::write(repo.join("issues/new.md"), "# new\n").unwrap();

    let c = Ctx { root: root.clone(), repo, script };
    let mut failed = false;
    for checks in [start_creates_the_worktree(&c), start_refuses_and_changes_nothing(&c), finish_needs_a_merged_branch(&c), finish_removes_only_a_clean_sibling(&c), finish_accepts_a_patch_equivalent_branch(&c), finish_accepts_a_squash_of_several_commits(&c), finish_forces_work_that_is_not_on_trunk(&c), finish_refuses_from_inside_the_worktree(&c)] {
        for (name, ok) in checks {
            println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
            failed |= !ok;
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    if failed {
        std::process::exit(1);
    }
}
