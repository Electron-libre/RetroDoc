use super::*;

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

    let features = build_features(dir.path(), &domains, &repo_map, &provider)
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

    let features = build_features(dir.path(), &domains, &repo_map, &provider)
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

    let features = build_features(dir.path(), &domains, &repo_map, &provider)
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

    build_features(dir.path(), &domains, &map("v1"), &provider)
        .await
        .unwrap();
    assert_eq!(calls(), 1);

    let again = build_features(dir.path(), &domains, &map("v1"), &provider)
        .await
        .unwrap();
    assert_eq!(calls(), 1, "unchanged unit must not call the LLM");
    assert_eq!(again.len(), 1);

    build_features(dir.path(), &domains, &map("v2"), &provider)
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

    let features = build_features(dir.path(), &domains, &repo_map, &provider)
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
    assert!(build_features(dir.path(), &domains, &repo_map, &flaky)
        .await
        .is_err());
    assert_eq!(load_features(dir.path()).unwrap().len(), 1);

    // The first domain is cached: the second one is the call of rank 1.
    let healthy = FakeLlm::replying(|n, _| Ok(feature_reply(n + 1)));
    let features = build_features(dir.path(), &domains, &repo_map, &healthy)
        .await
        .unwrap();
    assert_eq!(features.len(), 2);
    // Only the second domain was sent to the LLM again.
    assert_eq!(healthy.calls(), 1);
}
