use super::*;
use std::collections::HashMap;

use retrodoc_ingest::{FileEntry, FileKind, IngestResult};

use crate::repo_map::{build_repo_map, FileSummary, ModuleSummary, RepoMapOptions};
use crate::testing::FakeLlm;

/// A provider that panics if called, to assert a fast path never reaches
/// the LLM.
fn never_called() -> FakeLlm {
    FakeLlm::replying(|_, _| panic!("LLM should not have been called"))
}

fn file_summary(path: &str) -> FileSummary {
    FileSummary {
        path: PathBuf::from(path),
        role_summary: format!("role of {path}"),
        commit_count: 1,
        author_count: 1,
    }
}

#[test]
fn enforce_coverage_dedupes_overlap_drops_unknown_and_buckets_uncovered() {
    let mut map = DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".to_string(),
            name: "Billing".to_string(),
            description: "d".to_string(),
            paths: vec![PathBuf::from("a.rs"), PathBuf::from("ghost.rs")],
            sub_domains: vec![SubDomainCluster {
                slug: "invoices".to_string(),
                name: "Invoices".to_string(),
                description: "d".to_string(),
                // "a.rs" is a duplicate (already claimed above).
                paths: vec![PathBuf::from("a.rs")],
            }],
        }],
    };
    let all_paths = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];

    let report = enforce_coverage(&mut map, &all_paths);

    assert_eq!(report.overlapping, vec![PathBuf::from("a.rs")]);
    assert_eq!(report.unknown, vec![PathBuf::from("ghost.rs")]);
    assert_eq!(report.uncovered, vec![PathBuf::from("b.rs")]);
    assert!(!report.is_clean());

    // "b.rs" landed in the synthetic uncategorized domain.
    let uncategorized = map
        .domains
        .iter()
        .find(|d| d.slug == UNCATEGORIZED_SLUG)
        .unwrap();
    assert_eq!(uncategorized.paths, vec![PathBuf::from("b.rs")]);

    // "a.rs" only appears once in the repaired map.
    let billing = &map.domains[0];
    assert_eq!(billing.paths, vec![PathBuf::from("a.rs")]);
    assert_eq!(billing.sub_domains[0].paths, Vec::<PathBuf>::new());
}

#[test]
fn clean_map_reports_nothing() {
    let mut map = DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".to_string(),
            name: "Billing".to_string(),
            description: "d".to_string(),
            paths: vec![PathBuf::from("a.rs")],
            sub_domains: vec![],
        }],
    };
    let all_paths = vec![PathBuf::from("a.rs")];

    let report = enforce_coverage(&mut map, &all_paths);

    assert!(report.is_clean());
    assert!(!map.domains.iter().any(|d| d.slug == UNCATEGORIZED_SLUG));
}

#[tokio::test]
async fn build_domains_skips_the_llm_when_there_are_no_files() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap::default();

    let (map, report) = build_domains(
        dir.path(),
        &repo_map,
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &never_called(),
    )
    .await
    .unwrap();

    assert!(map.domains.is_empty());
    assert!(report.is_clean());
}

#[tokio::test]
async fn build_domains_parses_response_and_persists_the_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs"), file_summary("b.rs")],
        modules: vec![ModuleSummary {
            path: PathBuf::new(),
            role_summary: "root".to_string(),
            file_count: 2,
        }],
    };
    let provider = FakeLlm::answering(
        r#"```json
        {"domains":[{"slug":"billing","name":"Billing","description":"Handles invoices.",
        "paths":[""],"sub_domains":[]}]}
        ```"#,
    );

    let (map, report) = build_domains(
        dir.path(),
        &repo_map,
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &provider,
    )
    .await
    .unwrap();

    assert!(report.is_clean());
    assert_eq!(map.domains.len(), 1);
    assert_eq!(map.domains[0].slug, "billing");
    assert_eq!(
        map.domains[0].paths,
        vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")]
    );

    let reloaded = DomainMap::load(dir.path()).unwrap();
    assert_eq!(reloaded.domains.len(), 1);
    assert_eq!(reloaded.domains[0].slug, "billing");
}

#[tokio::test]
async fn build_domains_reuses_the_saved_clustering_when_the_input_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs")],
        modules: vec![],
    };
    let provider = FakeLlm::answering(
        r#"{"domains":[{"slug":"billing","name":"Billing","description":"d",
        "paths":["a.rs"],"sub_domains":[]}]}"#,
    );
    build_domains(
        dir.path(),
        &repo_map,
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &provider,
    )
    .await
    .unwrap();

    // Same input: the LLM must not be called again.
    let (map, _) = build_domains(
        dir.path(),
        &repo_map,
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &never_called(),
    )
    .await
    .unwrap();
    assert_eq!(map.domains[0].slug, "billing");

    // Changed input: it is.
    let changed = RepoMap {
        files: vec![file_summary("a.rs"), file_summary("b.rs")],
        modules: vec![],
    };
    let (map, _) = build_domains(
        dir.path(),
        &changed,
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(map.domains[0].paths.len(), 1); // canned answer, b.rs uncovered
    assert!(map.domains.iter().any(|d| d.slug == UNCATEGORIZED_SLUG));
}

#[tokio::test]
async fn build_domains_works_end_to_end_with_a_real_repo_map() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();

    let ingest = IngestResult {
        files: vec![FileEntry {
            path: PathBuf::from("a.rs"),
            kind: FileKind::Source,
            size_bytes: 9,
        }],
        history_by_path: HashMap::new(),
        existing_docs: Vec::new(),
        commits: Vec::new(),
    };

    let repo_map_provider = FakeLlm::answering("a summary");
    let repo_map = build_repo_map(
        dir.path(),
        &ingest,
        &repo_map_provider,
        RepoMapOptions::default(),
    )
    .await
    .unwrap();

    let clustering_provider = FakeLlm::answering(
        r#"{"domains":[{"slug":"core","name":"Core","description":"d",
        "paths":[""],"sub_domains":[]}]}"#,
    );
    let (map, report) = build_domains(
        dir.path(),
        &repo_map,
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &clustering_provider,
    )
    .await
    .unwrap();

    assert!(report.is_clean());
    assert_eq!(map.domains[0].paths, vec![PathBuf::from("a.rs")]);
}

fn domain_with_dirs(slug: &str, dirs: &[&str]) -> DomainCluster {
    DomainCluster {
        slug: slug.to_string(),
        name: slug.to_string(),
        description: "d".to_string(),
        paths: dirs.iter().map(PathBuf::from).collect(),
        sub_domains: Vec::new(),
    }
}

#[test]
fn expand_to_files_resolves_longest_prefix_match() {
    let map = DomainMap {
        domains: vec![
            domain_with_dirs("backend", &["src"]),
            domain_with_dirs("frontend", &["src/ui"]),
        ],
    };
    let files = vec![
        file_summary("src/main.rs"),
        file_summary("src/ui/button.rs"),
    ];

    let expanded = expand_to_files(map, &files);

    assert_eq!(
        expanded.domains[0].paths,
        vec![PathBuf::from("src/main.rs")]
    );
    assert_eq!(
        expanded.domains[1].paths,
        vec![PathBuf::from("src/ui/button.rs")]
    );
}

#[test]
fn expand_to_files_uses_root_as_fallback_only_when_nothing_more_specific_wins() {
    let map = DomainMap {
        domains: vec![
            domain_with_dirs("core", &[""]),
            domain_with_dirs("docs", &["docs"]),
        ],
    };
    let files = vec![file_summary("lib.rs"), file_summary("docs/helper.rs")];

    let expanded = expand_to_files(map, &files);

    assert_eq!(expanded.domains[0].paths, vec![PathBuf::from("lib.rs")]);
    assert_eq!(
        expanded.domains[1].paths,
        vec![PathBuf::from("docs/helper.rs")]
    );
}

#[test]
fn expand_to_files_tolerates_a_directory_matching_no_real_file() {
    let map = DomainMap {
        domains: vec![
            domain_with_dirs("ghost-hunters", &["ghost/dir"]),
            domain_with_dirs("real", &["src"]),
        ],
    };
    let files = vec![file_summary("src/main.rs")];

    let expanded = expand_to_files(map, &files);

    assert_eq!(expanded.domains[0].paths, Vec::<PathBuf>::new());
    assert_eq!(
        expanded.domains[1].paths,
        vec![PathBuf::from("src/main.rs")]
    );
}

#[test]
fn expand_to_files_resolves_duplicate_directory_claim_to_the_first_domain() {
    let map = DomainMap {
        domains: vec![
            domain_with_dirs("first", &["src"]),
            domain_with_dirs("second", &["src"]),
        ],
    };
    let files = vec![file_summary("src/main.rs")];

    let expanded = expand_to_files(map, &files);

    assert_eq!(
        expanded.domains[0].paths,
        vec![PathBuf::from("src/main.rs")]
    );
    assert_eq!(expanded.domains[1].paths, Vec::<PathBuf>::new());
}

#[test]
fn clustering_prompt_renders_root_module_as_empty_string_not_dot() {
    let repo_map = RepoMap {
        files: vec![],
        modules: vec![
            ModuleSummary {
                path: PathBuf::new(),
                role_summary: "root".to_string(),
                file_count: 3,
            },
            ModuleSummary {
                path: PathBuf::from("src"),
                role_summary: "source".to_string(),
                file_count: 2,
            },
        ],
    };

    let prompt = clustering_prompt(
        &repo_map,
        &[],
        &Surface::default(),
        &ProductBrief::default(),
    );

    assert!(prompt.contains("- \"\" "));
    assert!(!prompt
        .lines()
        .any(|l| l.trim() == "- ." || l.trim().starts_with("- .:")));
}

#[test]
fn flags_domains_named_after_a_technical_layer() {
    let cluster = |slug: &str| DomainCluster {
        slug: slug.to_string(),
        name: slug.to_string(),
        description: String::new(),
        paths: Vec::new(),
        sub_domains: Vec::new(),
    };
    let map = DomainMap {
        domains: vec![
            cluster("presentation-layer"),
            cluster("contract-signing"),
            cluster("services"),
            cluster("uncategorized"),
        ],
    };
    assert_eq!(
        layer_named_domains(&map),
        vec!["presentation-layer", "services"]
    );
}

#[tokio::test]
async fn the_surface_reaches_the_prompt_and_invalidates_the_saved_clustering() {
    use crate::entry_points::EntryPoints;
    use crate::glossary::{Entity, Glossary, ModelFile};

    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs")],
        modules: vec![ModuleSummary {
            path: PathBuf::new(),
            role_summary: "root".to_string(),
            file_count: 1,
        }],
    };
    let glossary = Glossary {
        models: std::collections::BTreeMap::from([(
            PathBuf::from("a.rs"),
            ModelFile {
                content_hash: String::new(),
                entities: vec![Entity {
                    name: "Contract".to_string(),
                    description: "An agreement".to_string(),
                    attributes: Vec::new(),
                    associations: Vec::new(),
                }],
            },
        )]),
        tests: Vec::new(),
    };
    let surface = Surface::new(&glossary, &EntryPoints::default());
    let provider = FakeLlm::answering(
        r#"{"domains":[{"slug":"contracts","name":"Contracts","description":"d","paths":[""]}]}"#,
    );

    // Without a surface: the plain prompt. With one: entities + naming rule.
    build_domains(
        dir.path(),
        &repo_map,
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &provider,
    )
    .await
    .unwrap();
    build_domains(
        dir.path(),
        &repo_map,
        &[],
        &surface,
        &ProductBrief::default(),
        &provider,
    )
    .await
    .unwrap();

    let prompts = provider.prompt_pairs();
    assert_eq!(prompts.len(), 2, "a new surface must recompute the domains");
    assert!(!prompts[0].0.contains("technical layer"));
    assert!(!prompts[0].1.contains("Business entities"));
    assert!(prompts[1]
        .0
        .contains("Never name a domain after a technical layer"));
    assert!(prompts[1].1.contains("- Contract (in \"\"): An agreement"));
}

/// The clustering the smoke test on a small Ruby repo got: three
/// sub-domains on single files, no folder.
const SINGLE_FILES_CLUSTERING: &str = r#"{"domains":[
    {"slug":"delivery","name":"Delivery","description":"Delivers orders.","paths":[],
     "sub_domains":[
        {"slug":"orders","name":"Orders","description":"Orders.","paths":["lib/order.rb"]},
        {"slug":"customers","name":"Customers","description":"Customers.","paths":["lib/customer.rb"]}]}]}"#;

fn lib_repo_map() -> RepoMap {
    RepoMap {
        files: ["order", "customer", "rider", "router"]
            .iter()
            .map(|name| file_summary(&format!("lib/{name}.rb")))
            .collect(),
        modules: vec![ModuleSummary {
            path: PathBuf::from("lib"),
            role_summary: "the application".to_string(),
            file_count: 4,
        }],
    }
}

fn paths_of(domain: &DomainCluster) -> Vec<&str> {
    domain.paths.iter().map(|p| p.to_str().unwrap()).collect()
}

#[tokio::test]
async fn files_left_unassigned_are_placed_by_a_second_call() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeLlm::sequence(&[
        SINGLE_FILES_CLUSTERING,
        r#"{"assignments":[
            {"path":"lib/rider.rb","domain":"delivery","sub_domain":null},
            {"path":"lib/router.rb","domain":"delivery","sub_domain":"orders"}]}"#,
    ]);

    let (map, report) = build_domains(
        dir.path(),
        &lib_repo_map(),
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &provider,
    )
    .await
    .unwrap();

    assert_eq!(provider.calls(), 2);
    assert!(report.is_clean(), "{report:?}");
    assert!(!map.domains.iter().any(|d| d.slug == UNCATEGORIZED_SLUG));
    let delivery = &map.domains[0];
    assert_eq!(paths_of(delivery), vec!["lib/rider.rb"]);
    assert_eq!(
        delivery.sub_domains[0].paths,
        vec![
            PathBuf::from("lib/order.rb"),
            PathBuf::from("lib/router.rb")
        ]
    );
    let prompt = &provider.prompts()[1];
    assert!(prompt.contains("lib/rider.rb: role of lib/rider.rb"));
    assert!(!prompt.contains("lib/order.rb"), "only the unplaced files");
}

#[tokio::test]
async fn invalid_placements_are_ignored_and_the_rest_is_bucketed() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeLlm::sequence(&[
        SINGLE_FILES_CLUSTERING,
        r#"{"assignments":[
            {"path":"lib/ghost.rb","domain":"delivery"},
            {"path":"lib/order.rb","domain":"delivery"},
            {"path":"lib/rider.rb","domain":"no-such-domain"},
            {"path":"lib/router.rb","domain":"delivery","sub_domain":"no-such-sub"}]}"#,
    ]);

    let (map, report) = build_domains(
        dir.path(),
        &lib_repo_map(),
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &provider,
    )
    .await
    .unwrap();

    // A bad sub-domain falls back to the domain; an unknown domain, an
    // invented file and an already placed file change nothing.
    assert_eq!(paths_of(&map.domains[0]), vec!["lib/router.rb"]);
    assert_eq!(
        map.domains[0].sub_domains[0].paths,
        vec![PathBuf::from("lib/order.rb")]
    );
    assert_eq!(report.uncovered, vec![PathBuf::from("lib/rider.rb")]);
    let uncategorized = map
        .domains
        .iter()
        .find(|d| d.slug == UNCATEGORIZED_SLUG)
        .unwrap();
    assert_eq!(paths_of(uncategorized), vec!["lib/rider.rb"]);
}

#[tokio::test]
async fn an_unusable_placement_answer_leaves_the_files_uncategorized() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeLlm::sequence(&[SINGLE_FILES_CLUSTERING, "no idea"]);

    let (map, report) = build_domains(
        dir.path(),
        &lib_repo_map(),
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &provider,
    )
    .await
    .unwrap();

    assert_eq!(provider.calls(), 3, "clustering + placement and its retry");
    assert_eq!(report.uncovered.len(), 2);
    assert!(map.domains.iter().any(|d| d.slug == UNCATEGORIZED_SLUG));
}

#[tokio::test]
async fn no_placement_call_when_the_clustering_covers_every_file() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeLlm::answering(
        r#"{"domains":[{"slug":"delivery","name":"Delivery","description":"d",
            "paths":["lib"],"sub_domains":[]}]}"#,
    );

    let (_, report) = build_domains(
        dir.path(),
        &lib_repo_map(),
        &[],
        &Surface::default(),
        &ProductBrief::default(),
        &provider,
    )
    .await
    .unwrap();

    assert!(report.is_clean());
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn the_brief_heads_the_clustering_prompt_and_a_changed_brief_clusters_again() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs")],
        modules: vec![],
    };
    let provider = FakeLlm::answering(
        r#"{"domains":[{"slug":"billing","name":"Billing","description":"d",
        "paths":["a.rs"],"sub_domains":[]}]}"#,
    );
    let brief = crate::testing::brief("Sells things.");

    build_domains(
        dir.path(),
        &repo_map,
        &[],
        &Surface::default(),
        &brief,
        &provider,
    )
    .await
    .unwrap();
    let prompt = provider.prompts().remove(0);
    assert!(
        prompt.starts_with("Product brief of the application"),
        "{prompt}"
    );

    build_domains(
        dir.path(),
        &repo_map,
        &[],
        &Surface::default(),
        &brief,
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(provider.calls(), 1);

    let other = crate::testing::brief("Rents things.");
    build_domains(
        dir.path(),
        &repo_map,
        &[],
        &Surface::default(),
        &other,
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(provider.calls(), 2);
}

#[tokio::test]
async fn another_model_clusters_the_same_input_again() {
    let dir = tempfile::tempdir().unwrap();
    let repo_map = RepoMap {
        files: vec![file_summary("a.rs")],
        modules: vec![],
    };
    let answer = r#"{"domains":[{"slug":"billing","name":"Billing","description":"d",
        "paths":["a.rs"],"sub_domains":[]}]}"#;
    let run = |provider: FakeLlm| {
        let (root, repo_map) = (dir.path().to_path_buf(), repo_map.clone());
        async move {
            build_domains(
                &root,
                &repo_map,
                &[],
                &Surface::default(),
                &ProductBrief::default(),
                &provider,
            )
            .await
            .unwrap();
            provider.calls()
        }
    };
    assert_eq!(run(FakeLlm::answering(answer).with_model("local")).await, 1);
    assert_eq!(run(FakeLlm::answering(answer).with_model("local")).await, 0);
    assert_eq!(
        run(FakeLlm::answering(answer).with_model("strong")).await,
        1
    );
}
