use super::*;
use crate::edits::{edits_path, Fields};
use crate::reviews::tests::{docs, feature_pay, refund, repo};
use crate::reviews::{reviews_path, SectionKind};

fn name(name: &str) -> Fields {
    Fields {
        name: Some(name.into()),
        ..Fields::default()
    }
}

fn validated() -> (Status, String) {
    (Status::Validated, String::new())
}

/// `docs` showing the edits saved in `root`, as `retrodoc ui` serves them.
fn shown(root: &std::path::Path) -> GeneratedDocs {
    docs().with_edits(edits::load(root).unwrap())
}

// spec: SFM-4 (internal: the fingerprint the store writes; the web form drives it through the router)
#[test]
fn a_review_saved_with_an_edit_is_not_outdated_by_it() {
    let dir = repo("A");
    let correction = Correction {
        edit: EditChange::Set(name("Payments")),
        review: Some((Status::Wrong, "misses the refunds".into())),
    };

    let saved = save(dir.path(), &docs(), &feature_pay(), &correction).unwrap();

    let review = saved.review.unwrap();
    assert_eq!(review.status, Status::Wrong);
    assert_eq!(saved.edit.unwrap().fields, name("Payments"));
    assert_eq!(reviews::outdated(&shown(dir.path()), &review), None);
}

// spec: SFM-17, SED-36 (internal: the store of the confirmation, the form drives it through the router)
#[test]
fn an_outdated_edit_confirmed_is_no_longer_outdated() {
    let dir = repo("A");
    let root = dir.path();
    let before = crate::reviews::tests::docs_with("Before.");
    edits::record(root, &before, &refund(), name("Refunds")).unwrap();
    edits::record(
        root,
        &docs().with_edits(edits::load(root).unwrap()),
        &refund(),
        name("Refunds"),
    )
    .unwrap();
    let outdated = shown(root);
    let edit = edits::load(root).unwrap().get(&refund()).unwrap().clone();
    assert!(
        edits::outdated(&outdated, &edit).is_some(),
        "kept as outdated"
    );
    let correction = Correction {
        edit: EditChange::Confirm(name("Refunds")),
        review: None,
    };

    let saved = save(root, &outdated, &refund(), &correction).unwrap();

    let confirmed = saved.edit.unwrap();
    assert_eq!(confirmed.fields, name("Refunds"));
    assert_eq!(edits::outdated(&shown(root), &confirmed), None);
}

// spec: SFM-3 (internal: an edit left as it was keeps its date and its fingerprint)
#[test]
fn a_review_alone_leaves_the_edit_as_it_was() {
    let dir = repo("A");
    edits::record(dir.path(), &docs(), &feature_pay(), name("Payments")).unwrap();
    let before = std::fs::read(edits_path(dir.path())).unwrap();
    let correction = Correction {
        edit: EditChange::Keep,
        review: Some(validated()),
    };

    let saved = save(dir.path(), &shown(dir.path()), &feature_pay(), &correction).unwrap();

    assert_eq!(std::fs::read(edits_path(dir.path())).unwrap(), before);
    assert_eq!(saved.edit.unwrap().fields, name("Payments"));
    let review = saved.review.unwrap();
    assert_eq!(reviews::outdated(&shown(dir.path()), &review), None);
}

// spec: SFM-3, SFM-5 (internal: the revert of the store, the button drives it through the router)
#[test]
fn removing_the_edit_fingerprints_the_review_on_the_generated_section() {
    let dir = repo("A");
    edits::record(dir.path(), &docs(), &feature_pay(), name("Payments")).unwrap();
    let correction = Correction {
        edit: EditChange::Remove,
        review: Some((Status::Wrong, String::new())),
    };

    let saved = save(dir.path(), &shown(dir.path()), &feature_pay(), &correction).unwrap();

    assert_eq!(saved.edit, None);
    assert_eq!(edits::load(dir.path()).unwrap().edits(), []);
    let review = saved.review.unwrap();
    assert_eq!(reviews::outdated(&shown(dir.path()), &review), None);
}

// spec: SFM-3 (internal: no status removes the review, the edit still written)
#[test]
fn no_status_removes_the_review_and_writes_the_edit() {
    let dir = repo("A");
    reviews::record(dir.path(), &docs(), &refund(), Status::Validated, "").unwrap();
    let correction = Correction {
        edit: EditChange::Set(name("Refund an order")),
        review: None,
    };

    let saved = save(dir.path(), &docs(), &refund(), &correction).unwrap();

    assert_eq!(saved.review, None);
    assert_eq!(reviews::load(dir.path()).unwrap().reviews(), []);
    assert_eq!(edits::load(dir.path()).unwrap().edits().len(), 1);
}

// spec: SFM-13 (internal: a refusal of the store before any file is written)
#[test]
fn a_narrative_of_a_feature_writes_neither_file() {
    let dir = repo("A");
    let correction = Correction {
        edit: EditChange::Set(Fields {
            narrative: Some("The clerk pays.".into()),
            ..Fields::default()
        }),
        review: Some(validated()),
    };

    let error = save(dir.path(), &docs(), &feature_pay(), &correction).unwrap_err();

    assert!(
        matches!(error, CorrectionError::Edits(EditsError::NoNarrative(_))),
        "{error}"
    );
    assert!(!edits_path(dir.path()).exists());
    assert!(!reviews_path(dir.path()).exists());
}

// spec: SFM-12 (internal: the store names the section it does not know)
#[test]
fn a_section_the_docs_lack_writes_neither_file() {
    let dir = repo("A");
    let correction = Correction {
        edit: EditChange::Set(name("Ghost")),
        review: Some(validated()),
    };

    let error = save(
        dir.path(),
        &docs(),
        &Section::new(SectionKind::Feature, "ghost"),
        &correction,
    )
    .unwrap_err();

    assert!(error.is_unknown_section(), "{error}");
    assert!(!edits_path(dir.path()).exists());
    assert!(!reviews_path(dir.path()).exists());
}

// spec: SFM-14 (internal: both files checked before either is written)
#[test]
fn an_unreadable_file_leaves_both_files_as_they_were() {
    for unreadable in [edits_path, reviews_path] {
        let dir = repo("A");
        reviews::record(dir.path(), &docs(), &refund(), Status::ToReview, "").unwrap();
        edits::record(dir.path(), &docs(), &refund(), name("Refund")).unwrap();
        std::fs::write(unreadable(dir.path()), "{ not json").unwrap();
        let files = |root: &std::path::Path| {
            [edits_path(root), reviews_path(root)].map(|path| std::fs::read(path).unwrap())
        };
        let before = files(dir.path());
        let correction = Correction {
            edit: EditChange::Set(name("Refund an order")),
            review: Some(validated()),
        };

        let error = save(dir.path(), &docs(), &refund(), &correction).unwrap_err();

        assert!(!error.is_unknown_section(), "{error}");
        assert_eq!(files(dir.path()), before);
    }
}
