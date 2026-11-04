use retrodoc_core::model::{ConfidenceScore, Domain, Feature, UseCase};
use retrodoc_pipeline::Glossary;

use super::*;

fn domain(slug: &str) -> Domain {
    Domain {
        slug: slug.into(),
        name: slug.to_uppercase(),
        description: String::new(),
        confidence: Some(ConfidenceScore::new(0.7, None)),
    }
}

fn feature(slug: &str, domain: &str) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: slug.to_uppercase(),
        description: String::new(),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: Some(ConfidenceScore::new(0.8, None)),
    }
}

fn use_case(slug: &str, feature: &str, narrative: &str) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: Some(narrative.into()),
        business_language: None,
        slug: slug.into(),
        feature_slug: feature.into(),
        name: slug.to_uppercase(),
        description: String::new(),
        steps: Vec::new(),
        diagram_mermaid: None,
        confidence: Some(ConfidenceScore::new(0.6, None)),
        narrative_check: None,
    }
}

fn docs_with(narrative: &str) -> GeneratedDocs {
    GeneratedDocs::new(
        vec![domain("billing"), domain("a")],
        vec![feature("pay", "billing"), feature("a", "a")],
        vec![use_case("refund", "pay", narrative), use_case("b", "a", "")],
        Glossary::default(),
        &[],
    )
}

fn docs() -> GeneratedDocs {
    docs_with("The clerk refunds the order.")
}

/// A git repository whose `user.name` is `name`.
fn repo(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for args in [&["init", "-q"][..], &["config", "user.name", name]] {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(status.success());
    }
    dir
}

fn feature_pay() -> Section {
    Section::new(SectionKind::Feature, "pay")
}

fn refund() -> Section {
    Section::new(SectionKind::UseCase, "pay/refund")
}

// spec: REV-1, REV-2, REV-3, REV-4, REV-5 (internal: the binary writes no review yet, the web form will)
#[test]
fn a_review_holds_its_section_status_comment_author_and_date() {
    let dir = repo("Ada Reviewer");

    let review = record(
        dir.path(),
        &docs(),
        &refund(),
        Status::Wrong,
        "The refund needs a manager.",
    )
    .unwrap();

    assert_eq!(review.section, refund());
    assert_eq!(review.status, Status::Wrong);
    assert_eq!(review.comment, "The refund needs a manager.");
    assert_eq!(review.author.as_deref(), Some("Ada Reviewer"));
    let date = chrono::DateTime::parse_from_rfc3339(&review.reviewed_at).unwrap();
    assert_eq!(date.offset().local_minus_utc(), 0);
    assert!(review.reviewed_at.ends_with('Z'));
    assert!(!review.reviewed_at.contains('.'));
    assert_eq!(load(dir.path()).unwrap().reviews(), [review]);
}

// spec: REV-2, REV-3 (internal: the format of the file the other crates read)
#[test]
fn the_status_and_an_empty_comment_are_saved_as_named() {
    let dir = repo("A");

    record(dir.path(), &docs(), &refund(), Status::ToReview, "").unwrap();

    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(reviews_path(dir.path())).unwrap()).unwrap();
    let review = &saved["reviews"][0];
    assert_eq!(review["kind"], "use_case");
    assert_eq!(review["id"], "pay/refund");
    assert_eq!(review["status"], "to_review");
    assert_eq!(review["comment"], "");
    assert_eq!(saved["version"], VERSION);
}

// spec: REV-4 (internal: the binary writes no review yet)
#[test]
fn a_review_outside_a_git_repository_has_no_author() {
    let dir = tempfile::tempdir().unwrap();

    let review = record(dir.path(), &docs(), &feature_pay(), Status::Validated, "").unwrap();

    assert_eq!(review.author, None);
}

// spec: REV-6 (internal: the fingerprint of the read model's sections)
#[test]
fn the_fingerprint_follows_the_content_of_the_section() {
    let before = fingerprint(&docs(), &refund()).unwrap();

    assert_eq!(fingerprint(&docs(), &refund()).unwrap(), before);
    assert_ne!(
        fingerprint(&docs_with("The clerk refunds the paid order."), &refund()).unwrap(),
        before
    );
    assert_ne!(fingerprint(&docs(), &feature_pay()).unwrap(), before);
    let dir = repo("A");
    let review = record(dir.path(), &docs(), &refund(), Status::Validated, "").unwrap();
    assert_eq!(review.fingerprint, before);
}

// spec: REV-1 (internal: the ids of the read model's sections)
#[test]
fn a_feature_and_a_use_case_with_the_same_slug_are_two_sections() {
    let dir = repo("A");
    let feature_a = Section::new(SectionKind::Feature, "a");
    let use_case_ab = Section::new(SectionKind::UseCase, "a/b");
    let domain_a = Section::new(SectionKind::Domain, "a");

    record(dir.path(), &docs(), &feature_a, Status::Validated, "").unwrap();
    record(dir.path(), &docs(), &use_case_ab, Status::Wrong, "").unwrap();
    record(dir.path(), &docs(), &domain_a, Status::ToReview, "").unwrap();

    let reviews = load(dir.path()).unwrap();
    assert_eq!(reviews.reviews().len(), 3);
    assert_eq!(reviews.get(&feature_a).unwrap().status, Status::Validated);
    assert_eq!(reviews.get(&use_case_ab).unwrap().status, Status::Wrong);
    assert_eq!(reviews.get(&domain_a).unwrap().status, Status::ToReview);
}

// spec: REV-7 (internal: the path of the file; `generate` keeping it is tested through the binary)
#[test]
fn the_reviews_are_saved_in_one_file_outside_the_cache() {
    let dir = repo("A");

    record(dir.path(), &docs(), &refund(), Status::Validated, "").unwrap();

    assert_eq!(
        reviews_path(dir.path()),
        dir.path().join(".retrodoc/reviews.json")
    );
    assert!(reviews_path(dir.path()).is_file());
}

// spec: REV-9 (internal: what a save leaves on disk)
#[test]
fn a_save_leaves_only_the_whole_file() {
    let dir = repo("A");

    record(dir.path(), &docs(), &refund(), Status::Validated, "").unwrap();
    record(dir.path(), &docs(), &feature_pay(), Status::Wrong, "").unwrap();

    let names: Vec<String> = std::fs::read_dir(dir.path().join(".retrodoc"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["reviews.json"]);
    assert_eq!(load(dir.path()).unwrap().reviews().len(), 2);
}

// spec: REV-10, REV-11 (internal: an error the web form will report)
#[test]
fn an_unreadable_file_is_reported_and_kept() {
    let dir = repo("A");
    let path = reviews_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ not json").unwrap();

    assert!(matches!(
        load(dir.path()),
        Err(ReviewsError::Unreadable { .. })
    ));
    assert!(record(dir.path(), &docs(), &refund(), Status::Validated, "").is_err());
    assert!(remove(dir.path(), &refund()).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
}

// spec: REV-10, REV-11 (internal: an error the web form will report)
#[test]
fn a_file_of_a_later_format_is_reported_and_kept() {
    let dir = repo("A");
    let path = reviews_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let later = format!(r#"{{"version": {}, "reviews": []}}"#, VERSION + 1);
    std::fs::write(&path, &later).unwrap();

    let error = load(dir.path()).unwrap_err();
    assert!(matches!(error, ReviewsError::LaterFormat { version, .. } if version == VERSION + 1));
    assert!(error.to_string().contains(&(VERSION + 1).to_string()));
    assert!(record(dir.path(), &docs(), &refund(), Status::Validated, "").is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), later);
}

// spec: REV-12 (internal: the binary writes no review yet)
#[test]
fn a_new_review_replaces_the_former_one() {
    let first = repo("A");
    record(
        first.path(),
        &docs(),
        &feature_pay(),
        Status::Validated,
        "ok",
    )
    .unwrap();
    // B reviews in the same folder with another git identity.
    let status = std::process::Command::new("git")
        .args(["config", "user.name", "B"])
        .current_dir(first.path())
        .status()
        .unwrap();
    assert!(status.success());

    record(first.path(), &docs(), &feature_pay(), Status::Wrong, "no").unwrap();

    let reviews = load(first.path()).unwrap();
    assert_eq!(reviews.reviews().len(), 1);
    let review = reviews.get(&feature_pay()).unwrap();
    assert_eq!(review.status, Status::Wrong);
    assert_eq!(review.comment, "no");
    assert_eq!(review.author.as_deref(), Some("B"));
}

// spec: REV-13 (internal: the binary writes no review yet)
#[test]
fn removing_a_review_keeps_the_others() {
    let dir = repo("A");
    let billing = Section::new(SectionKind::Domain, "billing");
    record(dir.path(), &docs(), &billing, Status::Validated, "").unwrap();
    record(dir.path(), &docs(), &refund(), Status::Wrong, "").unwrap();

    remove(dir.path(), &billing).unwrap();

    let reviews = load(dir.path()).unwrap();
    assert_eq!(reviews.get(&billing), None);
    assert_eq!(reviews.reviews().len(), 1);
    assert_eq!(reviews.get(&refund()).unwrap().status, Status::Wrong);
}

// spec: REV-14 (internal: the binary writes no review yet)
#[test]
fn an_absent_file_is_no_review() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(load(dir.path()).unwrap().reviews(), []);
    remove(dir.path(), &refund()).unwrap();
    assert!(!reviews_path(dir.path()).exists());
}

// spec: REV-15 (internal: the layout of the file)
#[test]
fn the_reviews_are_saved_by_kind_then_id() {
    let dir = repo("A");
    for section in [
        Section::new(SectionKind::UseCase, "pay/refund"),
        Section::new(SectionKind::Feature, "pay"),
        Section::new(SectionKind::Domain, "billing"),
        Section::new(SectionKind::Feature, "a"),
        Section::new(SectionKind::Domain, "a"),
    ] {
        record(dir.path(), &docs(), &section, Status::Validated, "").unwrap();
    }

    // As written on disk: `load` sorts too.
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(reviews_path(dir.path())).unwrap()).unwrap();
    let order: Vec<String> = saved["reviews"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            format!(
                "{} {}",
                r["kind"].as_str().unwrap(),
                r["id"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            "domain a",
            "domain billing",
            "feature a",
            "feature pay",
            "use_case pay/refund",
        ]
    );
}

// spec: REV-16 (internal: the binary writes no review yet)
#[test]
fn a_review_of_a_section_the_docs_lack_is_refused() {
    let dir = repo("A");

    let error = record(
        dir.path(),
        &docs(),
        &Section::new(SectionKind::Feature, "ship"),
        Status::Validated,
        "",
    )
    .unwrap_err();

    assert!(matches!(error, ReviewsError::UnknownSection(_)));
    assert_eq!(error.to_string(), "the docs hold no feature ship");
    assert!(!reviews_path(dir.path()).exists());
}

// spec: REV-17 (internal: a file merged by hand)
#[test]
fn several_reviews_of_one_section_load_as_the_newest() {
    let dir = tempfile::tempdir().unwrap();
    let path = reviews_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let review = |status: &str, at: &str| {
        format!(
            r#"{{"kind": "feature", "id": "pay", "status": "{status}", "comment": "", "author": null, "reviewed_at": "{at}", "fingerprint": "f"}}"#
        )
    };
    let newest = review("wrong", "2026-11-05T09:00:00Z");
    let older = review("validated", "2026-11-04T09:00:00Z");
    std::fs::write(
        &path,
        format!(r#"{{"version": 1, "reviews": [{newest}, {older}]}}"#),
    )
    .unwrap();

    let reviews = load(dir.path()).unwrap();

    assert_eq!(reviews.reviews().len(), 1);
    assert_eq!(reviews.get(&feature_pay()).unwrap().status, Status::Wrong);
}

// spec: REV-3 (internal: a file written by hand)
#[test]
fn a_review_with_no_comment_loads_with_an_empty_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = reviews_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{"version": 1, "reviews": [{"kind": "feature", "id": "pay", "status": "validated", "author": null, "reviewed_at": "2026-11-04T09:00:00Z", "fingerprint": "f"}]}"#,
    )
    .unwrap();

    assert_eq!(
        load(dir.path())
            .unwrap()
            .get(&feature_pay())
            .unwrap()
            .comment,
        ""
    );
}

// spec: REV-18 (internal: the requests of the web server run on threads of one process)
#[test]
fn reviews_saved_at_the_same_time_by_one_process_are_all_kept() {
    let dir = repo("A");
    let docs = docs();
    let sections = [
        Section::new(SectionKind::Domain, "billing"),
        Section::new(SectionKind::Domain, "a"),
        Section::new(SectionKind::Feature, "pay"),
        Section::new(SectionKind::Feature, "a"),
        Section::new(SectionKind::UseCase, "pay/refund"),
        Section::new(SectionKind::UseCase, "a/b"),
    ];

    for _ in 0..5 {
        std::thread::scope(|scope| {
            for section in &sections {
                scope.spawn(|| record(dir.path(), &docs, section, Status::Validated, "").unwrap());
            }
        });
        assert_eq!(load(dir.path()).unwrap().reviews().len(), sections.len());
        for section in &sections {
            remove(dir.path(), section).unwrap();
        }
    }
}

// spec: REV-4 (internal: the binary writes no review yet)
#[test]
fn a_review_in_a_subfolder_of_a_repository_has_its_author() {
    let dir = repo("Ada Reviewer");
    let sub = dir.path().join("services/billing");
    std::fs::create_dir_all(&sub).unwrap();

    let review = record(&sub, &docs(), &feature_pay(), Status::Validated, "").unwrap();

    assert_eq!(review.author.as_deref(), Some("Ada Reviewer"));
}
