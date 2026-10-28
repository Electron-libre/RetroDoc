//! A `retrodoc.toml` naming a removed setting stops the command with what to change, not a request to run
//! `init` (`specs/pipeline-settings.md`).

use std::process::Command;

// spec: SET-8
#[test]
fn a_removed_path_is_named_first_and_the_command_stops() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("retrodoc.toml"),
        "[pipeline]\ndomains_from = \"files\"\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["generate", "--no-confidence"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{stderr}");
    let first = stderr.lines().next().unwrap_or_default();
    assert!(
        first.contains("[pipeline] domains_from = \"files\" was removed"),
        "{stderr}"
    );
    assert!(first.contains("domains_from = \"surface\""), "{stderr}");
    assert!(!stderr.contains("retrodoc init"), "{stderr}");
}
