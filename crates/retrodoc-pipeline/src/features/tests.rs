use super::*;

use crate::brief::Evidence;
use crate::domains::{DomainCluster, SubDomainCluster};
use crate::testing::FakeLlm;

fn file_summary(path: &str) -> FileSummary {
    FileSummary {
        path: PathBuf::from(path),
        role_summary: format!("role of {path}"),
        commit_count: 1,
        author_count: 1,
    }
}

fn domain(slug: &str, paths: &[&str], subs: Vec<SubDomainCluster>) -> DomainCluster {
    DomainCluster {
        slug: slug.to_string(),
        name: slug.to_string(),
        description: "d".to_string(),
        paths: paths.iter().map(PathBuf::from).collect(),
        sub_domains: subs,
    }
}

#[test]
fn slugify_produces_file_name_safe_kebab_case() {
    assert_eq!(slugify("User Login / SSO!"), "user-login-sso");
    assert_eq!(slugify("../../etc"), "etc");
    assert_eq!(slugify("???"), "unnamed");
}

#[test]
fn unique_slug_suffixes_collisions() {
    let taken = ["login", "login-2"];
    assert_eq!(unique_slug("Login", taken.into_iter()), "login-3");
    assert_eq!(unique_slug("logout", taken.into_iter()), "logout");
}

#[tokio::test]
async fn build_features_grounds_features_on_known_files_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs"), file_summary("b.rs")],
        modules: Vec::new(),
    };
    let domains = DomainMap {
        domains: vec![
            domain("billing", &["a.rs", "b.rs"], Vec::new()),
            domain(UNCATEGORIZED_SLUG, &["c.rs"], Vec::new()),
        ],
    };
    // Second feature cites only a hallucinated file → dropped; the
    // first one's hallucinated file is filtered out.
    let provider = FakeLlm::answering(
        r#"```json
            {"features":[
              {"slug":"Invoice Creation","name":"Invoice creation","description":"d",
               "files":["a.rs","ghost.rs"]},
              {"slug":"phantom","name":"Phantom","description":"d","files":["ghost.rs"]}
            ]}
            ```"#,
    );

    let features = build_features(
        dir.path(),
        &domains,
        &repo_map,
        &ProductBrief::default(),
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();

    assert_eq!(features.len(), 1);
    assert_eq!(features[0].slug, "invoice-creation");
    assert_eq!(features[0].domain_slug, "billing");
    assert_eq!(features[0].sub_domain_slug, None);
    assert_eq!(features[0].source_paths, vec!["a.rs".to_string()]);

    let reloaded = load_features(dir.path()).unwrap();
    assert_eq!(reloaded.len(), 1);
}

#[tokio::test]
async fn build_features_covers_sub_domains_and_dedupes_slugs() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs"), file_summary("b.rs")],
        modules: Vec::new(),
    };
    let sub = SubDomainCluster {
        slug: "pdf".to_string(),
        name: "PDF".to_string(),
        description: "d".to_string(),
        paths: vec![PathBuf::from("b.rs")],
    };
    let domains = DomainMap {
        domains: vec![domain("billing", &["a.rs"], vec![sub])],
    };
    // Same answer for both units → the second "export" gets a suffix.
    let provider = FakeLlm::answering(
        r#"{"features":[{"slug":"export","name":"Export","description":"d",
            "files":["a.rs","b.rs"]}]}"#,
    );

    let features = build_features(
        dir.path(),
        &domains,
        &repo_map,
        &ProductBrief::default(),
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();

    assert_eq!(features.len(), 2);
    assert_eq!(features[0].slug, "export");
    assert_eq!(features[0].source_paths, vec!["a.rs".to_string()]);
    assert_eq!(features[1].slug, "export-2");
    assert_eq!(features[1].sub_domain_slug.as_deref(), Some("pdf"));
    assert_eq!(features[1].source_paths, vec!["b.rs".to_string()]);
}

#[tokio::test]
async fn build_features_skips_a_unit_with_an_unparseable_response() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs")],
        modules: Vec::new(),
    };
    let domains = DomainMap {
        domains: vec![domain("billing", &["a.rs"], Vec::new())],
    };
    let provider = FakeLlm::answering("I cannot do that");

    let features = build_features(
        dir.path(),
        &domains,
        &repo_map,
        &ProductBrief::default(),
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();

    assert!(features.is_empty());
}

#[tokio::test]
async fn rerun_reuses_unchanged_units_and_redoes_changed_ones() {
    let dir = tempfile::tempdir().unwrap();
    let domains = DomainMap {
        domains: vec![domain("billing", &["a.rs"], Vec::new())],
    };
    let map = |summary: &str| RepoMap {
        files: vec![FileSummary {
            role_summary: summary.to_string(),
            ..file_summary("a.rs")
        }],
        modules: Vec::new(),
    };
    let provider = FakeLlm::answering(
        r#"{"features":[{"slug":"f","name":"F","description":"d","files":["a.rs"]}]}"#,
    );
    let calls = || provider.calls();

    build_features(
        dir.path(),
        &domains,
        &map("v1"),
        &ProductBrief::default(),
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(calls(), 1);

    let again = build_features(
        dir.path(),
        &domains,
        &map("v1"),
        &ProductBrief::default(),
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(calls(), 1, "unchanged unit must not call the LLM");
    assert_eq!(again.len(), 1);

    build_features(
        dir.path(),
        &domains,
        &map("v2"),
        &ProductBrief::default(),
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(
        calls(),
        2,
        "a changed file summary must invalidate the unit"
    );
}

#[tokio::test]
async fn feature_slugs_are_unique_across_domains() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs"), file_summary("b.rs")],
        modules: Vec::new(),
    };
    let domains = DomainMap {
        domains: vec![
            domain("billing", &["a.rs"], Vec::new()),
            domain("shipping", &["b.rs"], Vec::new()),
        ],
    };
    // Both domains get the same answer, hence the same "export" slug.
    let provider = FakeLlm::answering(
        r#"{"features":[{"slug":"export","name":"Export","description":"d",
            "files":["a.rs","b.rs"]}]}"#,
    );

    let features = build_features(
        dir.path(),
        &domains,
        &repo_map,
        &ProductBrief::default(),
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();

    assert_eq!(features.len(), 2);
    assert_eq!(features[0].slug, "export");
    assert_eq!(features[1].slug, "export-2");
    assert_eq!(features[1].domain_slug, "shipping");
}

/// The answer of the call of rank `n`: one feature on `a.rs`, then `b.rs`.
fn feature_reply(n: usize) -> String {
    format!(
        r#"{{"features":[{{"slug":"f{n}","name":"F","description":"d","files":["{}"]}}]}}"#,
        if n == 0 { "a.rs" } else { "b.rs" }
    )
}

#[tokio::test]
async fn a_failed_run_keeps_the_units_done_and_the_rerun_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs"), file_summary("b.rs")],
        modules: Vec::new(),
    };
    let domains = DomainMap {
        domains: vec![
            domain("one", &["a.rs"], Vec::new()),
            domain("two", &["b.rs"], Vec::new()),
        ],
    };
    let flaky = FakeLlm::failing_after(1, feature_reply);
    assert!(build_features(
        dir.path(),
        &domains,
        &repo_map,
        &ProductBrief::default(),
        &Evidence::default(),
        &flaky
    )
    .await
    .is_err());
    assert_eq!(load_features(dir.path()).unwrap().len(), 1);

    // The first domain is cached: the second one is the call of rank 1.
    let healthy = FakeLlm::replying(|n, _| Ok(feature_reply(n + 1)));
    let features = build_features(
        dir.path(),
        &domains,
        &repo_map,
        &ProductBrief::default(),
        &Evidence::default(),
        &healthy,
    )
    .await
    .unwrap();
    assert_eq!(features.len(), 2);
    // Only the second domain was sent to the LLM again.
    assert_eq!(healthy.calls(), 1);
}

#[tokio::test]
async fn the_brief_heads_the_prompt_and_a_changed_brief_derives_again() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs")],
        modules: Vec::new(),
    };
    let domains = DomainMap {
        domains: vec![domain("billing", &["a.rs"], Vec::new())],
    };
    let provider = FakeLlm::answering(
        r#"{"features":[{"slug":"f","name":"F","description":"d","files":["a.rs"]}]}"#,
    );
    let brief = crate::testing::brief("Sells things.");

    build_features(
        dir.path(),
        &domains,
        &repo_map,
        &brief,
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();
    let prompt = provider.prompts().remove(0);
    assert!(
        prompt.starts_with("Product brief of the application"),
        "{prompt}"
    );
    assert!(prompt.find("Purpose: Sells things.").unwrap() < prompt.find("Domain:").unwrap());

    build_features(
        dir.path(),
        &domains,
        &repo_map,
        &brief,
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(provider.calls(), 1);

    let other = crate::testing::brief("Rents things.");
    build_features(
        dir.path(),
        &domains,
        &repo_map,
        &other,
        &Evidence::default(),
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(provider.calls(), 2);
}

fn evidence_of(signals: &[(retrodoc_ingest::signals::SignalKind, &str, &str)]) -> Evidence {
    let signals: Vec<_> = signals
        .iter()
        .map(|(kind, origin, text)| retrodoc_ingest::signals::Signal {
            kind: *kind,
            origin: (*origin).to_string(),
            text: (*text).to_string(),
        })
        .collect();
    Evidence::new(&signals)
}

#[tokio::test]
async fn the_evidence_closest_to_a_unit_is_in_its_prompt_and_moves_only_its_fingerprint() {
    use retrodoc_ingest::signals::SignalKind::{CommitSubject, DocSection};
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![
            file_summary("billing/invoice.rs"),
            file_summary("auth/login.rs"),
        ],
        modules: Vec::new(),
    };
    let domains = DomainMap {
        domains: vec![
            domain("billing", &["billing/invoice.rs"], Vec::new()),
            domain("auth", &["auth/login.rs"], Vec::new()),
        ],
    };
    let provider = FakeLlm::replying(|_, request| {
        let file = if request.messages[1].content.contains("billing/invoice.rs") {
            "billing/invoice.rs"
        } else {
            "auth/login.rs"
        };
        Ok(format!(
            r#"{{"features":[{{"slug":"f-{}","name":"F","description":"d","files":["{file}"]}}]}}"#,
            file.len()
        ))
    });
    let run = |evidence: Evidence| {
        let (dir, domains, repo_map, provider) = (&dir, &domains, &repo_map, &provider);
        async move {
            build_features(
                dir.path(),
                domains,
                repo_map,
                &ProductBrief::default(),
                &evidence,
                provider,
            )
            .await
            .unwrap()
        }
    };
    let invoices = (
        DocSection,
        "docs/billing.md#Invoices",
        "An invoice is sent to the customer.",
    );
    let before = evidence_of(&[invoices]);

    run(before.clone()).await;
    assert_eq!(provider.calls(), 2);
    let prompts = provider.prompts();
    let billing = prompts
        .iter()
        .find(|p| p.contains("billing/invoice.rs"))
        .unwrap();
    assert!(
        billing.contains("[docs/billing.md#Invoices]"),
        "{}",
        billing
    );
    let auth = prompts
        .iter()
        .find(|p| p.contains("auth/login.rs"))
        .unwrap();
    assert!(!auth.contains("Project evidence"), "{}", auth);

    // The same evidence: nothing is asked again.
    run(before).await;
    assert_eq!(provider.calls(), 2);

    // A commit close to billing only derives billing again.
    let after = evidence_of(&[
        invoices,
        (CommitSubject, "commit:abc", "fix: invoice rounding"),
    ]);
    run(after).await;
    assert_eq!(provider.calls(), 3);
}
