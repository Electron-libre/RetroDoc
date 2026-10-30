//! `generate` goes through a repository of a language no built-in table knows (Elixir: no known
//! extension, manifest, schema nor grammar), from what the roles call names.

mod support;

use std::fmt::Write as _;

use support::{generate, repo, App, FakeLlm};

const CATALOG: &str = "lib/shop/catalog.ex";

/// A 20 KB module of blocks apart by blank lines, none of them starting like a `def`.
fn catalog() -> String {
    let mut text = String::from("defmodule Shop.Catalog do\n\n");
    let mut n = 0;
    while text.len() < 20_000 {
        n += 1;
        let _ = write!(
            text,
            "  # block {n}\n  @price_{n} {n}\n  def price_{n}, do: @price_{n}\n  def label_{n}, do: \"item {n}\"\n\n"
        );
    }
    text.push_str("end\n");
    text
}

/// Runs `generate` on an Elixir shop, the roles call naming `.EX` as source and a boundary regex
/// that matches no line.
fn run() -> (tempfile::TempDir, FakeLlm, std::process::Output) {
    let app = App {
        stack: "Elixir, Phoenix",
        source_extensions: vec![".EX"],
        chunk_boundaries: vec![("ex", "^NEVER MATCHES$")],
        business: "lib/",
        entry: "lib/shop_web/order_controller.ex",
        model: "lib/shop/order.ex",
    };
    // The business files also name the test file: read only if it were taken for source.
    let llm = FakeLlm::start(move |call| match call.pass.as_str() {
        "business-files" => serde_json::json!({"business": [
            {"path": "lib/", "reason": "the shop"},
            {"path": "test/shop_test.ex", "reason": "the shop"},
        ]})
        .to_string(),
        _ => app.answer(call),
    });
    let dir = repo(
        &[
            (
                "mix.exs",
                "defmodule Shop.MixProject do\n  use Mix.Project\n  def project, do: [app: :shop]\nend\n",
            ),
            (
                "lib/shop/order.ex",
                "defmodule Shop.Order do\n  defstruct [:total, :lines]\nend\n",
            ),
            (
                "lib/shop_web/order_controller.ex",
                "defmodule ShopWeb.OrderController do\n  def index(conn, _params) do\n    \
                 json(conn, Shop.Orders.list())\n  end\nend\n",
            ),
            (CATALOG, &catalog()),
            (
                "test/shop_test.ex",
                "defmodule ShopTest do\n  use ExUnit.Case\n  test \"lists\", do: assert true\nend\n",
            ),
        ],
        &llm,
    );
    let output = generate(dir.path());
    (dir, llm, output)
}

/// The paths every reading call was given.
fn read_paths(llm: &FakeLlm) -> Vec<String> {
    ["business-read", "glossary", "entry-points"]
        .iter()
        .flat_map(|pass| llm.calls_of(pass))
        .flat_map(|call| call.files().into_iter().map(|(path, _)| path))
        .collect()
}

// spec: LNG-1
#[test]
fn generate_documents_a_repository_of_an_unknown_language() {
    let (dir, _llm, output) = run();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "{stdout}");
    let functional = dir.path().join("docs/functional");
    assert!(functional.join("orders/README.md").is_file(), "{stdout}");
    let entry_points =
        std::fs::read_to_string(dir.path().join(".retrodoc/cache/entry-points.yaml"))
            .unwrap_or_default();
    assert!(
        entry_points.contains("lib/shop_web/order_controller.ex")
            && entry_points.contains("GET /orders"),
        "{entry_points}\n{stdout}"
    );
}

// spec: LNG-2
#[test]
fn a_named_extension_is_read_as_source_and_its_tests_as_tests() {
    let (_dir, llm, output) = run();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let read = read_paths(&llm);

    assert!(
        read.contains(&"lib/shop/order.ex".to_string()),
        "{read:?}\n{stdout}"
    );
    assert!(!read.contains(&"test/shop_test.ex".to_string()), "{read:?}");
}

// spec: LNG-3
#[test]
fn a_long_file_of_an_unparsed_language_is_cut_at_blank_lines() {
    let (_dir, llm, output) = run();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<String> = ["business-read", "glossary", "entry-points"]
        .iter()
        .flat_map(|pass| llm.calls_of(pass))
        .flat_map(|call| call.files())
        .filter(|(path, _)| path == CATALOG)
        .map(|(_, text)| text)
        .collect();

    assert!(parts.len() >= 2, "{parts:?}\n{stdout}");
    assert!(!stdout.contains("file too long"), "{stdout}");
    for part in parts.iter().filter(|p| !p.starts_with("(part 1/")) {
        let first = part.lines().nth(1).unwrap_or_default();
        assert!(
            first.starts_with("  # block"),
            "a part starts with {first:?}"
        );
    }
}
