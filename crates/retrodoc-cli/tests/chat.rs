//! `retrodoc chat` on the artifacts of a run: each line of the input is a question answered like
//! `ask`, after the earlier questions and answers, until the end of the input or `exit`.

mod support;

use retrodoc_core::model::Feature;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use support::{llm_section, repo_with_config, run_with_input, Call, FakeLlm, Turn};

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

/// The content of the last message of `call`: the question it answers.
fn question(call: &Call) -> &str {
    call.messages
        .last()
        .map_or("", |(_, content)| content.as_str())
}

/// A model that reads the feature for the first question, then answers each question by its text.
fn answering() -> FakeLlm {
    FakeLlm::turns(|call| {
        if question(call) == "How are the orders listed?" && call.tool_results.is_empty() {
            Turn::Calls(vec![(
                "c1",
                "get_feature",
                r#"{"id":"orders/list-orders"}"#.to_string(),
            )])
        } else if question(call) == "fail" {
            Turn::Error("400 Bad Request")
        } else {
            let asked = call
                .messages
                .iter()
                .rev()
                .find(|(role, _)| role == "user")
                .map_or("", |(_, c)| c.as_str());
            Turn::Text(format!("About {asked}: see `orders/list-orders`."))
        }
    })
}

fn success(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

// spec: CHT-1, CHT-2, CHT-3
#[test]
fn each_line_is_answered_after_the_earlier_ones_until_exit() {
    let llm = answering();
    let dir = generated_repo(&llm_section(&llm));

    let input = "How are the orders listed?\n\n  \nAnd by whom?\nexit\nNever asked\n";
    let out = success(&run_with_input(dir.path(), &["chat"], input));

    let first = out
        .find("About How are the orders listed?: see `orders/list-orders`.")
        .unwrap_or_else(|| panic!("{out}"));
    let second = out
        .find("About And by whom?: see `orders/list-orders`.")
        .unwrap_or_else(|| panic!("{out}"));
    assert!(first < second, "{out}");
    assert_eq!(
        out.matches("Sources:\n- feature `orders/list-orders`")
            .count(),
        2,
        "{out}"
    );
    let calls = llm.calls();
    assert_eq!(calls.len(), 3, "{out}");
    let roles: Vec<&str> = calls[2].messages.iter().map(|(r, _)| r.as_str()).collect();
    assert_eq!(roles, ["system", "user", "assistant", "user"]);
    assert_eq!(calls[2].messages[1].1, "How are the orders listed?");
    assert!(calls[2].messages[2].1.starts_with("About How are"));
    assert_eq!(calls[2].messages[3].1, "And by whom?");
    assert!(!out.contains("Never asked"), "{out}");
}

// spec: CHT-1, CHT-8
#[test]
fn a_failed_question_prints_its_error_and_the_next_one_is_answered_without_it() {
    let llm = answering();
    let dir = generated_repo(&llm_section(&llm));

    let out = success(&run_with_input(dir.path(), &["chat"], "fail\nAnd by whom?"));

    let error = out
        .find("could not be answered")
        .unwrap_or_else(|| panic!("{out}"));
    let answer = out
        .find("About And by whom?")
        .unwrap_or_else(|| panic!("{out}"));
    assert!(error < answer, "{out}");
    let last = llm.calls().pop().unwrap();
    let roles: Vec<&str> = last.messages.iter().map(|(r, _)| r.as_str()).collect();
    assert_eq!(roles, ["system", "user"]);
}

// spec: CHT-9
#[test]
fn the_chat_uses_the_settings_and_the_recap_of_the_ask_pass_and_writes_nothing() {
    let default = FakeLlm::start(|_| "unused".to_string());
    let ask = answering();
    let dir = generated_repo(&format!(
        "{}[llm.passes.ask]\nmodel = \"ask-model\"\nbase_url = \"{}\"\n",
        llm_section(&default),
        ask.url
    ));
    support::git(dir.path(), &["add", "-A"]);
    support::git(dir.path(), &["commit", "-qm", "docs"]);

    let out = success(&run_with_input(dir.path(), &["chat"], "And by whom?\n"));

    assert_eq!(ask.calls().len(), 1);
    assert!(default.calls().is_empty());
    assert!(
        out.lines()
            .any(|l| l.contains("ask") && l.contains("1 call")),
        "{out}"
    );
    let status = std::process::Command::new("git")
        .args(["status", "--porcelain", "--", ".", ":(exclude).retrodoc"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&status.stdout), "");
}

// spec: CHT-9
#[test]
fn without_generated_docs_the_chat_fails_before_any_call() {
    let llm = answering();
    let dir = repo_with_config(
        &[("app/order.rb", "class Order; end\n")],
        &llm_section(&llm),
    );

    let output = run_with_input(dir.path(), &["chat"], "And by whom?\n");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("retrodoc generate"), "{stderr}");
    assert!(llm.calls().is_empty());
}
