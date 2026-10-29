//! The built-in stack tables cover the reference languages of `specs/language-agnostic.md`: their
//! files are classified by `scan` with no LLM call.

use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=A", "-c", "user.email=a@example.com"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

/// A committed repo holding `files` (empty content) and a `retrodoc.toml`.
fn repo(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in files {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x\n").unwrap();
    }
    std::fs::write(
        dir.path().join("retrodoc.toml"),
        "[llm]\nmodel = \"m\"\nbase_url = \"http://127.0.0.1:9/chat/completions\"\n",
    )
    .unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);
    dir
}

fn scan(dir: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["scan", "--path"])
        .arg(dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// One file per extension of the reference set.
const SOURCES: &[&str] = &[
    "src/a.js",
    "src/a.jsx",
    "src/a.mjs",
    "src/a.cjs",
    "src/a.ts",
    "src/a.tsx",
    "src/a.mts",
    "src/a.cts",
    "src/a.py",
    "src/a.java",
    "src/a.kt",
    "src/main/kotlin/Order.kts",
    "src/a.cs",
    "src/a.php",
    "src/a.go",
    "src/a.rb",
    "src/a.rs",
    "src/a.c",
    "src/a.h",
    "src/a.cpp",
    "src/a.cc",
    "src/a.hpp",
    "src/a.swift",
    "src/a.scala",
    "src/main/java/shop/Contest.java",
];

// spec: LNG-5
#[test]
fn every_reference_extension_is_source() {
    let mut files = SOURCES.to_vec();
    // Gradle build scripts are manifests, not code of the application.
    files.extend(["build.gradle.kts", "settings.gradle.kts"]);
    let dir = repo(&files);

    let out = scan(dir.path());

    // retrodoc.toml and the two Gradle scripts are the other files.
    let expected = format!(
        "(source: {}, test: 0, markdown: 0, other: 3)",
        SOURCES.len()
    );
    assert!(out.contains(&expected), "{out}");
}

// spec: LNG-6
#[test]
fn every_reference_test_layout_is_test() {
    let tests = [
        "src/test/java/shop/Order.java",
        "src/main/java/shop/OrderTest.java",
        "Shop/OrderTests.cs",
        "pkg/order_test.go",
        "app/OrderTest.php",
        "app/test_order.py",
        "web/order.spec.ts",
        "lib/order_spec.rb",
    ];
    let dir = repo(&tests);

    let out = scan(dir.path());

    let expected = format!("(source: 0, test: {}, markdown: 0, other: 1)", tests.len());
    assert!(out.contains(&expected), "{out}");
}
