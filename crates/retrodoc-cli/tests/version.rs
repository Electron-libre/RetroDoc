//! The binary reports the version of the latest release: `just release <version>` writes the `CHANGELOG.md`
//! section but leaves `[workspace.package] version` alone, so this test fails until the version is bumped, before
//! the release is committed and tagged.

use std::process::Command;

#[test]
fn the_version_is_the_one_of_the_latest_changelog_section() {
    let changelog = include_str!("../../../CHANGELOG.md");
    let latest = changelog
        .lines()
        .filter_map(|line| line.strip_prefix("## ["))
        .filter_map(|rest| rest.split_once(']').map(|(version, _)| version))
        .find(|version| !version.eq_ignore_ascii_case("unreleased"))
        .expect("CHANGELOG.md has a `## [x.y.z]` section");
    let out = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("retrodoc {latest}"),
        "set `[workspace.package] version` of Cargo.toml to the latest CHANGELOG.md section"
    );
}
