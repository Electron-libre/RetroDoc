use super::*;
use crate::freshness::FileState;
use crate::reviews::tests::{docs, docs_citing, feature_pay, file_hash_of, refund, repo};
use crate::reviews::{self, Status};
use retrodoc_pipeline::Glossary;

fn name(name: &str) -> Fields {
    Fields {
        name: Some(name.into()),
        ..Fields::default()
    }
}

fn billing() -> Section {
    Section::new(SectionKind::Domain, "billing")
}

// spec: SED-1, SED-2, SED-6, SED-7 (internal: the binary writes no edit yet, the web form will)
#[test]
fn an_edit_holds_its_section_the_fields_it_changes_its_author_and_date() {
    let dir = repo("Ada Editor");

    let edit = record(dir.path(), &docs(), &feature_pay(), name("Payments")).unwrap();

    assert_eq!(edit.section, feature_pay());
    assert_eq!(edit.fields, name("Payments"));
    assert_eq!(edit.author.as_deref(), Some("Ada Editor"));
    let date = chrono::DateTime::parse_from_rfc3339(&edit.edited_at).unwrap();
    assert_eq!(date.offset().local_minus_utc(), 0);
    assert!(edit.edited_at.ends_with('Z'));
    assert!(!edit.edited_at.contains('.'));
    assert_eq!(load(dir.path()).unwrap().edits(), [edit]);
}

// spec: SED-1, SED-2 (internal: the format of the file the other crates read)
#[test]
fn a_field_left_out_is_absent_from_the_file() {
    let dir = repo("A");

    record(dir.path(), &docs(), &feature_pay(), name("Payments")).unwrap();

    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(edits_path(dir.path())).unwrap()).unwrap();
    assert_eq!(saved["version"], VERSION);
    let edit = saved["edits"][0].as_object().unwrap();
    assert_eq!(edit["kind"], "feature");
    assert_eq!(edit["id"], "pay");
    assert_eq!(edit["name"], "Payments");
    assert!(!edit.contains_key("description"));
    assert!(!edit.contains_key("narrative"));
}

// spec: SED-6 (internal: the binary writes no edit yet)
#[test]
fn an_edit_out_of_a_git_repository_has_no_author() {
    let dir = tempfile::tempdir().unwrap();

    let edit = record(dir.path(), &docs(), &feature_pay(), name("Payments")).unwrap();

    assert_eq!(edit.author, None);
}

// spec: SED-3 (internal: an error the web form will report)
#[test]
fn a_narrative_of_a_domain_or_a_feature_is_refused() {
    let dir = repo("A");
    let narrative = Fields {
        narrative: Some("The clerk pays.".into()),
        ..Fields::default()
    };

    for section in [billing(), feature_pay()] {
        let error = record(dir.path(), &docs(), &section, narrative.clone()).unwrap_err();
        assert!(matches!(error, EditsError::NoNarrative(_)), "{error}");
    }
    assert!(!edits_path(dir.path()).exists());
    let edit = record(dir.path(), &docs(), &refund(), narrative.clone()).unwrap();
    assert_eq!(edit.fields, narrative);
}

// spec: SED-4 (internal: an error the web form will report)
#[test]
fn an_edit_that_changes_no_field_is_refused() {
    let dir = repo("A");

    let error = record(dir.path(), &docs(), &refund(), Fields::default()).unwrap_err();

    assert!(matches!(error, EditsError::NothingEdited), "{error}");
    assert!(!edits_path(dir.path()).exists());
}

// spec: SED-5 (internal: an error the web form will report)
#[test]
fn a_blank_field_is_refused() {
    let dir = repo("A");
    let blank = [
        name(""),
        Fields {
            description: Some(" \n\t".into()),
            ..Fields::default()
        },
        Fields {
            name: Some("Refund".into()),
            narrative: Some("  ".into()),
            ..Fields::default()
        },
    ];

    for fields in blank {
        let error = record(dir.path(), &docs(), &refund(), fields).unwrap_err();
        assert!(matches!(error, EditsError::Blank(_)), "{error}");
    }
    assert!(!edits_path(dir.path()).exists());
}

// spec: SED-8 (internal: what `outdated` compares)
#[test]
fn an_edit_holds_the_fingerprint_of_the_generated_section() {
    let dir = repo("A");
    let docs = docs();

    let first = record(dir.path(), &docs, &feature_pay(), name("Payments")).unwrap();
    let second = record(dir.path(), &docs, &feature_pay(), name("Payment")).unwrap();

    let generated = reviews::fingerprint(&docs, &feature_pay()).unwrap();
    assert_eq!(first.fingerprint, generated);
    assert_eq!(second.fingerprint, generated);
}

// spec: SED-9 (internal: what `outdated` compares)
#[test]
fn an_edit_holds_the_hash_of_each_file_its_section_cites() {
    let dir = repo("A");
    let root = dir.path();
    let docs = docs_citing(root, "");
    std::fs::remove_file(root.join("app/refund.rb")).unwrap();

    let feature = record(root, &docs, &feature_pay(), name("Payments")).unwrap();
    let use_case = record(root, &docs, &refund(), name("Refund")).unwrap();

    assert_eq!(
        feature.files,
        BTreeMap::from([("app/pay.rb".to_string(), file_hash_of(root, "app/pay.rb"))])
    );
    assert!(use_case.files.is_empty());
}

// The path of the file and what a save leaves on disk; `generate` keeping it is tested through
// the binary.
// spec: SED-10, SED-12 (internal: the binary writes no edit yet)
#[test]
fn the_edits_are_saved_whole_in_one_file_outside_the_cache() {
    let dir = repo("A");

    record(dir.path(), &docs(), &refund(), name("Refund")).unwrap();
    record(dir.path(), &docs(), &feature_pay(), name("Payments")).unwrap();

    assert_eq!(
        edits_path(dir.path()),
        dir.path().join(".retrodoc/edits.json")
    );
    let names: Vec<String> = std::fs::read_dir(dir.path().join(".retrodoc"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["edits.json"]);
    assert_eq!(load(dir.path()).unwrap().edits().len(), 2);
}

// spec: SED-13, SED-14 (internal: an error the web form will report)
#[test]
fn an_unreadable_file_is_reported_and_kept() {
    let dir = repo("A");
    let path = edits_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ not json").unwrap();

    assert!(matches!(
        load(dir.path()),
        Err(EditsError::Store(StoreError::Unreadable { .. }))
    ));
    assert!(record(dir.path(), &docs(), &refund(), name("Refund")).is_err());
    assert!(remove(dir.path(), &refund()).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
}

// spec: SED-13, SED-14 (internal: an error the web form will report)
#[test]
fn a_file_of_a_later_format_is_reported_and_kept() {
    let dir = repo("A");
    let path = edits_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let later = format!(r#"{{"version": {}, "edits": []}}"#, VERSION + 1);
    std::fs::write(&path, &later).unwrap();

    let error = load(dir.path()).unwrap_err();
    assert!(
        matches!(error, EditsError::Store(StoreError::LaterFormat { version, .. }) if version == VERSION + 1)
    );
    assert!(error.to_string().contains(&(VERSION + 1).to_string()));
    assert!(record(dir.path(), &docs(), &refund(), name("Refund")).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), later);
}

// spec: SED-15 (internal: the binary writes no edit yet)
#[test]
fn a_new_edit_replaces_the_former_one_whole() {
    let dir = repo("A");
    record(
        dir.path(),
        &docs(),
        &refund(),
        Fields {
            name: Some("Refund".into()),
            narrative: Some("The manager refunds.".into()),
            ..Fields::default()
        },
    )
    .unwrap();
    let status = std::process::Command::new("git")
        .args(["config", "user.name", "B"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    let description = Fields {
        description: Some("Gives the money back.".into()),
        ..Fields::default()
    };

    record(dir.path(), &docs(), &refund(), description.clone()).unwrap();

    let edits = load(dir.path()).unwrap();
    assert_eq!(edits.edits().len(), 1);
    let edit = edits.get(&refund()).unwrap();
    assert_eq!(edit.fields, description);
    assert_eq!(edit.author.as_deref(), Some("B"));
}

// spec: SED-16 (internal: the binary writes no edit yet)
#[test]
fn removing_an_edit_keeps_the_others() {
    let dir = repo("A");
    record(dir.path(), &docs(), &billing(), name("Billing")).unwrap();
    record(dir.path(), &docs(), &refund(), name("Refund")).unwrap();

    remove(dir.path(), &billing()).unwrap();

    let edits = load(dir.path()).unwrap();
    assert_eq!(edits.get(&billing()), None);
    assert_eq!(edits.edits().len(), 1);
    assert_eq!(edits.get(&refund()).unwrap().fields, name("Refund"));
}

// spec: SED-17 (internal: the binary writes no edit yet)
#[test]
fn an_absent_file_is_no_edit() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(load(dir.path()).unwrap().edits(), []);
    remove(dir.path(), &refund()).unwrap();
    assert!(!edits_path(dir.path()).exists());
}

// spec: SED-18 (internal: the layout of the file)
#[test]
fn the_edits_are_saved_by_kind_then_id() {
    let dir = repo("A");
    for section in [
        Section::new(SectionKind::UseCase, "pay/refund"),
        Section::new(SectionKind::Feature, "pay"),
        Section::new(SectionKind::Domain, "billing"),
        Section::new(SectionKind::Feature, "a"),
        Section::new(SectionKind::Domain, "a"),
    ] {
        record(dir.path(), &docs(), &section, name("N")).unwrap();
    }

    // As written on disk: `load` sorts too.
    let text = std::fs::read_to_string(edits_path(dir.path())).unwrap();
    // The keys in the order of the structs, whatever the features of `serde_json`.
    let at = |key: &str| text.find(&format!("\"{key}\"")).unwrap();
    let keys = [
        "version",
        "edits",
        "kind",
        "id",
        "name",
        "author",
        "edited_at",
        "fingerprint",
    ];
    assert!(
        keys.windows(2).all(|pair| at(pair[0]) < at(pair[1])),
        "{text}"
    );
    let saved: serde_json::Value = serde_json::from_str(&text).unwrap();
    let order: Vec<String> = saved["edits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            format!(
                "{} {}",
                e["kind"].as_str().unwrap(),
                e["id"].as_str().unwrap()
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

// spec: SED-19 (internal: the binary writes no edit yet)
#[test]
fn an_edit_of_a_section_the_docs_lack_is_refused() {
    let dir = repo("A");

    let error = record(
        dir.path(),
        &docs(),
        &Section::new(SectionKind::Feature, "ship"),
        name("Shipping"),
    )
    .unwrap_err();

    assert!(matches!(error, EditsError::UnknownSection(_)));
    assert_eq!(error.to_string(), "the docs hold no feature ship");
    assert!(!edits_path(dir.path()).exists());
}

// spec: SED-20 (internal: a file merged by hand)
#[test]
fn several_edits_of_one_section_load_as_the_newest() {
    let dir = tempfile::tempdir().unwrap();
    let path = edits_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let edit = |name: &str, at: &str| {
        format!(
            r#"{{"kind": "feature", "id": "pay", "name": "{name}", "author": null, "edited_at": "{at}", "fingerprint": "f"}}"#
        )
    };
    let older = edit("Old", "2026-11-04T09:00:00Z");
    let newest = edit("New", "2026-11-05T09:00:00Z");
    std::fs::write(
        &path,
        format!(r#"{{"version": 1, "edits": [{older}, {newest}]}}"#),
    )
    .unwrap();

    let edits = load(dir.path()).unwrap();

    assert_eq!(edits.edits().len(), 1);
    assert_eq!(edits.get(&feature_pay()).unwrap().fields, name("New"));
}

// spec: SED-21 (internal: the requests of the web server run on threads of one process)
#[test]
fn edits_saved_at_the_same_time_by_one_process_are_all_kept() {
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
                scope.spawn(|| record(dir.path(), &docs, section, name("N")).unwrap());
            }
        });
        assert_eq!(load(dir.path()).unwrap().edits().len(), sections.len());
        for section in &sections {
            remove(dir.path(), section).unwrap();
        }
    }
}

// spec: SED-22 (internal: the two files of human work side by side)
#[test]
fn an_edit_and_a_review_leave_the_file_of_the_other_as_it_was() {
    let dir = repo("A");
    let root = dir.path();
    reviews::record(root, &docs(), &refund(), Status::Validated, "").unwrap();
    let reviewed = std::fs::read(reviews::reviews_path(root)).unwrap();

    record(root, &docs(), &refund(), name("Refund")).unwrap();
    remove(root, &feature_pay()).unwrap();
    record(root, &docs(), &feature_pay(), name("Payments")).unwrap();
    assert_eq!(
        std::fs::read(reviews::reviews_path(root)).unwrap(),
        reviewed
    );

    let edited = std::fs::read(edits_path(root)).unwrap();
    reviews::record(root, &docs(), &billing(), Status::Wrong, "").unwrap();
    reviews::remove(root, &refund()).unwrap();
    assert_eq!(std::fs::read(edits_path(root)).unwrap(), edited);
}

// spec: SED-23 (internal: the overlay every reader shares, the pages and the answers test their own output)
#[test]
fn an_edit_replaces_the_fields_it_gives_and_keeps_the_others_as_generated() {
    let dir = repo("A");
    let root = dir.path();
    let narrative = Fields {
        narrative: Some("The manager refunds the order.".into()),
        ..Fields::default()
    };
    record(root, &docs(), &refund(), narrative).unwrap();
    record(root, &docs(), &billing(), name("Invoicing")).unwrap();

    let shown = docs().with_edits(load(root).unwrap());

    let use_case = shown
        .use_cases()
        .iter()
        .find(|u| u.slug == "refund")
        .unwrap();
    assert_eq!(
        use_case.narrative.as_deref(),
        Some("The manager refunds the order.")
    );
    assert_eq!(use_case.name, "REFUND", "the name left out stays generated");
    let domain = shown
        .domains()
        .iter()
        .find(|d| d.slug == "billing")
        .unwrap();
    assert_eq!(domain.name, "Invoicing");
    let generated = &shown.generated().use_cases[0];
    assert_eq!(
        generated.narrative.as_deref(),
        Some("The clerk refunds the order."),
        "the generated text kept beneath"
    );
}

// spec: SED-24 (internal: the overlay every reader shares)
#[test]
fn an_edit_of_a_section_the_docs_no_longer_hold_is_left_out_and_kept() {
    let dir = repo("A");
    let root = dir.path();
    record(root, &docs(), &feature_pay(), name("Payments")).unwrap();
    let edits = load(root).unwrap();

    let without_pay =
        GeneratedDocs::new(Vec::new(), Vec::new(), Vec::new(), Glossary::default(), &[])
            .with_edits(edits.clone());

    assert_eq!(without_pay.features().len(), 0);
    assert_eq!(without_pay.edits(), &edits);
    assert_eq!(load(root).unwrap(), edits);
}

// spec: SED-25 (internal: the binary writes no edit yet, the web form will)
#[test]
fn an_edit_recorded_from_edited_docs_fingerprints_the_section_as_generated() {
    let dir = repo("A");
    let root = dir.path();
    let first = record(root, &docs(), &feature_pay(), name("Payments")).unwrap();
    let edited = docs().with_edits(load(root).unwrap());
    assert_eq!(edited.features()[0].name, "Payments");

    let description = Fields {
        description: Some("Settle what is due.".into()),
        ..Fields::default()
    };
    let second = record(root, &edited, &feature_pay(), description).unwrap();

    assert_eq!(second.fingerprint, first.fingerprint);
}

// spec: SED-26 (internal: the binary writes no edit yet, the web form will)
#[test]
fn an_edit_made_after_a_review_makes_it_outdated() {
    let dir = repo("A");
    let root = dir.path();
    let review = reviews::record(root, &docs(), &feature_pay(), Status::Validated, "").unwrap();
    assert!(reviews::outdated(&docs(), &review).is_none());

    record(root, &docs(), &feature_pay(), name("Payments")).unwrap();
    let edited = docs().with_edits(load(root).unwrap());

    let outdated = reviews::outdated(&edited, &review).unwrap();
    assert!(outdated.rewritten);
}

// spec: SED-34, SED-37 (internal: the rule the pages, the debt and the MCP answers read)
#[test]
fn an_edit_of_a_section_rewritten_since_is_outdated_and_still_shown() {
    let dir = repo("A");
    let root = dir.path();
    let edit = record(
        root,
        &docs_citing(root, "Before."),
        &refund(),
        name("Refund"),
    )
    .unwrap();
    let shown = |narrative| docs_citing(root, narrative).with_edits(load(root).unwrap());

    assert_eq!(outdated(&shown("Before."), &edit), None);
    let rewritten = shown("After.");
    assert_eq!(
        outdated(&rewritten, &edit),
        Some(Outdated {
            rewritten: true,
            changed: Vec::new(),
        })
    );
    assert_eq!(rewritten.use_cases()[0].name, "Refund");
    assert_eq!(load(root).unwrap().get(&refund()), Some(&edit));
}

// spec: SED-34 (internal: the rule the pages, the debt and the MCP answers read)
#[test]
fn an_edit_followed_by_another_field_is_not_outdated() {
    let dir = repo("A");
    let root = dir.path();
    record(root, &docs(), &feature_pay(), name("Payments")).unwrap();
    let description = Fields {
        description: Some("Settle what is due.".into()),
        ..Fields::default()
    };
    let edited = docs().with_edits(load(root).unwrap());
    let edit = record(root, &edited, &feature_pay(), description).unwrap();

    let shown = docs().with_edits(load(root).unwrap());

    assert_eq!(outdated(&shown, &edit), None);
}

// spec: SED-35 (internal: the rule the pages, the debt and the MCP answers read)
#[test]
fn an_edit_whose_cited_file_was_deleted_is_outdated_naming_it() {
    let dir = repo("A");
    let root = dir.path();
    let docs = docs_citing(root, "");
    let edit = record(root, &docs, &refund(), name("Refund")).unwrap();

    std::fs::remove_file(root.join("app/refund.rb")).unwrap();

    assert_eq!(
        outdated(&docs, &edit),
        Some(Outdated {
            rewritten: false,
            changed: vec![("app/refund.rb".into(), FileState::Deleted)],
        })
    );
}

// spec: SED-35 (internal: a file written by hand)
#[test]
fn an_edit_with_no_file_hashes_judges_no_file() {
    let dir = repo("A");
    let root = dir.path();
    let docs = docs_citing(root, "");
    let mut edit = record(root, &docs, &feature_pay(), name("Payments")).unwrap();
    edit.files.clear();

    std::fs::write(root.join("app/pay.rb"), "edited").unwrap();

    assert_eq!(outdated(&docs, &edit), None);
}

// spec: SED-36 (internal: the rule the pages, the debt and the MCP answers read)
#[test]
fn an_outdated_edit_saved_again_is_no_longer_outdated() {
    let dir = repo("A");
    let root = dir.path();
    let edit = record(
        root,
        &docs_citing(root, "Before."),
        &refund(),
        name("Refund"),
    )
    .unwrap();
    let rewritten = docs_citing(root, "After.").with_edits(load(root).unwrap());
    assert!(outdated(&rewritten, &edit).is_some());

    let again = record(root, &rewritten, &refund(), edit.fields.clone()).unwrap();

    let shown = docs_citing(root, "After.").with_edits(load(root).unwrap());
    assert_eq!(outdated(&shown, &again), None);
}
