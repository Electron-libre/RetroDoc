//! `retrodoc benchmark`: reads the artifacts of a run and a reference, no LLM but the judge's second
//! call, served by a fake LLM.

mod support;

use std::fmt::Write as _;
use std::path::Path;
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
        stdout.contains("Run: 1 domain(s), 2 feature(s)"),
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

// spec: BSC-20
#[test]
fn a_domain_with_no_feature_is_not_counted() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join(".retrodoc/cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join("domains.yaml"),
        "domains:\n  - {slug: ordering, name: Ordering, description: d}\n  \
         - {slug: feeds, name: Feeds, description: d}\n",
    )
    .unwrap();
    std::fs::write(
        cache.join("features.yaml"),
        "- {slug: checkout, domain_slug: ordering, name: Checkout, description: d}\n",
    )
    .unwrap();
    let reference = dir.path().join("reference.yaml");
    std::fs::write(&reference, REFERENCE).unwrap();

    let output = retrodoc(&[
        "benchmark",
        "--path",
        dir.path().to_str().unwrap(),
        "--reference",
        reference.to_str().unwrap(),
    ]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        stdout.contains("Run: 1 domain(s), 1 feature(s)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Domains: recall 100%, precision 100%"),
        "{stdout}"
    );
    assert!(!stdout.contains("Feeds"), "{stdout}");
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
    assert_eq!(
        report["scoring"],
        retrodoc_bench::benchmark::SCORING_VERSION
    );
}

/// A run of one domain and the `features` given, its saved judgement holding the feature pairs given.
fn judged_run(features: &[&str], proposals: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join(".retrodoc/cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join("domains.yaml"),
        "domains:\n  - {slug: ordering, name: Ordering, description: d}\n",
    )
    .unwrap();
    let features = features
        .iter()
        .enumerate()
        .fold(String::new(), |mut out, (i, name)| {
            let _ = writeln!(
                out,
                "- {{slug: f{i}, domain_slug: ordering, name: {name}, description: d}}"
            );
            out
        });
    std::fs::write(cache.join("features.yaml"), features).unwrap();
    let proposals = proposals
        .iter()
        .fold(String::new(), |mut out, (generated, reference)| {
            let _ = writeln!(
                out,
                "  - {{generated: {generated}, reference: {reference}}}"
            );
            out
        });
    std::fs::create_dir_all(dir.path().join(".retrodoc/benchmark")).unwrap();
    std::fs::write(
        dir.path().join(".retrodoc/benchmark/judge.yaml"),
        format!(
            "domains: []\nfeatures:\n{proposals}narratives: {{total: 0, rated: 0, business: 0}}\n"
        ),
    )
    .unwrap();
    std::fs::write(dir.path().join("reference.yaml"), REFERENCE).unwrap();
    dir
}

/// `retrodoc benchmark --saved-judge --out` on the run in `dir`, with `matches` as its `matches.yaml`.
fn rescore(dir: &Path, matches: &str) -> (std::process::Output, Option<serde_json::Value>) {
    std::fs::write(dir.join("matches.yaml"), matches).unwrap();
    let out = dir.join("run.json");
    let output = retrodoc(&[
        "benchmark",
        "--path",
        dir.to_str().unwrap(),
        "--reference",
        dir.join("reference.yaml").to_str().unwrap(),
        "--saved-judge",
        "--out",
        out.to_str().unwrap(),
    ]);
    let report = std::fs::read_to_string(&out)
        .ok()
        .map(|raw| serde_json::from_str(&raw).unwrap());
    (output, report)
}

// spec: BSC-16, BSC-18
#[test]
fn a_rejected_proposal_does_not_count_and_the_unaudited_ones_are_counted() {
    let dir = judged_run(
        &["Pay", "Give back", "Bundle services"],
        &[
            ("Pay", "Checkout"),
            ("Give back", "Refund"),
            ("Bundle services", "Refund"),
        ],
    );

    let (output, report) = rescore(
        dir.path(),
        "features:\n  - {generated: Pay, reference: Checkout}\n\
         rejected:\n  features:\n    - {generated: bundle-services, reference: refund}\n",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = report.unwrap();
    assert_eq!(
        report["judged"]["features"]["unmatched_generated"],
        serde_json::json!(["Bundle services"])
    );
    assert_eq!(report["unaudited"]["features"], 1, "{report}");
    assert_eq!(report["unaudited"]["domains"], 0, "{report}");
    assert!(
        stdout.contains("1 of the judge's pairs not audited by hand"),
        "{stdout}"
    );

    let (output, report) = rescore(
        dir.path(),
        "features:\n  - {generated: Pay, reference: Checkout}\n",
    );
    assert!(output.status.success());
    let report = report.unwrap();
    assert_eq!(
        report["judged"]["features"]["unmatched_generated"],
        serde_json::json!([]),
        "with no rejected pair, the proposals count as before"
    );
    assert_eq!(report["unaudited"]["features"], 2, "{report}");
}

// spec: BSC-17
#[test]
fn a_rejected_pair_naming_an_unknown_reference_item_is_an_error() {
    let dir = judged_run(&["Pay"], &[("Pay", "Checkout")]);

    let (output, _) = rescore(
        dir.path(),
        "rejected:\n  features:\n    - {generated: Pay, reference: Typo}\n",
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("`Typo`"), "{stderr}");
    assert!(stderr.contains("matches.yaml"), "{stderr}");
}

// spec: BSC-19
#[test]
fn a_pair_both_kept_and_rejected_is_an_error() {
    let dir = judged_run(&["Pay"], &[("Pay", "Checkout")]);

    let (output, _) = rescore(
        dir.path(),
        "features:\n  - {generated: Pay, reference: Checkout}\n\
         rejected:\n  features:\n    - {generated: pay, reference: checkout}\n",
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("`pay`"), "{stderr}");
    assert!(stderr.contains("both"), "{stderr}");
}

#[test]
fn an_empty_directory_has_no_table() {
    let dir = tempfile::tempdir().unwrap();
    let output = retrodoc(&["benchmark-table", dir.path().to_str().unwrap()]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no run found"), "{stderr}");
}

// spec: BSC-22
#[test]
fn a_saved_judgement_gets_its_second_call_alone() {
    let dir = judged_run(
        &["Pay and refund", "Checkout button"],
        &[("Pay and refund", "Refund")],
    );
    let llm = support::FakeLlm::start(|_| {
        r#"{"matches":[{"generated":"Checkout button","reference":"Checkout"}]}"#.to_string()
    });
    std::fs::write(dir.path().join("retrodoc.toml"), support::llm_section(&llm)).unwrap();
    std::fs::write(
        dir.path().join("matches.yaml"),
        "features:\n  - {generated: Pay and refund, reference: Checkout}\n",
    )
    .unwrap();
    let reference = dir.path().join("reference.yaml");
    let out = dir.path().join("run.json");
    let benchmark = |extra: &[&str]| {
        let mut args = vec![
            "benchmark",
            "--reference",
            reference.to_str().unwrap(),
            "--saved-judge",
            "--out",
            out.to_str().unwrap(),
        ];
        args.extend_from_slice(extra);
        let output = support::run(dir.path(), &args);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            output.status.success(),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout
    };

    let before = benchmark(&[]);
    assert!(
        before.contains("second call") && before.contains("biased low"),
        "{before}"
    );
    assert!(llm.calls().is_empty());

    let after = benchmark(&["--second-call"]);
    let calls = llm.calls();
    assert_eq!(calls.len(), 1, "the domain is paired: one features call");
    let (generated, offered) = calls[0].user.split_once("Reference features:").unwrap();
    assert!(
        generated.contains("- Checkout button:") && !generated.contains("- Pay and refund:"),
        "{generated}"
    );
    assert!(
        offered.contains("- Checkout:") && !offered.contains("- Refund:"),
        "{offered}"
    );
    assert!(!after.contains("biased low"), "{after}");
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(report["judged"]["features"]["precision"], 1.0);
    assert_eq!(report["judged"]["features"]["recall"], 1.0);
    let saved = std::fs::read_to_string(dir.path().join(".retrodoc/benchmark/judge.yaml")).unwrap();
    assert!(
        saved.contains("generated: Pay and refund") && saved.contains("generated: Checkout button"),
        "{saved}"
    );

    benchmark(&["--second-call"]);
    assert_eq!(llm.calls().len(), 1, "every second call already ran");

    let alone = retrodoc(&[
        "benchmark",
        "--reference",
        reference.to_str().unwrap(),
        "--second-call",
    ]);
    assert!(!alone.status.success(), "--second-call needs --saved-judge");
}
