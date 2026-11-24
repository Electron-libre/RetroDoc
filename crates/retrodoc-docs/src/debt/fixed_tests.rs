use std::collections::BTreeMap;

use crate::edits::{Edit, Edits, Fields};
use crate::reviews::{self, Review, Reviews, SectionKind, Status};
use crate::GeneratedDocs;

use super::super::tests::{docs, feature, use_case};
use super::super::{debt, DebtKind, DebtList};
use super::Fixed;

const MORNING: &str = "2026-11-24T09:00:00Z";
const NOON: &str = "2026-11-24T12:00:00Z";

fn section(kind: SectionKind, id: &str) -> reviews::Section {
    reviews::Section::new(kind, id)
}

fn review(docs: &GeneratedDocs, section: &reviews::Section, status: Status, at: &str) -> Review {
    Review {
        section: section.clone(),
        status,
        comment: format!("{status:?} at {at}"),
        author: Some("Ada".into()),
        reviewed_at: at.into(),
        fingerprint: reviews::fingerprint(docs, section).unwrap(),
        files: BTreeMap::new(),
    }
}

fn reviewed(list: impl IntoIterator<Item = Review>) -> Reviews {
    let mut reviews = Reviews::default();
    for review in list {
        reviews.set(review);
    }
    reviews
}

/// `docs` with the name of `section` edited by Bob `at`.
fn edited(docs: GeneratedDocs, section: &reviews::Section, at: &str) -> GeneratedDocs {
    let mut edits = Edits::default();
    edits.set(Edit {
        section: section.clone(),
        fields: Fields {
            name: Some("Corrected".into()),
            ..Fields::default()
        },
        author: Some("Bob".into()),
        edited_at: at.into(),
        fingerprint: reviews::fingerprint_in(docs.generated(), section).unwrap(),
        files: BTreeMap::new(),
    });
    docs.with_edits(edits)
}

/// `pay` (billing, 0.9) with the use cases `refund` (low, 0.3) and `ship` (0.9).
fn run() -> GeneratedDocs {
    docs(
        vec![feature("pay", "billing", Some(0.9))],
        vec![
            use_case("refund", "pay", Some(0.3)),
            use_case("ship", "pay", Some(0.9)),
        ],
    )
}

fn kinds_of(list: &DebtList, label: &str) -> Vec<DebtKind> {
    list.items
        .iter()
        .filter(|i| i.label() == label)
        .map(|i| i.kind)
        .collect()
}

fn fixed_labels(list: &DebtList) -> Vec<String> {
    list.fixed.iter().map(Fixed::label).collect()
}

// spec: FIX-1, FIX-2, FIX-4, FIX-6 (internal: the read model of the debt page, shown by FIX-H1)
#[test]
fn a_use_case_marked_wrong_leaves_the_items_left_for_fixed() {
    let docs = run();
    let refund = section(SectionKind::UseCase, "pay/refund");
    let before = debt(&docs, &Reviews::default()).items.len();

    let list = debt(
        &docs,
        &reviewed([review(&docs, &refund, Status::Wrong, MORNING)]),
    );

    assert_eq!(kinds_of(&list, "billing/pay/refund"), [DebtKind::SetApart]);
    assert_eq!(list.items.len(), before - 1);
    assert_eq!(fixed_labels(&list), ["billing/pay/refund"]);
    let fixed = &list.fixed[0];
    assert_eq!(fixed.status, Status::Wrong);
    assert_eq!(fixed.comment, "Wrong at 2026-11-24T09:00:00Z");
    assert_eq!(fixed.author.as_deref(), Some("Ada"));
    assert_eq!(fixed.reviewed_at, MORNING);
    assert_eq!(fixed.edit, None);
    assert_eq!(
        fixed.kinds,
        [DebtKind::LowConfidence, DebtKind::MarkedWrong]
    );
}

// spec: FIX-1, FIX-4 (internal: the read model of the debt page, shown by FIX-H3)
#[test]
fn a_validated_section_is_fixed_with_the_kinds_of_judgement_it_took_out() {
    let docs = run();
    let reviews = reviewed([
        review(
            &docs,
            &section(SectionKind::UseCase, "pay/refund"),
            Status::Validated,
            MORNING,
        ),
        review(
            &docs,
            &section(SectionKind::Domain, "billing"),
            Status::Validated,
            MORNING,
        ),
    ]);

    let list = debt(&docs, &reviews);

    let kinds: Vec<(String, Vec<DebtKind>)> = list
        .fixed
        .iter()
        .map(|f| (f.label(), f.kinds.clone()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("billing".to_string(), vec![]),
            (
                "billing/pay/refund".to_string(),
                vec![DebtKind::LowConfidence]
            ),
        ]
    );
}

// spec: FIX-1 (internal: the read model of the debt page)
#[test]
fn a_section_left_to_review_or_gone_from_the_run_is_not_fixed() {
    let docs = run();
    let mut gone = review(
        &docs,
        &section(SectionKind::Feature, "pay"),
        Status::Validated,
        MORNING,
    );
    gone.section = section(SectionKind::Feature, "gone");
    let reviews = reviewed([
        review(
            &docs,
            &section(SectionKind::UseCase, "pay/refund"),
            Status::ToReview,
            MORNING,
        ),
        gone,
    ]);

    let list = debt(&docs, &reviews);

    assert!(list.fixed.is_empty(), "{:?}", list.fixed);
    assert!(kinds_of(&list, "billing/pay/refund").contains(&DebtKind::ToReview));
}

// spec: FIX-1, FIX-2 (internal: the read model of the debt page, shown by FIX-H2)
#[test]
fn an_edit_after_the_review_keeps_the_section_fixed() {
    let docs = run();
    let pay = section(SectionKind::Feature, "pay");
    let wrong = review(&docs, &pay, Status::Wrong, MORNING);

    let list = debt(&edited(docs, &pay, NOON), &reviewed([wrong]));

    assert_eq!(kinds_of(&list, "billing/pay"), []);
    assert_eq!(fixed_labels(&list), ["billing/pay"]);
    assert_eq!(
        list.fixed[0].edit,
        Some((Some("Bob".to_string()), NOON.to_string()))
    );
}

// spec: FIX-1 (internal: the read model of the debt page)
#[test]
fn an_edit_in_the_second_of_the_review_keeps_it_fixed() {
    let docs = run();
    let pay = section(SectionKind::Feature, "pay");
    let wrong = review(&docs, &pay, Status::Wrong, NOON);

    let list = debt(&edited(docs, &pay, NOON), &reviewed([wrong]));

    assert_eq!(fixed_labels(&list), ["billing/pay"]);
}

// spec: FIX-5 (internal: the read model of the debt page)
#[test]
fn an_edit_before_the_review_does_not_fix_a_review_since_rewritten() {
    let docs = run();
    let pay = section(SectionKind::Feature, "pay");
    // Reviewed as generated, then edited before the review was saved: the edit is older.
    let wrong = review(&docs, &pay, Status::Wrong, NOON);

    let list = debt(&edited(docs, &pay, MORNING), &reviewed([wrong]));

    assert!(list.fixed.is_empty(), "{:?}", list.fixed);
    let item = list
        .items
        .iter()
        .find(|i| i.kind == DebtKind::MarkedWrong)
        .unwrap();
    assert!(item.outdated.as_ref().unwrap().rewritten);
}

// spec: FIX-5 (internal: the read model of the debt page)
#[test]
fn a_section_rewritten_by_a_later_run_comes_back_marked_wrong() {
    let reviewed_run = run();
    let pay = section(SectionKind::Feature, "pay");
    let wrong = review(&reviewed_run, &pay, Status::Wrong, MORNING);
    let edits = edited(reviewed_run, &pay, NOON).edits().clone();
    let mut rewritten = feature("pay", "billing", Some(0.9));
    rewritten.description = "Takes the payment.".into();
    let later_run = docs(vec![rewritten], vec![]).with_edits(edits);

    let list = debt(&later_run, &reviewed([wrong]));

    assert!(list.fixed.is_empty(), "{:?}", list.fixed);
    assert_eq!(
        kinds_of(&list, "billing/pay"),
        [DebtKind::MarkedWrong, DebtKind::OutdatedEdit]
    );
}

// spec: FIX-7 (internal: the read model of the debt page)
#[test]
fn the_fixed_sections_are_the_newest_first_then_by_kind_and_id() {
    let docs = run();
    let reviews = reviewed([
        review(
            &docs,
            &section(SectionKind::UseCase, "pay/ship"),
            Status::Validated,
            MORNING,
        ),
        review(
            &docs,
            &section(SectionKind::UseCase, "pay/refund"),
            Status::Validated,
            MORNING,
        ),
        review(
            &docs,
            &section(SectionKind::Domain, "billing"),
            Status::Validated,
            MORNING,
        ),
        review(
            &docs,
            &section(SectionKind::Feature, "pay"),
            Status::Wrong,
            NOON,
        ),
    ]);

    let list = debt(&docs, &reviews);

    assert_eq!(
        fixed_labels(&list),
        [
            "billing/pay",
            "billing",
            "billing/pay/refund",
            "billing/pay/ship"
        ]
    );
}
