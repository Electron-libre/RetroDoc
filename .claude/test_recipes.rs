#!/usr/bin/env rust-script
//! Behavior test of the recipes of `harness.just` whose output the agent reads: `test-harness` prints nothing when
//! every test script passes, and only the failing lines of the script that fails.
//! Usage: `just test-harness` (or `rust-script .claude/test_recipes.rs`).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// Runs `just test-harness` in a throwaway project holding the given test scripts. A fake `rust-script` first on the
/// PATH runs them with `sh`, so no script is built.
fn test_harness(recipes: &Path, name: &str, scripts: &[(&str, &str)]) -> (bool, String) {
    let project = std::env::temp_dir().join(format!("recipetest-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(project.join(".claude/hooks")).unwrap();
    fs::create_dir_all(project.join("bin")).unwrap();
    fs::copy(recipes, project.join("justfile")).unwrap();
    let fake = project.join("bin/rust-script");
    fs::write(&fake, "#!/bin/sh\nexec sh \"$1\"\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    for (file, body) in scripts {
        fs::write(project.join(".claude/hooks").join(file), body).unwrap();
    }
    let path = format!("{}:{}", project.join("bin").display(), std::env::var("PATH").unwrap_or_default());
    let out = Command::new("just")
        .arg("test-harness")
        .current_dir(&project)
        .env("PATH", path)
        .output()
        .expect("just must be installed");
    let _ = fs::remove_dir_all(&project);
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

const PASSING: &str = "echo 'ok   - one'\necho 'ok   - two'\n";
const FAILING: &str = "echo 'ok   - one'\necho 'FAIL - two'\nexit 1\n";

fn main() {
    let here = fs::canonicalize(Path::new(file!()).parent().unwrap_or(Path::new("."))).unwrap();
    let recipes = here.join("../harness.just");
    let mut failed = false;
    let mut check = |name: &str, ok: bool, output: &str| {
        println!("{} - {name}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            println!("       output: {}", output.trim());
        }
        failed |= !ok;
    };

    let (ok, out) = test_harness(&recipes, "pass", &[("test_a.rs", PASSING), ("test_b.rs", PASSING)]);
    check("test-harness passes and prints nothing when every script passes", ok && out.trim().is_empty(), &out);

    let (ok, out) = test_harness(&recipes, "fail", &[("test_a.rs", PASSING), ("test_b.rs", FAILING)]);
    check("test-harness fails when a script fails", !ok, &out);
    check("test-harness shows the failing line", out.contains("FAIL - two"), &out);
    check("test-harness names the failing script", out.contains("test_b.rs"), &out);
    check("test-harness hides the passing lines", !out.contains("ok   -"), &out);

    std::process::exit(i32::from(failed));
}
