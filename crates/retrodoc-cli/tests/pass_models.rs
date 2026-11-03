//! `[llm.passes.<name>]`: a pass calls its own server and model, the others keep `[llm]`.

mod support;

use support::{llm_section, repo_with_config, retrodoc, FakeLlm};

#[test]
fn a_pass_calls_its_own_server_and_not_the_default_one() {
    // Neither server answers anything useful: only the requests they get count.
    let default = FakeLlm::start(|_| "not json".to_string());
    let roles = FakeLlm::start(|_| "not json".to_string());
    let dir = repo_with_config(
        &[("app.rb", "class Order; end\n")],
        &format!(
            "{}\n[llm.passes.roles]\nmodel = \"roles-model\"\nbase_url = \"{}\"\n",
            llm_section(&default),
            roles.url
        ),
    );

    let output = retrodoc(dir.path(), "roles");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!roles.calls().is_empty(), "{stdout}");
    assert_eq!(default.calls().len(), 0, "{stdout}");
}

#[test]
fn an_unknown_pass_stops_the_command_before_any_call() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("retrodoc.toml"),
        "[llm.passes.domain]\nmodel = \"x\"\n",
    )
    .unwrap();
    let output = retrodoc(dir.path(), "roles");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("domain") && stderr.contains("use-cases"),
        "{stderr}"
    );
}
