//! The files no role rule matches get a role from a second call over their paths, kept apart from
//! the rules of `roles.yaml` and asked once per set of such files.

mod support;

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use support::{generate, repo, retrodoc, warnings, App, FakeLlm};

const RULES: &str = r#"[{"pattern": "app/models/**", "role": "model"}]"#;
const COFFEE: &str = "class Cart\n  SECRET_CONTENT = 1\n";

/// A Rails-like repository whose rules match the models only, the fake LLM answering `unmatched`
/// to the second call (changeable between runs).
fn shop(
    unmatched: Value,
    extra: &[(&str, &str)],
) -> (tempfile::TempDir, FakeLlm, Arc<Mutex<Value>>) {
    let answer = Arc::new(Mutex::new(unmatched));
    let script = Arc::clone(&answer);
    let llm = FakeLlm::start(move |call| match call.pass.as_str() {
        "roles" => {
            json!({"stack": "Rails", "rules": serde_json::from_str::<Value>(RULES).unwrap()})
                .to_string()
        }
        "roles-unmatched" => script.lock().unwrap().to_string(),
        other => panic!("no script for {other}"),
    });
    let mut files = vec![
        ("app/models/order.rb", "class Order\nend\n"),
        ("app/assets/javascripts/cart.coffee", COFFEE),
        ("app/assets/javascripts/menu.coffee", "class Menu\n"),
        ("Rakefile", "require 'rake'\n"),
    ];
    files.extend_from_slice(extra);
    let dir = repo(&files, &llm);
    (dir, llm, answer)
}

fn roles(dir: &tempfile::TempDir) -> String {
    let output = retrodoc(dir.path(), "roles");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

fn saved_roles(dir: &tempfile::TempDir) -> String {
    std::fs::read_to_string(dir.path().join(".retrodoc/cache/roles.yaml")).unwrap()
}

/// The paths a call of the second pass listed, one per line.
fn asked_paths(llm: &FakeLlm) -> Vec<Vec<String>> {
    llm.calls_of("roles-unmatched")
        .iter()
        .map(|call| {
            call.user
                .lines()
                .filter_map(|line| line.strip_prefix("- "))
                .map(str::to_string)
                .collect()
        })
        .collect()
}

// spec: LNG-16
#[test]
fn the_unmatched_files_get_a_role_from_their_paths() {
    let (dir, llm, _) = shop(
        json!({"rules": [{"pattern": "app/assets/**/*.coffee", "role": "view"}]}),
        &[],
    );

    let out = roles(&dir);

    let asked = asked_paths(&llm);
    assert_eq!(asked.len(), 1, "{out}");
    assert!(
        asked[0].contains(&"app/assets/javascripts/cart.coffee".to_string()),
        "{asked:?}"
    );
    assert!(asked[0].contains(&"Rakefile".to_string()), "{asked:?}");
    assert!(
        !asked[0].contains(&"app/models/order.rb".to_string()),
        "{asked:?}"
    );
    assert!(!llm.calls_of("roles-unmatched")[0]
        .user
        .contains("SECRET_CONTENT"));
    assert!(out.contains("  view         2"), "{out}");
    assert!(
        saved_roles(&dir).contains("unmatched_rules:"),
        "{}",
        saved_roles(&dir)
    );
}

// spec: LNG-16
#[test]
fn generate_asks_about_the_unmatched_files() {
    let app = App {
        stack: "Elixir",
        source_extensions: vec!["ex"],
        chunk_boundaries: vec![],
        business: "lib/",
        entry: "lib/shop_web/order_controller.ex",
        model: "lib/shop/order.ex",
    };
    let llm = FakeLlm::start(move |call| app.answer(call));
    let dir = repo(
        &[
            ("lib/shop/order.ex", "defmodule Shop.Order do\nend\n"),
            ("priv/repo/seeds.exs", "Shop.Repo.insert!(%Shop.Order{})\n"),
        ],
        &llm,
    );

    let output = generate(dir.path());

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let asked = asked_paths(&llm);
    assert_eq!(asked.len(), 1);
    assert!(
        asked[0].contains(&"priv/repo/seeds.exs".to_string()),
        "{asked:?}"
    );
}

// spec: LNG-17
#[test]
fn an_added_rule_neither_changes_a_matched_file_nor_the_saved_rules() {
    let (dir, _llm, _) = shop(
        json!({"rules": [{"pattern": "app/**", "role": "logic"}]}),
        &[],
    );

    let out = roles(&dir);

    assert!(out.contains("  model        1"), "{out}");
    assert!(out.contains("  logic        2"), "{out}");
    let saved = saved_roles(&dir);
    let (rules, added) = saved.split_once("unmatched_rules:").expect(&saved);
    assert!(
        rules.contains("rules:\n- pattern: app/models/**\n  role: model\n"),
        "{saved}"
    );
    assert!(!rules.contains("app/**"), "{saved}");
    assert!(added.contains("pattern: app/**"), "{saved}");
}

// spec: LNG-18
#[test]
fn the_same_unmatched_files_are_asked_about_once() {
    let (dir, llm, _) = shop(
        json!({"rules": [{"pattern": "**/*.coffee", "role": "view"}]}),
        &[],
    );

    roles(&dir);
    let out = roles(&dir);

    assert_eq!(asked_paths(&llm).len(), 1, "{out}");
    assert!(out.contains("Rakefile"), "left unclassified: {out}");
    assert!(out.contains("no call"), "{out}");
}

// spec: LNG-18
#[test]
fn a_new_unmatched_file_is_asked_about() {
    let (dir, llm, answer) = shop(
        json!({"rules": [{"pattern": "**/*.coffee", "role": "view"}]}),
        &[],
    );
    roles(&dir);
    std::fs::write(dir.path().join("Procfile"), "web: rails s\n").unwrap();
    *answer.lock().unwrap() = json!({"rules": [{"pattern": "Procfile", "role": "infra"}]});

    let out = roles(&dir);

    let asked = asked_paths(&llm);
    assert_eq!(asked.len(), 2, "{out}");
    assert!(asked[1].contains(&"Procfile".to_string()), "{asked:?}");
    assert!(
        !asked[1].iter().any(|p| p.ends_with(".coffee")),
        "{asked:?}"
    );
    assert!(out.contains("  infra        1"), "{out}");
    assert!(
        out.contains("  view         2"),
        "the first answer is kept: {out}"
    );
}

// spec: LNG-19
#[test]
fn the_call_sends_at_most_400_paths_and_warns_of_the_files_left() {
    let names: Vec<String> = (0..450).map(|i| format!("data/f{i:03}.dat")).collect();
    let extra: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x")).collect();
    let (dir, llm, _) = shop(json!({"rules": []}), &extra);

    let output = retrodoc(dir.path(), "roles");

    assert!(output.status.success());
    let asked = asked_paths(&llm);
    assert_eq!(asked[0].len(), 400, "{:?}", asked[0].len());
    let warned = warnings(&output);
    assert!(
        warned
            .iter()
            .any(|w| w.contains("left unclassified") && w.contains("left=454")),
        "{warned:?}"
    );
}
