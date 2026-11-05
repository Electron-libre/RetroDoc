//! `retrodoc ask` on the artifacts of a run: the model navigates them with the read-only tools, the
//! command prints the answer, its sources and the recap of its calls, and writes nothing but its usage.

mod support;

use std::collections::BTreeMap;
use std::path::Path;

use retrodoc_core::model::Feature;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use support::{llm_section, repo_with_config, run, FakeLlm, Turn};

const QUESTION: &str = "How are the orders listed?";

/// A repository whose last `generate` documented one feature grounded on `app/order.rb`.
fn generated_repo(config: &str) -> tempfile::TempDir {
    let dir = repo_with_config(&[("app/order.rb", "class Order; end\n")], config);
    let feature = Feature {
        back_office: false,
        technical: false,
        slug: "list-orders".into(),
        domain_slug: "orders".into(),
        name: "List orders".into(),
        description: "The customer sees their orders".into(),
        source_paths: vec!["app/order.rb".into()],
        entry_points: Vec::new(),
        confidence: None,
    };
    retrodoc_pipeline::save_features(dir.path(), &[feature]).unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "orders".into(),
            name: "Orders".into(),
            description: "Orders of the shop".into(),
            paths: vec![],
            ..DomainCluster::default()
        }],
    }
    .save(dir.path())
    .unwrap();
    dir
}

/// A model that reads the feature, then answers naming it, its file and a feature it never read.
fn navigating() -> FakeLlm {
    FakeLlm::turns(|call| {
        if call.tool_results.is_empty() {
            Turn::Calls(vec![(
                "c1",
                "get_feature",
                r#"{"id":"orders/list-orders"}"#.to_string(),
            )])
        } else {
            Turn::Text(
                "The customer lists them in `orders/list-orders`, coded in `app/order.rb`; \
                 refunds are in `orders/refund`."
                    .to_string(),
            )
        }
    })
}

fn stdout(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

/// Every file under `dir` but `.git` and `.retrodoc`, with its content.
fn files_outside_the_workspace(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(folder) = todo.pop() {
        for entry in std::fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            let name = path
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if name == ".git" || name == ".retrodoc" {
                continue;
            }
            if path.is_dir() {
                todo.push(path);
            } else {
                files.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }
    files
}

// spec: ASK-12, ASK-14
#[test]
fn the_answer_is_printed_then_its_sources_then_the_recap_of_the_ask_pass() {
    let llm = navigating();
    let dir = generated_repo(&llm_section(&llm));

    let out = stdout(&run(dir.path(), &["ask", QUESTION]));

    let text = out
        .find("The customer lists them in `orders/list-orders`")
        .unwrap_or_else(|| panic!("{out}"));
    let sources = out.find("Sources:\n- feature `orders/list-orders`\n- file `app/order.rb`\n");
    let sources = sources.unwrap_or_else(|| panic!("{out}"));
    assert!(text < sources, "{out}");
    assert!(!out.contains("- feature `orders/refund`"), "{out}");
    assert!(
        out[sources..]
            .lines()
            .any(|l| l.contains("ask") && l.contains("2 call")),
        "{out}"
    );
    let calls = llm.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].user, QUESTION);
    assert!(calls[0].tools.contains(&"get_feature".to_string()));
    assert!(
        calls[1].tool_results[0].contains("# List orders"),
        "{:?}",
        calls[1]
    );
}

// spec: ASK-13
#[test]
fn the_ask_pass_calls_the_server_of_its_own_settings() {
    let default = FakeLlm::start(|_| "unused".to_string());
    let ask = navigating();
    let dir = generated_repo(&format!(
        "{}[llm.passes.ask]\nmodel = \"ask-model\"\nbase_url = \"{}\"\n",
        llm_section(&default),
        ask.url
    ));

    stdout(&run(dir.path(), &["ask", QUESTION]));

    assert_eq!(ask.calls().len(), 2);
    assert_eq!(default.calls().len(), 0);
}

// spec: ASK-6
#[test]
fn max_rounds_bounds_the_rounds_of_tool_calls() {
    let llm = navigating();
    let dir = generated_repo(&llm_section(&llm));

    let out = stdout(&run(dir.path(), &["ask", QUESTION, "--max-rounds", "0"]));

    let calls = llm.calls();
    assert_eq!(calls.len(), 1, "{out}");
    assert_eq!(calls[0].tools, Vec::<String>::new());
}

// spec: ASK-15
#[test]
fn the_question_writes_nothing_outside_the_workspace() {
    let llm = navigating();
    let dir = generated_repo(&llm_section(&llm));
    let before = files_outside_the_workspace(dir.path());

    stdout(&run(dir.path(), &["ask", QUESTION]));

    assert_eq!(files_outside_the_workspace(dir.path()), before);
}

// spec: ASK-16
#[test]
fn without_generated_docs_the_command_fails_before_any_call() {
    let llm = navigating();
    let dir = repo_with_config(
        &[("app/order.rb", "class Order; end\n")],
        &llm_section(&llm),
    );

    let output = run(dir.path(), &["ask", QUESTION]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("retrodoc generate"), "{stderr}");
    assert!(llm.calls().is_empty());
}
