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

/// A Rails app whose saved `roles.yaml` names template and stylesheet extensions.
fn rails(llm: &FakeLlm) -> tempfile::TempDir {
    let roles = "stack: Rails\nrules:\n- pattern: app/**\n  role: logic\nsource_extensions:\n\
                 - rb\n- erb\n- jbuilder\n- scss\n- vue\n- json\n";
    repo(
        &[
            ("app/models/order.rb", "class Order\nend\n"),
            (
                "app/views/orders/show.html.erb",
                "<h1><%= @order.id %></h1>\n",
            ),
            ("app/views/orders/show.json.jbuilder", "json.id @order.id\n"),
            ("app/assets/stylesheets/app.scss", "h1 { color: red; }\n"),
            ("app/javascript/Order.vue", "<template><p/></template>\n"),
            (".retrodoc/cache/roles.yaml", roles),
        ],
        llm,
    )
}

// spec: LNG-20
#[test]
fn templates_stylesheets_and_data_are_never_promoted() {
    let llm = FakeLlm::start(|call| panic!("no call expected, got {}", call.pass));
    let dir = rails(&llm);

    let out = stdout_of(dir.path(), "scan");

    assert!(out.contains("source: 2,"), "{out}");
    // No file holds `json`: it is not reported.
    assert!(
        out.contains("not counted as source: erb, jbuilder, scss ("),
        "{out}"
    );
}

// spec: LNG-20
#[test]
fn roles_prints_the_extensions_left_out() {
    let llm = FakeLlm::start(|call| panic!("no call expected, got {}", call.pass));
    let dir = rails(&llm);

    let out = stdout_of(dir.path(), "roles");

    assert!(
        out.contains("not counted as source: erb, jbuilder, scss ("),
        "{out}"
    );
}

// spec: LNG-21
#[test]
fn templates_of_a_built_in_extension_are_never_promoted() {
    let llm = FakeLlm::start(|call| panic!("no call expected, got {}", call.pass));
    let roles =
        "stack: Laravel\nrules:\n- pattern: '**'\n  role: logic\nsource_extensions:\n- php\n";
    let dir = repo(
        &[
            (
                "app/Models/Order.php",
                "<?php\nclass Order extends Model {}\n",
            ),
            (
                "resources/views/orders/index.blade.php",
                "<h1>{{ $order->id }}</h1>\n",
            ),
            ("templates/order.html.php", "<h1><?= $order->id ?></h1>\n"),
            ("themes/node.tpl.php", "<h1><?php print $title; ?></h1>\n"),
            (".retrodoc/cache/roles.yaml", roles),
        ],
        &llm,
    );

    let out = stdout_of(dir.path(), "scan");

    assert!(out.contains("source: 1,"), "{out}");
}
