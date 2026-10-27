//! `retrodoc benchmark` without the judge: reads the artifacts of a run and a reference, no LLM.

use std::process::Command;

const REFERENCE: &str = "repository: https://example.com/shop
commit: abc123
purpose: Sells things.
actors: [Customer]
domains:
  - name: Ordering
    description: Placing orders.
    features:
      - {name: Checkout, description: Pay for a cart.}
      - {name: Refund, description: Give money back.}
";

fn retrodoc(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn prints_the_scores_of_a_run_against_the_reference() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join(".retrodoc/cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join("domains.yaml"),
        "domains:\n  - {slug: ordering, name: Ordering, description: d}\n",
    )
    .unwrap();
    std::fs::write(
        cache.join("features.yaml"),
        "- {slug: checkout, domain_slug: ordering, name: Checkout, description: d}\n\
         - {slug: news, domain_slug: ordering, name: Newsletter, description: d}\n",
    )
    .unwrap();
    let reference = dir.path().join("reference.yaml");
    std::fs::write(&reference, REFERENCE).unwrap();
    let path = dir.path().to_str().unwrap();
    let reference = reference.to_str().unwrap();

    let output = retrodoc(&["benchmark", "--path", path, "--reference", reference]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Run: 1 domain(s), 0 sub-domain(s), 2 feature(s)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Domains: recall 100%, precision 100%"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Features: recall 50%, precision 50%"),
        "{stdout}"
    );
    assert!(
        stdout.contains("in the reference, not generated: Refund"),
        "{stdout}"
    );
}

#[test]
fn says_to_generate_first_when_the_run_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("reference.yaml");
    std::fs::write(&reference, REFERENCE).unwrap();

    let output = retrodoc(&[
        "benchmark",
        "--path",
        dir.path().to_str().unwrap(),
        "--reference",
        reference.to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("run `retrodoc generate` first"), "{stderr}");
}

#[test]
fn tabulates_saved_runs_and_their_change_against_a_previous_benchmark() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("clone/.retrodoc/cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join("domains.yaml"),
        "domains:\n  - {slug: ordering, name: Ordering, description: d}\n",
    )
    .unwrap();
    std::fs::write(
        cache.join("features.yaml"),
        "- {slug: checkout, domain_slug: ordering, name: Checkout, description: d}\n",
    )
    .unwrap();
    let reference = dir.path().join("reference.yaml");
    std::fs::write(&reference, REFERENCE).unwrap();
    let measure = |out: &std::path::Path| {
        let output = retrodoc(&[
            "benchmark",
            "--path",
            dir.path().join("clone").to_str().unwrap(),
            "--reference",
            reference.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    measure(&first.join("hidden/run-1.json"));
    // The next benchmark finds one more feature.
    std::fs::write(
        cache.join("features.yaml"),
        "- {slug: checkout, domain_slug: ordering, name: Checkout, description: d}\n\
         - {slug: refund, domain_slug: ordering, name: Refund, description: d}\n",
    )
    .unwrap();
    measure(&second.join("hidden/run-1.json"));

    let output = retrodoc(&[
        "benchmark-table",
        second.to_str().unwrap(),
        "--previous",
        first.to_str().unwrap(),
    ]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("## hidden (1 run(s))"), "{stdout}");
    assert!(
        stdout.contains("| Features recall | 100% | — | 1 | 50% | +50 pts |"),
        "{stdout}"
    );
    assert!(
        stdout.contains("| Features | 2 | — | 1 | 1 | +1 |"),
        "{stdout}"
    );
}

// spec: BSC-8
#[test]
fn rescores_a_run_with_its_saved_judgement_and_no_llm() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join(".retrodoc/cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join("domains.yaml"),
        "domains:\n  - {slug: ordering, name: Ordering, description: d}\n",
    )
    .unwrap();
    std::fs::write(
        cache.join("features.yaml"),
        "- {slug: pay, domain_slug: ordering, name: Pay and refund, description: d}\n",
    )
    .unwrap();
    let reference = dir.path().join("reference.yaml");
    std::fs::write(&reference, REFERENCE).unwrap();
    let out = dir.path().join("run.json");
    let args = [
        "benchmark",
        "--path",
        dir.path().to_str().unwrap(),
        "--reference",
        reference.to_str().unwrap(),
        "--saved-judge",
        "--out",
        out.to_str().unwrap(),
    ];

    let missing = retrodoc(&args);
    assert!(!missing.status.success());
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(stderr.contains("judge.yaml"), "{stderr}");

    std::fs::create_dir_all(dir.path().join(".retrodoc/benchmark")).unwrap();
    std::fs::write(
        dir.path().join(".retrodoc/benchmark/judge.yaml"),
        "domains: []\n\
         features:\n  - {generated: Pay and refund, reference: Checkout}\n  \
         - {generated: Pay and refund, reference: Refund}\n\
         narratives: {total: 4, rated: 4, business: 3}\n",
    )
    .unwrap();
    // No `retrodoc.toml` and no key: a call to the judge would fail.
    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(args)
        .env_remove("OPENROUTER_API_KEY")
        .env_remove("DEEPSEEK_API_KEY")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(report["strict"]["features"]["recall"], 0.0);
    assert_eq!(report["judged"]["features"]["recall"], 1.0);
    assert_eq!(report["narratives"]["business"], 3);
    assert_eq!(report["scoring"], 1);
}

#[test]
fn an_empty_directory_has_no_table() {
    let dir = tempfile::tempdir().unwrap();
    let output = retrodoc(&["benchmark-table", dir.path().to_str().unwrap()]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no run found"), "{stderr}");
}
