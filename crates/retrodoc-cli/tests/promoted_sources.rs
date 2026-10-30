//! `scan` and `roles` count the files of the languages the saved `roles.yaml` names as source, as
//! `generate` reads them.

mod support;

use support::{repo, stdout_of, FakeLlm};

/// Every file matched, so `roles` has no call to make.
const ROLES: &str =
    "stack: Elixir\nrules:\n- pattern: lib/**\n  role: logic\n- pattern: '*.toml'\n  \
                     role: config\nsource_extensions:\n- ex\n";

/// An Elixir shop the walker does not know, with the roles saved or not.
fn shop(llm: &FakeLlm, roles_saved: bool) -> tempfile::TempDir {
    let mut files = vec![
        ("lib/shop/order.ex", "defmodule Shop.Order do\nend\n"),
        ("test/shop_test.ex", "defmodule ShopTest do\nend\n"),
    ];
    if roles_saved {
        files.push((".retrodoc/cache/roles.yaml", ROLES));
    }
    repo(&files, llm)
}

// spec: LNG-15
#[test]
fn scan_counts_the_promoted_sources() {
    let llm = FakeLlm::start(|call| panic!("no call expected, got {}", call.pass));
    let dir = shop(&llm, true);

    let out = stdout_of(dir.path(), "scan");

    assert!(out.contains("source: 1,"), "{out}");
    assert!(out.contains("test: 1,"), "{out}");
    assert!(
        out.contains("2 file(s) of the languages of roles.yaml"),
        "{out}"
    );
}

// spec: LNG-15
#[test]
fn scan_without_saved_roles_keeps_the_built_in_kinds() {
    let llm = FakeLlm::start(|call| panic!("no call expected, got {}", call.pass));
    let dir = shop(&llm, false);

    let out = stdout_of(dir.path(), "scan");

    assert!(out.contains("source: 0,"), "{out}");
    assert!(!out.contains("languages of roles.yaml"), "{out}");
}

// spec: LNG-15
#[test]
fn roles_counts_the_promoted_sources() {
    let llm = FakeLlm::start(|call| panic!("no call expected, got {}", call.pass));
    let dir = shop(&llm, true);

    let out = stdout_of(dir.path(), "roles");

    assert!(
        out.contains("2 file(s) of the languages of roles.yaml"),
        "{out}"
    );
    assert!(out.contains("source: 1"), "{out}");
}
