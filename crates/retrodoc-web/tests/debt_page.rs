//! The debt page, its filters and the links of every page to it, asked of the
//! router without a socket over the artifacts a run leaves in `.retrodoc/cache/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{Actor, ActorKind, ConfidenceScore, Feature, SourceRef, Step, UseCase};
use retrodoc_mcp::{GeneratedDocs, Uncovered};
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::reconcile::{
    Contradiction, ItemKind, ItemVerdict, Reconciliation, SectionRef, Status,
};
use retrodoc_pipeline::{BehaviourCoverage, DomainCluster, DomainMap, SourceHashes};
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use tower::ServiceExt;

fn score(value: f32, why: &str) -> ConfidenceScore {
    ConfidenceScore::new(value, why.to_string())
}

fn feature(slug: &str, domain: &str, name: &str) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: name.into(),
        description: String::new(),
        source_paths: vec![format!("app/{slug}.rb")],
        entry_points: Vec::new(),
        confidence: Some(score(0.9, "fine")),
    }
}

fn use_case(slug: &str, feature: &str, name: &str, confidence: Option<f32>) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: slug.into(),
        feature_slug: feature.into(),
        name: name.into(),
        description: String::new(),
        steps: vec![Step {
            order: 1,
            description: "Acts".into(),
            actor: Actor {
                name: "Clerk".into(),
                kind: ActorKind::Human,
            },
            action: String::new(),
            source_refs: vec![SourceRef {
                path: format!("app/{feature}.rb"),
                start_line: Some(1),
                end_line: None,
            }],
        }],
        diagram_mermaid: None,
        confidence: confidence.map(|c| score(c, "few cited lines")),
        narrative_check: None,
    }
}

/// `billing` (feature `pay-invoice`: `Refund <script>` low at 30%, `Pay by card` not scored)
/// and `shipping` (feature `ship-order`, whose file changed since the run: `Ship` stale).
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(
        root,
        &[
            feature("pay-invoice", "billing", "Pay an invoice"),
            feature("ship-order", "shipping", "Ship an order"),
        ],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(
        root,
        &[
            use_case("refund", "pay-invoice", "Refund <script>", Some(0.3)),
            use_case("pay-by-card", "pay-invoice", "Pay by card", None),
            use_case("ship", "ship-order", "Ship", Some(0.9)),
        ],
    )
    .unwrap();
    DomainMap {
        domains: ["billing", "shipping"]
            .map(|slug| DomainCluster {
                slug: slug.into(),
                name: format!("{}{}", slug[..1].to_uppercase(), &slug[1..]),
                ..DomainCluster::default()
            })
            .to_vec(),
    }
    .save(root)
    .unwrap();
    std::fs::create_dir_all(root.join("app")).unwrap();
    let mut hashes = SourceHashes::default();
    for name in ["app/pay-invoice.rb", "app/ship-order.rb"] {
        std::fs::write(root.join(name), name).unwrap();
        hashes.put(name, &hash_content(name));
    }
    hashes.save(root).unwrap();
    std::fs::write(root.join("app/ship-order.rb"), "edited").unwrap();
    dir
}

async fn get_in(
    lang: Option<&'static str>,
    root: &Path,
    path: &str,
    headers: &[&str],
) -> (StatusCode, String) {
    get_docs(lang, GeneratedDocs::load(root, &[]), path, headers).await
}

async fn get_docs(
    lang: Option<&'static str>,
    docs: Option<GeneratedDocs>,
    path: &str,
    headers: &[&str],
) -> (StatusCode, String) {
    let mut request = Request::get(path);
    for name in headers {
        request = request.header(*name, "true");
    }
    let project = Project {
        lang,
        ..Project::default()
    };
    let response = retrodoc_web::router(project, docs, DocLinks::default(), Vec::new())
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn debt(root: &Path, path: &str) -> String {
    let (status, page) = get_in(None, root, path, &[]).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

/// The list of the items, without the form.
fn list(page: &str) -> &str {
    let start = page.find("id=\"debt-list\"").expect(page);
    &page[start..]
}

/// The items of the list, one string per `<li class="debt-item">`.
fn items(page: &str) -> Vec<&str> {
    list(page)
        .split("<li class=\"debt-item\"")
        .skip(1)
        .collect()
}

fn shown(page: &str) -> Vec<&'static str> {
    let list = list(page);
    [
        "Refund &#60;script&#62;",
        "Pay by card",
        ">Ship<",
        ">Ship an order<",
    ]
    .into_iter()
    .filter(|s| list.contains(s))
    .collect()
}

// spec: DBT-1, DBT-2, DBT-3, DBT-7, DBT-8
#[tokio::test]
async fn the_debt_page_lists_each_section_that_needs_a_human() {
    let repo = generated_repo();
    let page = debt(repo.path(), "/debt").await;
    let items = items(&page);

    // Below the stale sections (weight 1, weakness 0.7 against 1), above the not scored and set
    // apart ones (weight 0.5 and 0.25).
    let low = items[2];
    assert!(low.contains("Low confidence"), "{low}");
    assert!(low.contains("Billing"), "{low}");
    assert!(
        low.contains("<a href=\"/domains/billing/pay-invoice/refund\">"),
        "{low}"
    );
    assert!(low.contains("Refund &#60;script&#62;"), "{low}");
    assert!(low.contains("Pay an invoice"), "{low}");
    assert!(
        low.contains("30%") && low.contains("few cited lines"),
        "{low}"
    );
    // 30% is under the confidence that sets a use case apart too.
    let kinds: Vec<&str> = items
        .iter()
        .map(|item| {
            between(item, "class=\"badge kind\">", "</span>")
                .trim_start_matches("class=\"badge kind\">")
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "Stale",
            "Stale",
            "Low confidence",
            "Not scored",
            "Set apart"
        ],
        "{page}"
    );
    assert!(items[3].contains("/domains/billing/pay-invoice/pay-by-card"));
    assert!(items[4].contains("confidence"), "{}", items[4]);
    assert!(
        items[0].contains("<a href=\"/domains/shipping/ship-order\">Ship an order</a>"),
        "{}",
        items[0]
    );
    assert!(items[0].contains("app/ship-order.rb"), "{}", items[0]);
    assert!(items[1].contains("/domains/shipping/ship-order/ship"));
}

// spec: DBT-3
#[tokio::test]
async fn a_run_with_no_score_says_so_rather_than_listing_every_use_case() {
    let repo = generated_repo();
    retrodoc_pipeline::save_use_cases(
        repo.path(),
        &[use_case("refund", "pay-invoice", "Refund", None)],
    )
    .unwrap();
    let page = debt(repo.path(), "/debt").await;
    assert!(list(&page).contains("no confidence score"), "{page}");
    assert!(!list(&page).contains("Not scored"), "{page}");
}

// spec: DBT-3, DBT-11
#[tokio::test]
async fn a_run_with_no_score_and_nothing_else_does_not_say_the_docs_need_no_human() {
    let clean = tempfile::tempdir().unwrap();
    let mut unscored = feature("pay-invoice", "billing", "Pay an invoice");
    unscored.confidence = None;
    retrodoc_pipeline::save_features(clean.path(), &[unscored]).unwrap();
    retrodoc_pipeline::save_use_cases(
        clean.path(),
        &[use_case("refund", "pay-invoice", "Refund", None)],
    )
    .unwrap();
    let page = debt(clean.path(), "/debt").await;
    assert!(list(&page).contains("no confidence score"), "{page}");
    assert!(!list(&page).contains("needs a human"), "{page}");
}

// spec: DBT-9
#[tokio::test]
async fn the_debt_page_keeps_the_items_of_the_kind_and_the_domain_asked() {
    let repo = generated_repo();
    for (query, expected) in [
        (
            "",
            &[
                "Refund &#60;script&#62;",
                "Pay by card",
                ">Ship<",
                ">Ship an order<",
            ][..],
        ),
        (
            "?kind=",
            &[
                "Refund &#60;script&#62;",
                "Pay by card",
                ">Ship<",
                ">Ship an order<",
            ][..],
        ),
        ("?kind=not_scored", &["Pay by card"][..]),
        ("?domain=shipping", &[">Ship<", ">Ship an order<"][..]),
        ("?kind=stale&domain=billing", &[][..]),
        (
            "?kind=low_confidence&domain=billing",
            &["Refund &#60;script&#62;"][..],
        ),
        ("?kind=nonsense", &[][..]),
    ] {
        let page = debt(repo.path(), &format!("/debt{query}")).await;
        assert_eq!(shown(&page), expected, "{query}");
    }
}

// spec: DBT-10
#[tokio::test]
async fn the_debt_form_offers_the_kinds_and_the_domains_with_the_asked_ones_selected() {
    let repo = generated_repo();
    let page = debt(repo.path(), "/debt?kind=stale&domain=shipping").await;
    let form = between(&page, "<form class=\"filters\"", "</form>");
    assert!(form.contains("hx-get=\"/debt\""), "{form}");
    for kind in [
        "low_confidence",
        "not_scored",
        "unbacked_narrative",
        "technical_language",
        "set_apart",
        "contradicted",
        "undocumented",
        "entry_point_without_feature",
        "no_domain",
        "no_behaviour",
        "not_analysed",
    ] {
        assert!(
            form.contains(&format!(
                "<input type=\"radio\" name=\"kind\" value=\"{kind}\">"
            )),
            "{kind}: {form}"
        );
    }
    assert!(
        form.contains("<input type=\"radio\" name=\"kind\" value=\"stale\" checked>"),
        "{form}"
    );
    assert!(!form.contains("value=\"\" checked"), "{form}");
    assert!(
        form.contains("<option value=\"billing\">Billing</option>"),
        "{form}"
    );
    assert!(
        form.contains("<option value=\"shipping\" selected>Shipping</option>"),
        "{form}"
    );
}

// spec: WEB-107, WEB-108
#[tokio::test]
async fn the_debt_filters_apply_on_change_and_the_list_counts_them() {
    let repo = generated_repo();
    let page = debt(repo.path(), "/debt").await;
    let form = between(&page, "<form class=\"filters\"", "</form>");
    assert!(form.contains("hx-trigger=\"change\""), "{form}");
    assert!(
        form.contains("<input type=\"radio\" name=\"kind\" value=\"\" checked>"),
        "{form}"
    );
    assert!(form.contains("<option value=\"\">All</option>"), "{form}");
    assert!(
        between(form, "<noscript>", "</noscript>").contains("<button type=\"submit\""),
        "{form}"
    );
    assert!(!list(&page).contains("Reset filters"), "{page}");

    let (_, fragment) = get_in(
        None,
        repo.path(),
        "/debt?kind=stale&domain=shipping",
        &["HX-Request"],
    )
    .await;
    assert!(fragment.contains("2 active filters"), "{fragment}");
    assert!(
        fragment.contains("<a href=\"/debt\">Reset filters</a>"),
        "{fragment}"
    );
}

// spec: DBT-11
#[tokio::test]
async fn the_debt_page_says_when_nothing_needs_a_human_or_nothing_matches() {
    let repo = generated_repo();
    let page = debt(repo.path(), "/debt?kind=unbacked_narrative").await;
    assert!(list(&page).contains("No item matches."), "{page}");

    let clean = tempfile::tempdir().unwrap();
    let mut fine = feature("pay-invoice", "billing", "Pay an invoice");
    fine.source_paths.clear();
    retrodoc_pipeline::save_features(clean.path(), &[fine]).unwrap();
    let page = debt(clean.path(), "/debt").await;
    assert!(
        list(&page).contains("No section of the docs needs a human."),
        "{page}"
    );
}

// spec: DBT-12
#[tokio::test]
async fn an_htmx_debt_request_gets_the_list_alone() {
    let repo = generated_repo();
    let (_, fragment) = get_in(None, repo.path(), "/debt?kind=not_scored", &["HX-Request"]).await;
    assert!(
        fragment
            .trim_start()
            .starts_with("<section id=\"debt-list\""),
        "{fragment}"
    );
    assert!(!fragment.contains("<header"), "{fragment}");
    assert!(fragment.contains("Pay by card"), "{fragment}");
    let (_, restored) = get_in(
        None,
        repo.path(),
        "/debt",
        &["HX-Request", "HX-History-Restore-Request"],
    )
    .await;
    assert!(restored.contains("<header"), "{restored}");
}

// spec: DBT-13
#[tokio::test]
async fn every_page_links_to_the_debt_page_under_audit_and_the_home_page_counts_its_items() {
    let repo = generated_repo();
    let empty = tempfile::tempdir().unwrap();
    for (root, path) in [
        (repo.path(), "/"),
        (repo.path(), "/rules"),
        (repo.path(), "/debt"),
        (repo.path(), "/unknown"),
        (empty.path(), "/"),
    ] {
        let (_, page) = get_in(None, root, path, &[]).await;
        let header = between(&page, "<header", "</header>");
        let nav = between(header, "aria-label=\"Audit\"", "</nav>");
        let link = between(nav, "href=\"/debt\"", "</a>");
        assert!(link.contains(" title=\""), "{path}: {nav}");
    }
    let home = debt(repo.path(), "/").await;
    let start = between(&home, "id=\"start-here\"", "</section>");
    let item = between(start, "href=\"/debt\"", "</li>");
    assert!(item.contains("5 items"), "{item}");
}

// spec: DBT-13
#[tokio::test]
async fn the_count_of_the_home_page_follows_a_file_changed_after_the_start() {
    let repo = generated_repo();
    let router = retrodoc_web::router(
        Project::default(),
        GeneratedDocs::load(repo.path(), &[]),
        DocLinks::default(),
        Vec::new(),
    );
    let count = |router: axum::Router| async move {
        let response = router
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let page = String::from_utf8(body.to_vec()).unwrap();
        let start = between(&page, "id=\"start-here\"", "</section>").to_string();
        between(&start, "href=\"/debt\"", "</li>").to_string()
    };
    assert!(count(router.clone()).await.contains("5 items"));
    // The feature `pay-invoice` and its two use cases cite it.
    std::fs::remove_file(repo.path().join("app/pay-invoice.rb")).unwrap();
    let after = count(router).await;
    assert!(after.contains("8 items"), "{after}");
}

// spec: DBT-14
#[tokio::test]
async fn the_debt_page_of_a_repository_with_no_run_says_to_run_generate() {
    let empty = tempfile::tempdir().unwrap();
    let page = debt(empty.path(), "/debt").await;
    assert!(page.contains("retrodoc generate"), "{page}");
    let (_, fragment) = get_in(None, empty.path(), "/debt", &["HX-Request"]).await;
    assert!(fragment.contains("retrodoc generate"), "{fragment}");
}

// spec: WEB-77, DBT-1, DBT-27
#[tokio::test]
async fn the_debt_page_is_in_french() {
    let repo = generated_repo();
    let (_, page) = get_in(Some("fr"), repo.path(), "/debt", &[]).await;
    for label in [
        "Confiance faible",
        "Non évalué",
        "Mis à part",
        "Périmé",
        "Dette",
    ] {
        assert!(page.contains(label), "{label} missing from {page}");
    }
    for label in ["Low confidence", "Not scored", "Set apart", ">Stale<"] {
        assert!(!page.contains(label), "{label} left in {page}");
    }
    // Zero takes the singular in French.
    assert!(page.contains("0 commit, 0 point d&#39;entrée"), "{page}");
}

fn git(root: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// [`generated_repo`] under git, `app/pay-invoice.rb` changed by two commits and
/// `app/ship-order.rb` by one.
fn committed_repo() -> tempfile::TempDir {
    let repo = generated_repo();
    let root = repo.path();
    let file = root.join("app/pay-invoice.rb");
    let content = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, "before").unwrap();
    git(root, &["init", "-q"]);
    git(root, &["add", "app"]);
    git(root, &["commit", "-q", "-m", "first"]);
    std::fs::write(&file, content).unwrap();
    git(root, &["commit", "-q", "-am", "second"]);
    repo
}

async fn debt_with_history(root: &Path) -> String {
    let docs = GeneratedDocs::load(root, &[]).map(GeneratedDocs::with_history);
    let (status, page) = get_docs(None, docs, "/debt", &[]).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

// spec: DBT-25, DBT-27, DBT-8, DBT-29
#[tokio::test]
async fn each_item_shows_its_priority_and_its_parts_from_the_history() {
    let repo = committed_repo();
    let page = debt_with_history(repo.path()).await;
    let items = items(&page);

    let shows = |at: usize, kind: &str, priority: &str| {
        let item: &str = items[at];
        assert!(item.contains(kind), "{item}");
        assert!(item.contains(priority), "{item}");
    };
    // 1 × 1 × (1 + ln 2): the one commit of the file its steps cite.
    shows(
        0,
        "Stale",
        "Priority 1.69: weight 1.00, weakness 1.00, 1 commit, 0 entry points",
    );
    // 1 × 0.7 × (1 + ln 3): the two commits of the file its step cites.
    shows(
        2,
        "Low confidence",
        "Priority 1.47: weight 1.00, weakness 0.70, 2 commits, 0 entry points",
    );
    shows(
        3,
        "Pay by card",
        "Priority 1.05: weight 0.50, weakness 1.00, 2 commits, 0 entry points",
    );
    shows(
        4,
        "Set apart",
        "Priority 0.52: weight 0.25, weakness 1.00, 2 commits, 0 entry points",
    );
    assert!(!page.contains("could not be read"), "{page}");
}

// spec: DBT-28
#[tokio::test]
async fn without_a_git_history_the_page_says_the_commits_are_unknown() {
    let repo = generated_repo();
    let page = debt_with_history(repo.path()).await;

    assert!(
        list(&page).contains("The git history could not be read: every item counts 0 commits."),
        "{page}"
    );
    assert!(items(&page)[0].contains("0 commits"), "{page}");
}

fn verdict(kind: ItemKind, label: &str, name: &str, status: Status) -> ItemVerdict {
    ItemVerdict {
        kind,
        label: label.into(),
        domain: label.split('/').next().unwrap_or_default().into(),
        name: name.into(),
        entry_points: 4,
        confidence: None,
        status,
        sections: Vec::new(),
        contradictions: Vec::new(),
    }
}

/// The run of [`generated_repo`] with what its docs leave out: a use case `README.md` contradicts,
/// an undocumented rule and feature, files and an entry point no doc covers.
fn uncovered_docs(root: &Path) -> GeneratedDocs {
    let mut refund = verdict(
        ItemKind::UseCase,
        "billing/pay-invoice/refund",
        "Refund <script>",
        Status::Contradicted,
    );
    refund.contradictions = vec![Contradiction {
        section: SectionRef::parse("README.md#Refunds and returns"),
        doc_states: "Refunds within 60 days".into(),
        item_states: "Refunds within 30 days".into(),
    }];
    let paths = |paths: &[&str]| paths.iter().map(Into::into).collect::<Vec<_>>();
    for file in ["lib/c.rb", "app/a.rb", "app/b.rb"] {
        std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
        std::fs::write(root.join(file), "puts 1\n").unwrap();
    }
    GeneratedDocs::load(root, &[])
        .unwrap()
        .with_uncovered(Uncovered {
            reconciliation: Some(Reconciliation {
                items: vec![
                    refund,
                    verdict(
                        ItemKind::Rule,
                        "billing/pay-invoice/pay-invoice-1",
                        "Paid within 30 days",
                        Status::Undocumented,
                    ),
                    ItemVerdict {
                        entry_points: 1,
                        ..verdict(ItemKind::Feature, "?/gone", "Gone", Status::Undocumented)
                    },
                ],
            }),
            no_domain: Vec::new(),
            skipped: Vec::new(),
            coverage: Some(BehaviourCoverage {
                unplaced_entry_points: vec!["app/a.rb#purge".into()],
                technical_support: paths(&["app/a.rb", "app/b.rb", "lib/c.rb"]),
                ..BehaviourCoverage::default()
            }),
        })
}

async fn uncovered_page(root: &Path, path: &str) -> String {
    let (status, page) = get_docs(None, Some(uncovered_docs(root)), path, &[]).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

// spec: DBT-15, DBT-18, WEB-10
#[tokio::test]
async fn a_contradicted_item_links_its_page_and_the_section_that_contradicts_it() {
    let repo = generated_repo();
    let page = uncovered_page(repo.path(), "/debt?kind=contradicted").await;
    let items = items(&page);
    assert_eq!(items.len(), 1, "{page}");
    let item = items[0];
    assert!(item.contains("Contradicted"), "{item}");
    assert!(item.contains("use case"), "{item}");
    assert!(
        item.contains(
            "<a href=\"/domains/billing/pay-invoice/refund\">Refund &#60;script&#62;</a>"
        ),
        "{item}"
    );
    assert!(item.contains("Billing"), "{item}");
    assert!(
        item.contains("href=\"/docs/README.md#refunds-and-returns\""),
        "{item}"
    );
    assert!(item.contains("Refunds within 60 days"), "{item}");
    assert!(item.contains("Refunds within 30 days"), "{item}");
}

// spec: DBT-16, DBT-18, DBT-9
#[tokio::test]
async fn an_undocumented_item_counts_its_entry_points_and_links_its_page() {
    let repo = generated_repo();
    let page = uncovered_page(repo.path(), "/debt?kind=undocumented").await;
    let items: Vec<&str> = items(&page)
        .into_iter()
        .map(|item| item.split("</li>").next().unwrap_or_default())
        .collect();
    assert_eq!(items.len(), 2, "{page}");
    assert!(items[0].contains("rule"), "{}", items[0]);
    assert!(
        items[0].contains(
            "<a href=\"/domains/billing/pay-invoice#rule-pay-invoice-1\">Paid within 30 days</a>"
        ),
        "{}",
        items[0]
    );
    assert!(items[0].contains("4 entry points"), "{}", items[0]);
    assert!(items[1].contains("Gone"), "{}", items[1]);
    assert!(!items[1].contains("<a href"), "{}", items[1]);

    let billing = uncovered_page(repo.path(), "/debt?kind=undocumented&domain=billing").await;
    assert_eq!(
        list(&billing).matches("<li class=\"debt-item\"").count(),
        1,
        "{billing}"
    );
}

// spec: DBT-17
#[tokio::test]
async fn with_no_collected_doc_the_page_says_there_is_nothing_to_compare() {
    let repo = generated_repo();
    let page = debt(repo.path(), "/debt").await;
    assert!(
        list(&page).contains("No project doc to compare the items with"),
        "{page}"
    );
    let page = uncovered_page(repo.path(), "/debt").await;
    assert!(
        !list(&page).contains("No project doc to compare the items with"),
        "{page}"
    );
}

// spec: DBT-19, DBT-20, DBT-21, DBT-22, DBT-8
#[tokio::test]
async fn the_code_no_doc_covers_is_listed_by_folder_with_links_to_its_files() {
    let repo = generated_repo();
    let page = uncovered_page(repo.path(), "/debt").await;
    let folders: Vec<&str> = items(&page)
        .into_iter()
        .filter(|i| i.contains("class=\"folder\""))
        .collect();
    assert_eq!(folders.len(), 3, "{page}");
    assert!(
        folders[0].contains("Entry point without a feature"),
        "{}",
        folders[0]
    );
    assert!(folders[0].contains("1 entry point"), "{}", folders[0]);
    assert!(
        folders[0].contains("<a href=\"/code/app/a.rb\">app/a.rb#purge</a>"),
        "{}",
        folders[0]
    );
    assert!(
        folders[1].contains("Reached by no behaviour"),
        "{}",
        folders[1]
    );
    assert!(folders[1].contains("<code>app</code>"), "{}", folders[1]);
    assert!(folders[1].contains("2 files"), "{}", folders[1]);
    assert!(
        folders[1].contains("<a href=\"/code/app/b.rb\">app/b.rb</a>"),
        "{}",
        folders[1]
    );
    assert!(folders[2].contains("<code>lib</code>"), "{}", folders[2]);
    assert!(folders[2].contains("1 file<"), "{}", folders[2]);

    let billing = uncovered_page(repo.path(), "/debt?domain=billing").await;
    assert!(!list(&billing).contains("class=\"folder\""), "{billing}");
}

// spec: WEB-31, WEB-36, DBT-21
#[tokio::test]
async fn the_files_of_the_debt_are_served_though_no_doc_cites_them() {
    let repo = generated_repo();
    let (status, page) = get_docs(
        None,
        Some(uncovered_docs(repo.path())),
        "/code/lib/c.rb",
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains("puts 1"), "{page}");

    let (_, files) = get_docs(None, Some(uncovered_docs(repo.path())), "/files", &[]).await;
    let row = between(&files, "id=\"lib/c.rb\"", "</tr>");
    assert!(row.contains("href=\"/code/lib/c.rb\""), "{row}");
    assert!(row.contains("not cited"), "{row}");

    let (status, _) = get_in(None, repo.path(), "/code/lib/c.rb", &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "listed by no debt item");
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{start} not in {text}"));
    let rest = &text[from..];
    let to = rest.find(end).unwrap_or(rest.len());
    &rest[..to]
}
