//! The human corrections guide the next run (`specs/human-corrections.md`): `generate` gives the
//! features and use cases passes the edits and the reviews of the sections each call writes.

mod support;

use std::path::Path;

use support::{generate, run, App, FakeLlm};

/// The shop of the tests: one domain `orders`, one feature `list-orders`, one use case
/// `list-orders/list-orders`.
fn shop(llm: &FakeLlm) -> tempfile::TempDir {
    support::repo(
        &[
            (
                "app/orders_controller.rb",
                "get '/orders' do\n  Order.all\nend\n",
            ),
            ("app/order.rb", "class Order\nend\n"),
        ],
        llm,
    )
}

fn fake_llm() -> FakeLlm {
    let app = App {
        stack: "Ruby",
        source_extensions: vec!["rb"],
        chunk_boundaries: Vec::new(),
        business: "app/",
        entry: "app/orders_controller.rb",
        model: "app/order.rb",
    };
    FakeLlm::start(move |call| app.answer(call))
}

fn succeeds(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Writes `.retrodoc/reviews.json` with one review of the use case.
fn review_the_use_case(dir: &Path, status: &str, comment: &str, at: &str) {
    let reviews = serde_json::json!({
        "version": 1,
        "reviews": [{
            "kind": "use_case", "id": "list-orders/list-orders", "status": status,
            "comment": comment, "author": "A", "reviewed_at": at, "fingerprint": "0f",
        }],
    });
    std::fs::write(
        dir.join(".retrodoc/reviews.json"),
        serde_json::to_string_pretty(&reviews).unwrap(),
    )
    .unwrap();
}

/// Writes `.retrodoc/edits.json` with `edits`.
fn write_edits(dir: &Path, edits: &serde_json::Value) {
    let edits = serde_json::json!({"version": 1, "edits": edits});
    std::fs::write(
        dir.join(".retrodoc/edits.json"),
        serde_json::to_string_pretty(&edits).unwrap(),
    )
    .unwrap();
}

/// The number of calls of each pass that writes a section: domains, features, use cases.
fn writing_calls(llm: &FakeLlm) -> [usize; 3] {
    ["domains", "features", "use-cases"].map(|pass| llm.calls_of(pass).len())
}

// spec: HCR-H1, HCR-2, HCR-4, HCR-6, HCR-8, HCR-10
#[test]
fn a_use_case_marked_wrong_asks_its_feature_again_once_with_the_comment() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));
    let [domains, features, use_cases] = writing_calls(&llm);

    review_the_use_case(
        dir.path(),
        "wrong",
        "The customer pays on delivery, not online.",
        "2026-11-24T10:00:00Z",
    );
    succeeds(&generate(dir.path()));

    assert_eq!(writing_calls(&llm), [domains, features, use_cases + 1]);
    let asked = llm.calls_of("use-cases").pop().unwrap().user;
    assert!(asked.contains("`list-orders`"), "{asked}");
    assert!(
        asked.contains("The customer pays on delivery, not online."),
        "{asked}"
    );

    // The same review, answered: nothing is asked again.
    let calls = llm.calls().len();
    succeeds(&generate(dir.path()));
    assert_eq!(llm.calls().len(), calls);

    // Another review of the same section asks again.
    review_the_use_case(
        dir.path(),
        "wrong",
        "Only the orders of the customer are listed.",
        "2026-11-25T10:00:00Z",
    );
    succeeds(&generate(dir.path()));
    assert_eq!(writing_calls(&llm), [domains, features, use_cases + 2]);
}

// spec: HCR-5, HCR-12
#[test]
fn an_edit_and_a_validated_review_ask_nothing_again() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));
    let calls = llm.calls().len();

    write_edits(
        dir.path(),
        &serde_json::json!([{
            "kind": "use_case", "id": "list-orders/list-orders", "name": "Browse the orders",
            "edited_at": "2026-11-24T10:00:00Z", "fingerprint": "0f",
        }]),
    );
    review_the_use_case(dir.path(), "validated", "", "2026-11-24T11:00:00Z");
    succeeds(&generate(dir.path()));

    assert_eq!(llm.calls().len(), calls);
}

// spec: HCR-1, HCR-3, HCR-6
#[test]
fn a_rewrite_is_given_the_edit_and_the_validated_text_to_keep() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));

    write_edits(
        dir.path(),
        &serde_json::json!([{
            "kind": "use_case", "id": "list-orders/list-orders",
            "narrative": "The customer browses the orders they placed.",
            "edited_at": "2026-11-24T10:00:00Z", "fingerprint": "0f",
        }]),
    );
    review_the_use_case(dir.path(), "validated", "", "2026-11-24T11:00:00Z");
    // `--force` removes the saved results: the corrections are read before.
    succeeds(&run(dir.path(), &["generate", "--force"]));

    let asked = llm.calls_of("use-cases").pop().unwrap().user;
    assert!(
        asked.contains("The customer browses the orders they placed."),
        "{asked}"
    );
    assert!(asked.contains("validated"), "{asked}");
    // The validated text, as saved: the generated name.
    assert!(asked.contains("List orders"), "{asked}");
}

// spec: HCR-9
#[test]
fn a_review_to_do_a_blank_field_and_an_unknown_section_reach_no_request() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));

    write_edits(
        dir.path(),
        &serde_json::json!([
            {
                "kind": "use_case", "id": "list-orders/gone", "name": "Gone",
                "edited_at": "2026-11-24T10:00:00Z", "fingerprint": "0f",
            },
            {
                "kind": "use_case", "id": "list-orders/list-orders", "name": "  ",
                "edited_at": "2026-11-24T10:00:00Z", "fingerprint": "0f",
            },
        ]),
    );
    review_the_use_case(
        dir.path(),
        "to_review",
        "Look again.",
        "2026-11-24T11:00:00Z",
    );
    succeeds(&run(dir.path(), &["generate", "--force"]));

    for call in llm.calls() {
        assert!(!call.user.contains("Human corrections"), "{}", call.user);
    }
}

// spec: HCR-11, EST-14
#[test]
fn generate_refuses_an_unreadable_file_of_edits_before_any_call() {
    for args in [&["generate"][..], &["generate", "--estimate"]] {
        let llm = fake_llm();
        let dir = shop(&llm);
        let retrodoc = dir.path().join(".retrodoc");
        std::fs::create_dir_all(&retrodoc).unwrap();
        std::fs::write(retrodoc.join("edits.json"), "{ not json").unwrap();

        let output = run(dir.path(), args);

        assert!(!output.status.success(), "{args:?}");
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(said.contains("edits.json"), "{args:?}: {said}");
        assert_eq!(llm.calls().len(), 0);
    }
}

/// The estimate block `generate` printed, and the line of the pass `pass` in it.
fn estimated<'a>(out: &'a str, pass: &str) -> (&'a str, &'a str) {
    let start = out.find("LLM estimate: ").expect("an estimate");
    let block = &out[start..];
    let line = block
        .lines()
        .find(|line| line.trim_start().starts_with(&format!("{pass} ")))
        .unwrap_or_else(|| panic!("{pass}: {block}"));
    (block.lines().next().unwrap(), line)
}

// spec: EST-H1, EST-12, EST-13
#[test]
fn the_estimate_counts_the_units_a_use_case_marked_wrong_asks_again_until_answered() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));
    review_the_use_case(
        dir.path(),
        "wrong",
        "The customer pays on delivery, not online.",
        "2026-11-24T10:00:00Z",
    );

    let output = generate(dir.path());
    succeeds(&output);

    let out = String::from_utf8_lossy(&output.stdout);
    let (total, use_cases) = estimated(&out, "use-cases");
    assert!(!total.starts_with("LLM estimate: 0 call(s)"), "{out}");
    // At least one call, whatever the measured table says of its cost.
    let asks = |line: &str| {
        line.contains("call(s)") && !line.contains(" 0 call(s)") && !line.contains(" 0-")
    };
    assert!(asks(use_cases), "{use_cases}");
    assert!(asks(estimated(&out, "rules").1), "{out}");
    for pass in ["brief", "domains", "features"] {
        let (_, line) = estimated(&out, pass);
        assert!(line.contains("reused from the cache"), "{line}");
    }

    // The review answered: nothing is asked again.
    let output = run(dir.path(), &["generate", "--estimate"]);
    succeeds(&output);
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(
        estimated(&out, "use-cases")
            .0
            .starts_with("LLM estimate: 0 call(s)"),
        "{out}"
    );
}

// spec: HCR-11
#[test]
fn an_unreadable_file_of_reviews_is_warned_of_and_asks_nothing_again() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));
    let calls = llm.calls().len();
    std::fs::write(dir.path().join(".retrodoc/reviews.json"), "{ not json").unwrap();

    let output = generate(dir.path());

    succeeds(&output);
    let warnings = support::warnings(&output);
    assert!(
        warnings.iter().any(|w| w.contains("reviews.json")),
        "{warnings:?}"
    );
    assert_eq!(llm.calls().len(), calls);
}

// spec: HCR-2, HCR-4, HCR-6
#[test]
fn a_feature_marked_wrong_asks_its_domain_again_once_with_the_comment() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));
    let [domains, features, use_cases] = writing_calls(&llm);
    let reviews = serde_json::json!({
        "version": 1,
        "reviews": [{
            "kind": "feature", "id": "list-orders", "status": "wrong",
            "comment": "Only the orders of the customer.", "author": "A",
            "reviewed_at": "2026-11-24T10:00:00Z", "fingerprint": "0f",
        }],
    });
    std::fs::write(
        dir.path().join(".retrodoc/reviews.json"),
        serde_json::to_string_pretty(&reviews).unwrap(),
    )
    .unwrap();

    succeeds(&generate(dir.path()));

    // The same answer: the use cases of the feature are reused.
    assert_eq!(writing_calls(&llm), [domains, features + 1, use_cases]);
    let asked = llm.calls_of("features").pop().unwrap().user;
    assert!(asked.contains("feature `list-orders`"), "{asked}");
    assert!(
        asked.contains("Only the orders of the customer."),
        "{asked}"
    );
    let calls = llm.calls().len();
    succeeds(&generate(dir.path()));
    assert_eq!(llm.calls().len(), calls);
}

// spec: HCR-2, HCR-4, HCR-6
#[test]
fn a_domain_marked_wrong_asks_the_domains_again_once_with_the_comment() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));
    let [domains, features, use_cases] = writing_calls(&llm);
    let reviews = serde_json::json!({
        "version": 1,
        "reviews": [{
            "kind": "domain", "id": "orders", "status": "wrong",
            "comment": "Name it after the purchases.", "author": "A",
            "reviewed_at": "2026-11-24T10:00:00Z", "fingerprint": "0f",
        }],
    });
    std::fs::write(
        dir.path().join(".retrodoc/reviews.json"),
        serde_json::to_string_pretty(&reviews).unwrap(),
    )
    .unwrap();

    succeeds(&generate(dir.path()));

    // The same answer: the features and the use cases are reused.
    assert_eq!(writing_calls(&llm), [domains + 1, features, use_cases]);
    let asked = llm.calls_of("domains").pop().unwrap().user;
    assert!(asked.contains("domain `orders`"), "{asked}");
    assert!(asked.contains("Name it after the purchases."), "{asked}");
    let calls = llm.calls().len();
    succeeds(&generate(dir.path()));
    assert_eq!(llm.calls().len(), calls);
}

// spec: HCR-1, HCR-6
#[test]
fn a_forced_run_gives_the_features_call_the_edit_of_a_feature() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));
    write_edits(
        dir.path(),
        &serde_json::json!([{
            "kind": "feature", "id": "list-orders", "name": "Browse the orders",
            "edited_at": "2026-11-24T10:00:00Z", "fingerprint": "0f",
        }]),
    );

    // `--force` removes `features.yaml`: the feature is matched with what was read before.
    succeeds(&run(dir.path(), &["generate", "--force"]));

    let asked = llm.calls_of("features").pop().unwrap().user;
    assert!(asked.contains("feature `list-orders`"), "{asked}");
    assert!(asked.contains("Browse the orders"), "{asked}");
}

// spec: HCR-3
#[test]
fn a_validated_use_case_is_given_with_its_narrative() {
    let llm = fake_llm();
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));
    review_the_use_case(dir.path(), "validated", "", "2026-11-24T11:00:00Z");

    succeeds(&run(dir.path(), &["generate", "--force"]));

    let asked = llm.calls_of("use-cases").pop().unwrap().user;
    assert!(asked.contains("validated"), "{asked}");
    assert!(asked.contains("The customer lists the orders."), "{asked}");
}

// spec: RER-H1, RER-13
#[test]
fn a_use_case_written_again_under_a_new_slug_keeps_its_saved_one() {
    let app = App {
        stack: "Ruby",
        source_extensions: vec!["rb"],
        chunk_boundaries: Vec::new(),
        business: "app/",
        entry: "app/orders_controller.rb",
        model: "app/order.rb",
    };
    // Asked again, the model renames the use case of the same entry point.
    let llm = FakeLlm::start(move |call| {
        let answer = app.answer(call);
        if call.pass == "use-cases" && call.user.contains("Human corrections") {
            answer.replacen("\"list-orders\"", "\"browse-orders\"", 1)
        } else {
            answer
        }
    });
    let dir = shop(&llm);
    succeeds(&generate(dir.path()));
    review_the_use_case(
        dir.path(),
        "wrong",
        "Only the orders of the customer are listed.",
        "2026-11-24T10:00:00Z",
    );

    succeeds(&generate(dir.path()));

    let use_cases =
        std::fs::read_to_string(dir.path().join(".retrodoc/cache/use-cases.yaml")).unwrap();
    assert!(use_cases.contains("slug: list-orders"), "{use_cases}");
    assert!(!use_cases.contains("browse-orders"), "{use_cases}");
}
