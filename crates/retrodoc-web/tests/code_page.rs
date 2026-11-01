//! The page of a cited file and the links to it, asked of the router without
//! a socket over a git repository and the artifacts a run leaves in
//! `.retrodoc/cache/`.

#![cfg(unix)]

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{
    Actor, ActorKind, BusinessRule, Feature, RuleEvidence, RuleKind, SourceRef, Step, UseCase,
};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::{DomainCluster, DomainMap, SourceHashes};
use retrodoc_render::DocLinks;
use tower::ServiceExt;

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "Ada Lovelace")
        .env("GIT_AUTHOR_EMAIL", "ada@example.com")
        .env("GIT_COMMITTER_NAME", "Ada Lovelace")
        .env("GIT_COMMITTER_EMAIL", "ada@example.com")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn commit(root: &Path, name: &str, content: &str, message: &str) {
    std::fs::write(root.join(name), content).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", message]);
}

fn source(path: &str, start_line: Option<u32>, end_line: Option<u32>) -> SourceRef {
    SourceRef {
        path: path.into(),
        start_line,
        end_line,
    }
}

/// 500 lines of `invoice line <n>`, line 5 of 500 characters.
fn invoice_code() -> String {
    let mut code = String::new();
    for n in 1..=500 {
        if n == 5 {
            let _ = writeln!(code, "{}", "x".repeat(500));
        } else {
            let _ = writeln!(code, "invoice line {n}");
        }
    }
    code
}

/// A git repository `repo` (beside a `secret.txt` outside it) where the
/// feature `pay-invoice` of `billing` cites `app/invoice.rb`, `app/card.rb`,
/// `app/link.rb` (a link to the secret) and `app/gone.rb` (deleted); its use
/// case `pay-by-card` cites `app/invoice.rb:40-42` and `app/card.rb`; its rule
/// cites `app/invoice.rb:40-42`. `app/other.rb` is cited by nothing. The hashes
/// of the run are recorded, then `app/card.rb` is changed.
fn generated_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let outer = tempfile::tempdir().unwrap();
    std::fs::write(outer.path().join("secret.txt"), "top secret").unwrap();
    let root = outer.path().join("repo");
    std::fs::create_dir_all(root.join("app")).unwrap();
    git(&root, &["init", "-q"]);
    commit(&root, "app/invoice.rb", "first\n", "add invoices");
    commit(&root, "app/card.rb", "card\n", "add cards");
    commit(&root, "app/other.rb", "other secret code\n", "add other");
    commit(
        &root,
        "app/invoice.rb",
        &invoice_code(),
        "charge late invoices",
    );
    std::os::unix::fs::symlink(outer.path().join("secret.txt"), root.join("app/link.rb")).unwrap();

    let mut hashes = SourceHashes::default();
    for name in ["app/invoice.rb", "app/card.rb", "app/other.rb"] {
        let content = std::fs::read_to_string(root.join(name)).unwrap();
        hashes.put(name, &hash_content(&content));
    }
    hashes.put("app/gone.rb", &hash_content("gone"));
    hashes.save(&root).unwrap();
    std::fs::write(root.join("app/card.rb"), "card changed\n").unwrap();

    retrodoc_pipeline::save_features(
        &root,
        &[Feature {
            back_office: false,
            slug: "pay-invoice".into(),
            domain_slug: "billing".into(),
            name: "Pay an invoice".into(),
            description: String::new(),
            source_paths: [
                "app/invoice.rb",
                "app/card.rb",
                "app/link.rb",
                "app/gone.rb",
            ]
            .map(String::from)
            .to_vec(),
            entry_points: Vec::new(),
            confidence: None,
        }],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(
        &root,
        &[UseCase {
            slug: "pay-by-card".into(),
            feature_slug: "pay-invoice".into(),
            name: "Pay by card".into(),
            description: String::new(),
            steps: vec![Step {
                order: 1,
                description: "Charges the card".into(),
                actor: Actor {
                    name: "Gateway".into(),
                    kind: ActorKind::System,
                },
                action: String::new(),
                source_refs: vec![
                    source("app/invoice.rb", Some(40), Some(42)),
                    source("app/card.rb", None, None),
                ],
            }],
            entry_points: Vec::new(),
            primary_actor: None,
            narrative: None,
            business_language: None,
            diagram_mermaid: None,
            confidence: None,
            narrative_check: None,
        }],
    )
    .unwrap();
    retrodoc_pipeline::save_rules(
        &root,
        &[BusinessRule {
            slug: "pay-invoice-1".into(),
            statement: "A late invoice is charged".into(),
            kind: RuleKind::Limit,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: vec![RuleEvidence::Code {
                source: source("app/invoice.rb", Some(40), Some(42)),
            }],
            confidence: None,
        }],
    )
    .unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            description: String::new(),
            ..DomainCluster::default()
        }],
    }
    .save(&root)
    .unwrap();
    (outer, root)
}

async fn get(root: &Path, path: &str) -> (StatusCode, String) {
    let docs = GeneratedDocs::load(root, &[]);
    let response = retrodoc_web::router(docs, DocLinks::default(), Vec::new())
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn page(root: &Path, path: &str) -> String {
    let (status, body) = get(root, path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

/// The numbers of the lines the page shows, in order.
fn shown_lines(page: &str) -> Vec<usize> {
    page.split("<tr id=\"L")
        .skip(1)
        .map(|rest| rest[..rest.find('"').unwrap()].parse().unwrap())
        .collect()
}

/// The numbers of the lines the page marks as cited.
fn cited_lines(page: &str) -> Vec<usize> {
    page.split("<tr id=\"L")
        .skip(1)
        .filter(|rest| rest[..rest.find('>').unwrap()].contains("class=\"cited\""))
        .map(|rest| rest[..rest.find('"').unwrap()].parse().unwrap())
        .collect()
}

// spec: WEB-26
#[tokio::test]
async fn a_cited_file_shows_its_first_200_lines_numbered_and_long_lines_cut() {
    let (_dir, root) = generated_repo();
    let page = page(&root, "/code/app/invoice.rb").await;
    assert_eq!(shown_lines(&page), (1..=200).collect::<Vec<_>>());
    assert!(page.contains("invoice line 200"), "{page}");
    assert!(!page.contains("invoice line 201"), "{page}");
    assert!(page.contains("lines 1-200 of 500"), "{page}");
    assert!(page.contains(&"x".repeat(400)));
    assert!(!page.contains(&"x".repeat(401)));
    assert!(page.contains("line cut, 500 characters"), "{page}");
    assert_eq!(cited_lines(&page), Vec::<usize>::new());
}

// spec: WEB-27
#[tokio::test]
async fn the_cited_lines_are_marked_with_ten_lines_around_them() {
    let (_dir, root) = generated_repo();
    let page = self::page(&root, "/code/app/invoice.rb?lines=40-42").await;
    assert_eq!(shown_lines(&page), (30..=52).collect::<Vec<_>>());
    assert_eq!(cited_lines(&page), vec![40, 41, 42]);

    let page = self::page(&root, "/code/app/invoice.rb?lines=40").await;
    assert_eq!(shown_lines(&page), (30..=50).collect::<Vec<_>>());
    assert_eq!(cited_lines(&page), vec![40]);

    let page = self::page(&root, "/code/app/invoice.rb?lines=1-3").await;
    assert_eq!(shown_lines(&page), (1..=13).collect::<Vec<_>>());
    assert_eq!(cited_lines(&page), vec![1, 2, 3]);
}

// spec: WEB-27
#[tokio::test]
async fn lines_that_are_no_range_show_the_file_from_the_top_with_nothing_marked() {
    let (_dir, root) = generated_repo();
    for query in ["lines=abc", "lines=9-3", "lines=", "other=1"] {
        let page = page(&root, &format!("/code/app/invoice.rb?{query}")).await;
        assert_eq!(shown_lines(&page).first(), Some(&1), "{query}");
        assert_eq!(shown_lines(&page).len(), 200, "{query}");
        assert!(cited_lines(&page).is_empty(), "{query}");
    }
}

// spec: WEB-28
#[tokio::test]
async fn the_page_of_a_file_lists_its_commits_without_their_author() {
    let (_dir, root) = generated_repo();
    let page = page(&root, "/code/app/invoice.rb").await;
    let newest = page.find("charge late invoices").expect(&page);
    let oldest = page.find("add invoices").expect(&page);
    assert!(newest < oldest, "{page}");
    assert!(!page.contains("add cards"), "{page}");
    assert!(
        !page.contains("Ada") && !page.contains("example.com"),
        "{page}"
    );
}

// spec: WEB-28
#[tokio::test]
async fn a_repository_with_no_history_still_shows_the_lines() {
    let (_dir, root) = generated_repo();
    std::fs::remove_dir_all(root.join(".git")).unwrap();
    let page = page(&root, "/code/app/invoice.rb").await;
    assert!(page.contains("history is not available"), "{page}");
    assert!(page.contains("invoice line 2"), "{page}");
}

// spec: WEB-29
#[tokio::test]
async fn the_page_of_a_file_lists_what_cites_it() {
    let (_dir, root) = generated_repo();
    let page = page(&root, "/code/app/invoice.rb").await;
    assert!(
        page.contains("<a href=\"/domains/billing/pay-invoice\">Pay an invoice</a>"),
        "{page}"
    );
    assert!(
        page.contains("<a href=\"/domains/billing/pay-invoice/pay-by-card\">Pay by card</a>"),
        "{page}"
    );
    let card = self::page(&root, "/code/app/card.rb").await;
    assert!(!card.contains("A late invoice is charged"), "{card}");
}

// spec: WEB-61
#[tokio::test]
async fn a_rule_that_cites_the_file_links_to_its_item_on_the_page_of_its_feature() {
    let (_dir, root) = generated_repo();
    let page = page(&root, "/code/app/invoice.rb").await;
    assert!(
        page.contains(
            "<a href=\"/domains/billing/pay-invoice#rule-pay-invoice-1\">A late invoice is charged</a>"
        ),
        "{page}"
    );
}

// spec: WEB-30
#[tokio::test]
async fn a_file_changed_since_the_run_is_flagged_stale() {
    let (_dir, root) = generated_repo();
    let card = page(&root, "/code/app/card.rb").await;
    assert!(card.contains("class=\"badge stale\""), "{card}");
    let invoice = page(&root, "/code/app/invoice.rb").await;
    assert!(!invoice.contains("badge stale"), "{invoice}");
    std::fs::remove_file(root.join(".retrodoc/cache/source-hashes.json")).unwrap();
    let card = page(&root, "/code/app/card.rb").await;
    assert!(!card.contains("badge stale"), "{card}");
}

// spec: WEB-31
#[tokio::test]
async fn a_path_no_doc_cites_or_outside_the_repository_is_not_found() {
    let (_dir, root) = generated_repo();
    for path in [
        "/code/app/other.rb",
        "/code/../secret.txt",
        "/code/%2E%2E/secret.txt",
        "/code/app/%2E%2E/%2E%2E/secret.txt",
        "/code/",
    ] {
        let (status, page) = get(&root, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(page.contains("<h1>Not found</h1>"), "{path}: {page}");
        assert!(
            !page.contains("top secret") && !page.contains("other secret code"),
            "{path}"
        );
    }
}

// spec: WEB-32
#[tokio::test]
async fn a_cited_file_that_cannot_be_shown_says_why_and_shows_no_line() {
    let (_dir, root) = generated_repo();
    let link = page(&root, "/code/app/link.rb").await;
    assert!(link.contains("symbolic link"), "{link}");
    assert!(!link.contains("top secret"), "{link}");
    assert_eq!(shown_lines(&link), Vec::<usize>::new());
    let gone = page(&root, "/code/app/gone.rb").await;
    assert!(gone.contains("no longer exists"), "{gone}");
    assert!(gone.contains("Pay an invoice"), "{gone}");
    for lines in ["900-901", "900-18446744073709551615"] {
        let past = page(&root, &format!("/code/app/invoice.rb?lines={lines}")).await;
        assert!(past.contains("past the end"), "{past}");
        assert_eq!(shown_lines(&past), Vec::<usize>::new());
    }
}

// spec: WEB-33
#[tokio::test]
async fn cited_lines_source_files_and_code_evidence_link_to_the_code() {
    let (_dir, root) = generated_repo();
    let feature = page(&root, "/domains/billing/pay-invoice").await;
    assert!(
        feature.contains("<a href=\"/code/app/card.rb\"><code>app/card.rb</code></a>"),
        "{feature}"
    );
    assert!(
        feature.contains(
            "<a href=\"/code/app/invoice.rb?lines=40-42#L40\"><code>app/invoice.rb:40-42</code></a>"
        ),
        "{feature}"
    );
    let use_case = page(&root, "/domains/billing/pay-invoice/pay-by-card").await;
    assert!(
        use_case.contains(
            "<a href=\"/code/app/invoice.rb?lines=40-42#L40\"><code>app/invoice.rb:40-42</code></a>"
        ),
        "{use_case}"
    );
    assert!(
        use_case.contains("<a href=\"/code/app/card.rb\"><code>app/card.rb</code></a>"),
        "{use_case}"
    );
}

// spec: WEB-32
#[tokio::test]
async fn a_range_just_past_the_end_or_on_an_empty_file_is_refused() {
    let (_dir, root) = generated_repo();
    let past = page(&root, "/code/app/invoice.rb?lines=505-510").await;
    assert!(past.contains("line 505 is past the end"), "{past}");
    assert_eq!(shown_lines(&past), Vec::<usize>::new());

    std::fs::write(root.join("app/card.rb"), "").unwrap();
    let empty = page(&root, "/code/app/card.rb").await;
    assert!(empty.contains("The file is empty."), "{empty}");
    assert!(!empty.contains("lines 0-0"), "{empty}");
    let past = page(&root, "/code/app/card.rb?lines=5").await;
    assert!(past.contains("line 5 is past the end"), "{past}");
}

// spec: WEB-38
#[tokio::test]
async fn a_window_before_the_end_links_to_the_next_lines() {
    let (_dir, root) = generated_repo();
    let top = page(&root, "/code/app/invoice.rb").await;
    assert!(
        top.contains("<a href=\"/code/app/invoice.rb?from=201#L201\">"),
        "{top}"
    );
    let next = page(&root, "/code/app/invoice.rb?from=201").await;
    assert_eq!(shown_lines(&next), (201..=400).collect::<Vec<_>>());
    assert_eq!(cited_lines(&next), Vec::<usize>::new());
    assert!(next.contains("?from=401#L401"), "{next}");
    let last = page(&root, "/code/app/invoice.rb?from=401").await;
    assert_eq!(shown_lines(&last), (401..=500).collect::<Vec<_>>());
    assert!(!last.contains("?from="), "{last}");

    let wide = page(&root, "/code/app/invoice.rb?lines=20-480").await;
    assert_eq!(shown_lines(&wide), (10..=409).collect::<Vec<_>>());
    assert!(wide.contains("?from=410#L410"), "{wide}");
}
