//! The files no role rule matches get a role from a second call over their paths, saved apart from
//! `roles.yaml`, each path sent once.

mod support;

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use support::{generate, repo, retrodoc, stdout_of, warnings, App, FakeLlm};

const COFFEE: &str = "class Cart\n  SECRET_CONTENT = 1\n";
/// A hand-written `roles.yaml` matching the models only.
const SAVED_ROLES: &str =
    "# kept by hand\nstack: Rails\nrules:\n- pattern: app/models/**\n  role: model\n";

/// A Rails-like repository whose rules match the models only, the fake LLM answering `unmatched`
/// to the second call (changeable between runs, `null` for a server error).
fn shop(
    unmatched: Value,
    extra: &[(&str, &str)],
) -> (tempfile::TempDir, FakeLlm, Arc<Mutex<Value>>) {
    let answer = Arc::new(Mutex::new(unmatched));
    let script = Arc::clone(&answer);
    let llm = FakeLlm::start(move |call| match call.pass.as_str() {
        "roles-unmatched" => {
            let answer = script.lock().unwrap().clone();
            assert!(!answer.is_null(), "server error");
            answer.to_string()
        }
        other => panic!("no script for {other}"),
    });
    let mut files = vec![
        (".retrodoc/cache/roles.yaml", SAVED_ROLES),
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
    stdout_of(dir.path(), "roles")
}

fn cached(dir: &tempfile::TempDir, file: &str) -> String {
    std::fs::read_to_string(dir.path().join(".retrodoc/cache").join(file)).unwrap_or_default()
}

/// The paths each call of the second pass listed, one per line.
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

fn coffee_is_view() -> Value {
    json!({"rules": [{"pattern": "**/*.coffee", "role": "view"}]})
}

// spec: LNG-16
#[test]
fn the_unmatched_files_get_a_role_from_their_paths() {
    let (dir, llm, _) = shop(coffee_is_view(), &[]);

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
    let added = cached(&dir, "roles-unmatched.yaml");
    assert!(added.contains("pattern: '**/*.coffee'"), "{added}");
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

// spec: LNG-16
#[test]
fn rules_that_match_nothing_are_not_completed_path_by_path() {
    let llm = FakeLlm::start(|call| panic!("no call expected, got {}", call.pass));
    let dir = repo(
        &[
            (".retrodoc/cache/roles.yaml", "stack: Rails\nrules: []\n"),
            ("app/models/order.rb", "class Order\nend\n"),
        ],
        &llm,
    );

    let output = retrodoc(dir.path(), "roles");

    assert!(llm.calls().is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no role rules identified"));
}

// spec: LNG-16
#[test]
fn a_failed_call_leaves_the_files_unclassified_and_the_pass_goes_on() {
    let (dir, _llm, _) = shop(Value::Null, &[]);

    let output = retrodoc(dir.path(), "roles");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("  model        1"), "{stdout}");
    assert!(
        warnings(&output)
            .iter()
            .any(|w| w.contains("could not be asked")),
        "{stdout}"
    );
}

// spec: LNG-17
#[test]
fn an_added_rule_neither_changes_a_matched_file_nor_roles_yaml() {
    let (dir, _llm, _) = shop(
        json!({"rules": [{"pattern": "app/**", "role": "logic"}]}),
        &[],
    );

    let out = roles(&dir);

    assert!(out.contains("  model        1"), "{out}");
    assert!(out.contains("  logic        2"), "{out}");
    assert_eq!(cached(&dir, "roles.yaml"), SAVED_ROLES);
}

// spec: LNG-18
#[test]
fn a_rerun_sends_no_path_again() {
    let (dir, llm, _) = shop(coffee_is_view(), &[]);
    roles(&dir);
    std::fs::write(
        dir.path().join("app/assets/javascripts/new.coffee"),
        "class New\n",
    )
    .unwrap();

    let out = roles(&dir);

    assert_eq!(asked_paths(&llm).len(), 1, "{out}");
    assert!(out.contains("Rakefile"), "left unclassified: {out}");
    assert!(out.contains("no call"), "{out}");
}

// spec: LNG-18
#[test]
fn a_new_unmatched_file_is_sent_alone() {
    let (dir, llm, answer) = shop(coffee_is_view(), &[]);
    roles(&dir);
    std::fs::write(dir.path().join("Procfile"), "web: rails s\n").unwrap();
    *answer.lock().unwrap() = json!({"rules": [{"pattern": "Procfile", "role": "infra"}]});

    let out = roles(&dir);

    let asked = asked_paths(&llm);
    assert_eq!(asked.len(), 2, "{out}");
    assert_eq!(asked[1], vec!["Procfile".to_string()]);
    assert!(out.contains("  infra        1"), "{out}");
    assert!(
        out.contains("  view         2"),
        "the first answer is kept: {out}"
    );
}

// spec: LNG-19
#[test]
fn a_call_sends_at_most_400_paths_and_the_next_run_the_others() {
    let names: Vec<String> = (0..450).map(|i| format!("data/f{i:03}.dat")).collect();
    let extra: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x")).collect();
    let (dir, llm, _) = shop(json!({"rules": []}), &extra);

    let output = retrodoc(dir.path(), "roles");
    roles(&dir);

    assert!(output.status.success());
    let asked = asked_paths(&llm);
    assert_eq!(asked[0].len(), 400);
    assert!(
        asked[0].contains(&"Rakefile".to_string()),
        "the shallowest first"
    );
    assert_eq!(asked[1].len(), 54);
    let warned = warnings(&output);
    assert!(
        warned.iter().any(|w| w.contains("left unclassified")
            && w.contains("left=454")
            && w.contains("not_sent=54")),
        "{warned:?}"
    );
}
