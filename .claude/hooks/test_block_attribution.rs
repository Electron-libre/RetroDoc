#!/usr/bin/env rust-script
//! Behavior test for `block_ai_attribution.rs`: feeds it `PreToolUse` events for `Bash` commands.
//! Usage: `just test-harness` (or `rust-script .claude/hooks/test_block_attribution.rs`).

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Exit code and stderr of the hook for one Bash command run from `cwd`.
fn run(hook: &Path, cwd: &Path, command: &str) -> (i32, String) {
    let event = format!(
        "{{\"tool_name\":\"Bash\",\"cwd\":{},\"tool_input\":{{\"command\":{}}}}}",
        json_string(&cwd.display().to_string()),
        json_string(command)
    );
    run_raw(hook, &event)
}

fn run_raw(hook: &Path, stdin: &str) -> (i32, String) {
    let mut child = Command::new("rust-script")
        .arg(hook)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rust-script must be installed");
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() {
    let here = fs::canonicalize(Path::new(file!()).parent().unwrap_or(Path::new("."))).unwrap();
    let hook = here.join("block_ai_attribution.rs");
    let dir = std::env::temp_dir().join(format!("attrtest-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("msg.txt"), "feat: x\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n").unwrap();
    fs::write(dir.join("clean.txt"), "feat: x\n\nA plain body.\n").unwrap();

    let mut failed = false;
    let mut check = |name: &str, ok: bool| {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    };

    let blocked = [
        ("trailer in -m", "git commit -m \"feat: x\" -m \"Co-Authored-By: Claude Sonnet <a@b.c>\""),
        ("trailer in a heredoc", "git commit -m \"$(cat <<'EOF'\nfeat: x\n\nCo-Authored-By: Claude <a@b.c>\nEOF\n)\""),
        ("trailer after an escaped newline", "git commit -m $'feat: x\\n\\nCo-Authored-By: Anthropic'"),
        ("trailer with a space before the colon", "git commit -m \"x\" -m \"Co-Authored-By : Claude <a@b.c>\""),
        ("trailer with spaces around the colon", "git commit -m \"x\" -m \"co-authored-by  :Anthropic\""),
        ("trailer with a spaced key", "git commit -m \"x\" -m \"Co - Authored - By: Claude\""),
        ("generated-with footer", "git commit -m \"feat: x\" -m \"Generated with Claude Code\""),
        ("emoji generated-with footer", "gh pr create --title t --body \"body\n\n🤖 Generated with [Claude Code](https://claude.com/claude-code)\""),
        ("anthropic noreply address", "git commit -m \"feat: x\" -m \"by <noreply@anthropic.com>\""),
        ("git with a global option", "git -C /tmp commit -m \"Co-Authored-By: Claude\""),
        ("git with many global options", "git -c user.name=a -c user.email=b -c core.x=y commit -m \"x\" -m \"Co-Authored-By: Claude\""),
        ("gh pr create after a repo flag", "gh -R owner/repo pr create --body \"Generated with Claude Code\""),
        ("gh pr edit after a long repo flag", "gh --repo x pr edit 3 --body \"Generated with Claude Code\""),
        ("gh pr edit after a repo flag with =", "gh --repo=x pr edit 3 --body \"Generated with Claude Code\""),
        ("git commit after a bracket", "(git commit -m \"x\" -m \"Co-Authored-By: Claude\")"),
        ("git with a git-dir option", "git --git-dir /x/.git commit -m \"x\" -m \"Co-Authored-By: Claude\""),
        ("git with a quoted option value", "git -C \"my repo\" commit -m \"x\" -m \"Co-Authored-By: Claude\""),
        ("git with a quoted config value", "git -c 'user.name=A B' commit -m \"x\" -m \"Co-Authored-By: Claude\""),
        ("git with a quoted subcommand", "git \"commit\" -m \"x\" -m \"Co-Authored-By: Claude\""),
        ("gh pr create after a hostname flag", "gh --hostname h.example pr create --body \"Generated with Claude Code\""),
        ("git commit on a later line", "git status\ngit -p commit -m \"x\" -m \"Co-Authored-By: Claude\""),
        ("a credit chained after another command", "echo hi && git commit -m \"x\" -m \"Co-Authored-By: Claude\""),
        ("a second commit chained after a clean one", "git commit -m ok; git commit -m \"Generated with Claude Code\""),
        ("a credit in a substitution of the message", "git commit -m \"$(echo 'Co-Authored-By: Claude')\""),
        ("a credit in an unquoted heredoc", "git commit -F - <<'EOF'\nfeat: x\n\nCo-Authored-By: Claude <a@b.c>\nEOF"),
        ("a credit in a heredoc of gh", "gh pr create --body-file - <<EOF\nbody\n\nGenerated with Claude Code\nEOF"),
        ("a credit in a substitution with a separator", "git commit -m $(echo ok; echo 'Co-Authored-By: Claude')"),
        ("a credit in a backtick substitution", "git commit -m `echo ok; echo 'Co-Authored-By: Claude'`"),
        ("a message file after a redirection", "git commit 2>&1 -F msg.txt"),
        ("a credit in a heredoc piped to the commit", "cat <<'EOF' | git commit -F -\nfeat: x\n\nCo-Authored-By: Claude <a@b.c>\nEOF"),
        ("a credit in a process substitution", "gh pr create --body-file <(echo ok; echo 'Generated with Claude Code')"),
        ("gh pr edit", "gh pr edit 3 --body \"Generated with Claude Code\""),
        ("message file of git commit", &format!("git commit -F {}/msg.txt", dir.display())),
        ("body file of gh pr create", &format!("gh pr create --body-file {}/msg.txt", dir.display())),
        ("relative message file", "git commit -F msg.txt"),
    ];
    for (name, command) in blocked {
        let (code, stderr) = run(&hook, &dir, command);
        check(&format!("blocks {name}"), code == 2 && stderr.contains("attribution"));
    }

    let allowed = [
        ("a message that discusses the rule", "git commit -m \"docs: forbid the Co-Authored-By trailer naming Claude\""),
        ("a heredoc message that discusses the rule", "git commit -m \"$(cat <<'EOF'\ndocs: say why we reject Generated with footers\n\nThe Co-Authored-By trailer is refused by the hook.\nEOF\n)\""),
        ("the noreply address in a discussion", "git commit -m \"docs: refuse the <noreply@anthropic.com> address\""),
        ("a trailer of a human", "git commit -m \"x\" -m \"Co-Authored-By: Ada <ada@example.com>\""),
        ("a message naming the trailer without a colon", "git commit -m \"docs: co-authored-by trailers naming Claude are banned\""),
        ("a trailer key glued to a name", "git commit -m \"x\" -m \"Co-Authored-By-Claude without a colon\""),
        ("a commit in a later command", "git log && echo commit -m \"Co-Authored-By: Claude\""),
        ("an echo of a credit chained with &&", "git commit -m ok && echo \"Co-Authored-By: Claude\""),
        ("an echo of a credit chained with ;", "git commit -m ok; echo 'Generated with Claude Code'"),
        ("an echo of a credit piped from the commit", "git commit -m ok | echo \"by <noreply@anthropic.com>\""),
        ("an echo of a credit on the next line", "git commit -m ok\necho \"Co-Authored-By: Claude\""),
        ("a clean unquoted heredoc", "git commit -F - <<'EOF'\nfeat: x\n\nA plain body.\nEOF"),
        ("a quoted << before a chained echo", "git commit -m \"docs: explain a<<b shift\" && echo \"Co-Authored-By: Claude\""),
        ("a clean commit", "git commit -m \"feat: x\""),
        ("a clean message file", "git commit -F clean.txt"),
        ("a missing message file", "git commit -F nowhere.txt"),
        ("a clean pull request", "gh pr create --title t --body \"plain\""),
        ("an unrelated command with the pattern", "echo \"Co-Authored-By: Claude\""),
        ("git log grepping the word commit", "git log --grep commit --format=\"Co-Authored-By: Claude\""),
        ("git show of a ref named commit", "git show commit --format=\"Co-Authored-By: Claude\""),
        ("gh pr view", "gh pr view 3 --template \"Co-Authored-By: Claude\""),
        ("gh issue create", "gh -R o/r issue create --body \"pr create Co-Authored-By: Claude\""),
        ("git log", "git log --grep=\"Co-Authored-By: Claude\""),
    ];
    for (name, command) in allowed {
        check(&format!("lets through {name}"), run(&hook, &dir, command).0 == 0);
    }
    check("lets through invalid JSON", run_raw(&hook, "not json").0 == 0);
    check("lets through an event without command", run_raw(&hook, "{}").0 == 0);

    let _ = fs::remove_dir_all(&dir);
    std::process::exit(i32::from(failed));
}
