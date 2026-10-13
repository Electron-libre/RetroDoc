use std::path::{Path, PathBuf};

use super::groups::{contexts_of, mechanical_groups, name_words};
use super::*;
use crate::brief::ProductBrief;
use crate::testing::FakeLlm;

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

/// A small Rails-like application: three entry point files, their models and services, a folder
/// of workflows and the migrations, which no name reaches.
fn sources() -> Vec<PathBuf> {
    paths(&[
        "app/controllers/contracts_controller.rb",
        "app/controllers/company_contracts_controller.rb",
        "app/controllers/api/base_controller.rb",
        "app/models/contract.rb",
        "app/models/company_contract.rb",
        "app/services/contracts/send.rb",
        "app/workflows/rules/approval.rb",
        "db/migrate/20240101_create_things.rb",
    ])
}

fn seeds() -> Vec<PathBuf> {
    paths(&[
        "app/controllers/contracts_controller.rb",
        "app/controllers/company_contracts_controller.rb",
        "app/controllers/api/base_controller.rb",
    ])
}

/// `g1` = contract (3 files), `g2` = `company_contract` (2), `g3` = base (1).
const ANSWER: &str = r#"{"contexts":[{"slug":"Contracts","name":"Contracts","description":"d",
    "groups":["g1","g2"],"folders":["app/workflows/"]}],
    "support":{"groups":["g3"],"folders":["db/migrate/"]}}"#;

async fn build(dir: &Path, llm: &FakeLlm, force: bool) -> ContextMap {
    build_context_map(
        dir,
        &sources(),
        &seeds(),
        &ProductBrief::default(),
        llm,
        force,
    )
    .await
    .unwrap()
}

#[test]
fn a_name_loses_the_words_of_its_directories() {
    assert_eq!(
        name_words(Path::new("app/controllers/contracts_controller.rb")),
        ["contract"]
    );
    assert_eq!(
        name_words(Path::new("app/policies/CompanyContractPolicy.rb")),
        ["company", "contract"]
    );
    assert_eq!(
        name_words(Path::new("bookmarks/bookmarks.py")),
        ["bookmark"]
    );
}

#[test]
fn a_generic_stem_takes_the_name_of_its_folder() {
    assert_eq!(
        name_words(Path::new("src/contracts/index.ts")),
        ["contract"]
    );
    assert_eq!(name_words(Path::new("src/users/index.ts")), ["user"]);
    assert_eq!(name_words(Path::new("bookmarks/urls.py")), ["bookmark"]);
    assert_eq!(name_words(Path::new("src/billing/mod.rs")), ["billing"]);
    // Nothing to take: the stem stays.
    assert_eq!(name_words(Path::new("main.go")), ["main"]);
}

#[test]
fn short_plurals_and_caches_meet_their_singular() {
    for (plural, singular) in [
        ("jobs_controller.rb", "job_controller.rb"),
        ("tags.rb", "tag.rb"),
        ("caches.rb", "cache.rb"),
        ("matches.rb", "match.rb"),
    ] {
        assert_eq!(
            name_words(Path::new(plural)),
            name_words(Path::new(singular)),
            "{plural}"
        );
    }
}

#[test]
fn plurals_meet_their_singular() {
    for (plural, singular) in [
        ("statuses_controller.rb", "status_controller.rb"),
        ("addresses.rb", "address.rb"),
        ("boxes.rb", "box.rb"),
        ("branches.rb", "branch.rb"),
        ("companies.rb", "company.rb"),
    ] {
        assert_eq!(
            name_words(Path::new(plural)),
            name_words(Path::new(singular)),
            "{plural}"
        );
    }
}

#[test]
fn a_file_joins_the_longest_group_its_name_or_directory_gives() {
    let groups: std::collections::BTreeSet<Vec<String>> = [
        vec!["contract".to_string()],
        vec!["company".to_string(), "contract".to_string()],
        vec!["folder".to_string()],
    ]
    .into();
    let of = |path: &str| contexts_of(Path::new(path), &groups);
    assert_eq!(of("app/models/contract_signer.rb"), [["contract"]]);
    assert_eq!(
        of("app/policies/company_contract_policy.rb"),
        [["company", "contract"]]
    );
    assert_eq!(of("app/services/folders/duplicate.rb"), [["folder"]]);
    assert_eq!(of("lib/pdf/renderer.rb"), Vec::<Vec<String>>::new());
}

#[test]
fn each_source_file_is_in_one_group_or_is_an_orphan() {
    let groups = mechanical_groups(&sources(), &seeds());

    assert_eq!(groups.groups["contract"].len(), 3);
    assert_eq!(groups.groups["company_contract"].len(), 2);
    assert_eq!(groups.groups["base"].len(), 1);
    assert_eq!(
        groups.orphans,
        paths(&[
            "app/workflows/rules/approval.rb",
            "db/migrate/20240101_create_things.rb"
        ])
    );
    let placed: usize = groups
        .groups
        .values()
        .map(std::collections::BTreeSet::len)
        .sum();
    assert_eq!(placed + groups.orphans.len(), sources().len());
}

#[tokio::test]
async fn the_map_merges_groups_names_the_support_and_places_the_folders() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(ANSWER);

    let map = build(dir.path(), &llm, false).await;

    assert_eq!(llm.calls(), 1);
    assert_eq!(map.contexts.len(), 1);
    assert_eq!(map.contexts[0].slug, "contracts");
    let resolved = map.resolve(&sources(), &seeds());
    assert_eq!(resolved.contexts.len(), 1);
    let contracts = &resolved.contexts[0];
    assert_eq!(contracts.files.len(), 6, "two groups and the workflows");
    assert_eq!(contracts.entry_files.len(), 2);
    assert_eq!(
        resolved.support,
        paths(&[
            "app/controllers/api/base_controller.rb",
            "db/migrate/20240101_create_things.rb"
        ])
    );
}

#[tokio::test]
async fn a_group_the_answer_left_out_is_a_context_and_a_double_citation_keeps_the_first() {
    let dir = tempfile::tempdir().unwrap();
    // g2 is cited twice, g3 and the folders not at all, g9 does not exist.
    let llm = FakeLlm::answering(
        r#"{"contexts":[{"slug":"a","name":"A","description":"d","groups":["g1","g2","g9"]},
            {"slug":"b","name":"B","description":"d","groups":["g2"]}],"support":{}}"#,
    );

    let map = build(dir.path(), &llm, false).await;

    let slugs: Vec<&str> = map.contexts.iter().map(|c| c.slug.as_str()).collect();
    assert_eq!(slugs, ["a", "base"], "`b` held nothing once g2 went to `a`");
    assert_eq!(map.contexts[0].groups, ["contract", "company_contract"]);
    let resolved = map.resolve(&sources(), &seeds());
    let covered: usize = resolved.contexts.iter().map(|c| c.files.len()).sum();
    assert_eq!(
        covered + resolved.support.len(),
        sources().len(),
        "every file is in a context or in the support"
    );
}

#[tokio::test]
async fn the_second_run_reads_the_saved_map_and_a_hand_edit_is_followed() {
    let dir = tempfile::tempdir().unwrap();
    build(dir.path(), &FakeLlm::answering(ANSWER), false).await;

    let silent = FakeLlm::answering("not called");
    let again = build(dir.path(), &silent, false).await;
    assert_eq!(silent.calls(), 0);
    assert_eq!(again.contexts.len(), 1);

    // The user moves the company contracts into a context of their own.
    let mut edited = again.clone();
    edited.contexts[0].groups = vec!["contract".to_string()];
    edited.contexts.push(BoundedContext {
        slug: "company-contracts".to_string(),
        name: "Company contracts".to_string(),
        description: String::new(),
        groups: vec!["company_contract".to_string()],
        folders: Vec::new(),
    });
    edited.save(dir.path()).unwrap();

    let after = build(dir.path(), &silent, false).await;
    assert_eq!(silent.calls(), 0, "an edit is kept, not asked again");
    let resolved = after.resolve(&sources(), &seeds());
    let sizes: Vec<usize> = resolved.contexts.iter().map(|c| c.files.len()).collect();
    assert_eq!(sizes, [4, 2], "contract + workflows; company contract");
}

#[tokio::test]
async fn force_asks_again() {
    let dir = tempfile::tempdir().unwrap();
    build(dir.path(), &FakeLlm::answering(ANSWER), false).await;

    let again = FakeLlm::answering(ANSWER);
    build(dir.path(), &again, true).await;

    assert_eq!(again.calls(), 1);
}

#[tokio::test]
async fn a_file_added_later_joins_a_context_by_its_name_or_waits_in_the_support() {
    let dir = tempfile::tempdir().unwrap();
    let map = build(dir.path(), &FakeLlm::answering(ANSWER), false).await;

    let mut grown = sources();
    grown.push(PathBuf::from("app/policies/company_contract_policy.rb"));
    grown.push(PathBuf::from("lib/pdf/renderer.rb"));
    let resolved = map.resolve(&grown, &seeds());

    assert_eq!(resolved.contexts[0].files.len(), 7);
    assert!(resolved
        .support
        .contains(&PathBuf::from("lib/pdf/renderer.rb")));
}

#[tokio::test]
async fn an_unusable_answer_gives_one_context_per_group_and_is_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering("sorry, no");

    let map = build(dir.path(), &llm, false).await;

    assert_eq!(map.contexts.len(), 3);
    assert!(
        ContextMap::load(dir.path()).is_none(),
        "asked again next run"
    );
}

#[tokio::test]
async fn without_an_entry_point_file_the_whole_application_is_one_context_and_nothing_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(ANSWER);

    let map = build_context_map(
        dir.path(),
        &sources(),
        &[],
        &ProductBrief::default(),
        &llm,
        false,
    )
    .await
    .unwrap();

    assert_eq!(llm.calls(), 0);
    assert_eq!(map.contexts.len(), 1);
    let resolved = map.resolve(&sources(), &[]);
    assert_eq!(resolved.contexts[0].files.len(), sources().len());
    assert_eq!(resolved.support, Vec::<PathBuf>::new());
    assert_eq!(
        ContextMap::load(dir.path()),
        Some(map),
        "saved, and editable"
    );
}

#[tokio::test]
async fn without_any_source_file_there_is_no_context() {
    let dir = tempfile::tempdir().unwrap();

    let map = build_context_map(
        dir.path(),
        &[],
        &[],
        &ProductBrief::default(),
        &FakeLlm::answering(ANSWER),
        false,
    )
    .await
    .unwrap();

    assert_eq!(map.contexts, Vec::<BoundedContext>::new());
}

#[tokio::test]
async fn the_prompt_lists_the_groups_with_ids_and_the_folders_of_the_orphans() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(ANSWER);

    build(dir.path(), &llm, false).await;

    let prompt = &llm.prompts()[0];
    assert!(prompt.contains("g1 | 3 | contract"), "{prompt}");
    assert!(prompt.contains("g3 | 1 | base"), "{prompt}");
    assert!(prompt.contains("app/workflows/ | 1"), "{prompt}");
    assert!(prompt.contains("db/migrate/ | 1"), "{prompt}");
}

#[tokio::test]
async fn a_layer_folder_with_many_orphans_is_listed_by_sub_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut many = sources();
    many.extend((0..45).map(|i| PathBuf::from(format!("app/models/actions/step_{i}.rb"))));
    many.extend(paths(&["app/models/zeta.rb", "app/models/eta.rb"]));
    let llm = FakeLlm::answering(ANSWER);

    build_context_map(
        dir.path(),
        &many,
        &seeds(),
        &ProductBrief::default(),
        &llm,
        false,
    )
    .await
    .unwrap();

    let prompt = &llm.prompts()[0];
    assert!(prompt.contains("app/models/actions/ | 45"), "{prompt}");
    assert!(prompt.contains("app/models/ | 2"), "{prompt}");
}

#[test]
fn an_entry_point_file_keeps_the_group_of_its_own_name_and_a_hand_written_folder_is_normalised() {
    let map = ContextMap {
        contexts: vec![
            BoundedContext {
                slug: "report".to_string(),
                name: "Report".to_string(),
                groups: vec!["report".to_string()],
                folders: vec!["./app/workflows".to_string()],
                ..BoundedContext::default()
            },
            BoundedContext {
                slug: "x-y".to_string(),
                name: "X Y".to_string(),
                groups: vec!["x_y".to_string()],
                ..BoundedContext::default()
            },
        ],
        support: Support::default(),
    };
    let seed = PathBuf::from("app/x_y/report.rb");
    let sources = vec![seed.clone(), PathBuf::from("app/workflows/run.rb")];

    let resolved = map.resolve(&sources, std::slice::from_ref(&seed));

    assert_eq!(
        resolved.contexts[0].files,
        paths(&["app/workflows/run.rb", "app/x_y/report.rb"]),
        "the seed by its own name, the workflow by the folder written with `./`"
    );
    assert_eq!(resolved.contexts[1].files, Vec::<PathBuf>::new());
    assert_eq!(resolved.support, Vec::<PathBuf>::new());
}

#[tokio::test]
async fn a_context_without_slug_and_name_is_named_after_its_first_group() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(r#"{"contexts":[{"groups":["g1"]},{"groups":["g2"]}]}"#);

    let map = build(dir.path(), &llm, false).await;

    let slugs: Vec<&str> = map.contexts.iter().map(|c| c.slug.as_str()).collect();
    assert_eq!(slugs, ["contract", "company-contract", "base"]);
}

/// The small application plus 160 task files and 5 rule files in `app/workflows`, which the first
/// answer gives to the `contracts` context: 171 files, over the limit of a context.
fn big_sources() -> Vec<PathBuf> {
    let mut many = sources();
    many.extend((0..160).map(|i| PathBuf::from(format!("app/workflows/tasks/task_{i}.rb"))));
    many.extend((0..5).map(|i| PathBuf::from(format!("app/workflows/rules/rule_{i}.rb"))));
    many
}

const SPLIT: &str = r#"{"contexts":[
    {"slug":"approvals","name":"Approvals","description":"d","groups":["g1"],"folders":["app/workflows/rules/"]},
    {"slug":"tasks","name":"Tasks","description":"d","groups":["g2"],"folders":["app/workflows/tasks/"]}]}"#;

/// The first call maps, the second splits the big context.
fn mapping_then_splitting() -> FakeLlm {
    FakeLlm::replying(|n, _| Ok(if n == 0 { ANSWER } else { SPLIT }.to_string()))
}

async fn build_big(dir: &Path, llm: &FakeLlm) -> ContextMap {
    build_context_map(
        dir,
        &big_sources(),
        &seeds(),
        &ProductBrief::default(),
        llm,
        false,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn a_context_over_the_limit_is_split_by_one_more_call_and_no_file_is_lost() {
    let dir = tempfile::tempdir().unwrap();
    let llm = mapping_then_splitting();

    let map = build_big(dir.path(), &llm).await;

    assert_eq!(llm.calls(), 2, "the map, then the split of `contracts`");
    let slugs: Vec<&str> = map.contexts.iter().map(|c| c.slug.as_str()).collect();
    assert_eq!(slugs, ["approvals", "tasks"]);
    let resolved = map.resolve(&big_sources(), &seeds());
    let sizes: Vec<usize> = resolved.contexts.iter().map(|c| c.files.len()).collect();
    // approvals: contract group (3) + the rules (5 + approval.rb); tasks: company_contract (2) + 160.
    assert_eq!(sizes, [9, 162]);
    assert_eq!(
        resolved.support.len(),
        2,
        "the same support as before the split: the base controller and the migration"
    );
    assert_eq!(ContextMap::load(dir.path()), Some(map), "saved split");
}

#[tokio::test]
async fn the_split_prompt_names_the_context_and_lists_its_groups_and_sub_folders() {
    let dir = tempfile::tempdir().unwrap();
    let llm = mapping_then_splitting();

    build_big(dir.path(), &llm).await;

    let prompt = &llm.prompts()[1];
    assert!(prompt.contains("Contracts"), "{prompt}");
    assert!(prompt.contains("g1 | 3 | contract"), "{prompt}");
    assert!(prompt.contains("app/workflows/tasks/ | 160"), "{prompt}");
    assert!(prompt.contains("app/workflows/rules/ | 6"), "{prompt}");
}

#[tokio::test]
async fn what_the_split_forgets_stays_with_the_first_part_and_an_unusable_split_keeps_the_context()
{
    let dir = tempfile::tempdir().unwrap();
    // The split cites g1 with the rules folder, and the tasks folder alone: g2 is forgotten.
    let partial = FakeLlm::replying(|n, _| {
        Ok(if n == 0 {
            ANSWER.to_string()
        } else {
            r#"{"contexts":[{"slug":"a","name":"A","groups":["g1"],"folders":["app/workflows/rules/"]},
                {"slug":"b","name":"B","groups":[],"folders":["app/workflows/tasks/"]}]}"#
                .to_string()
        })
    });
    let map = build_big(dir.path(), &partial).await;
    let slugs: Vec<&str> = map.contexts.iter().map(|c| c.slug.as_str()).collect();
    assert_eq!(slugs, ["a", "b"], "the forgotten group is not a context");
    assert!(map.contexts[0]
        .groups
        .contains(&"company_contract".to_string()));
    let resolved = map.resolve(&big_sources(), &seeds());
    let covered: usize = resolved.contexts.iter().map(|c| c.files.len()).sum();
    assert_eq!(covered, 171, "{map:?}");

    let dir = tempfile::tempdir().unwrap();
    let unusable = FakeLlm::replying(|n, _| {
        if n == 0 {
            Ok(ANSWER.to_string())
        } else {
            Ok("not json".to_string())
        }
    });
    let kept = build_big(dir.path(), &unusable).await;
    let slugs: Vec<&str> = kept.contexts.iter().map(|c| c.slug.as_str()).collect();
    assert_eq!(
        slugs,
        ["contracts"],
        "an unusable split leaves the context whole"
    );
}

#[tokio::test]
async fn a_failed_split_call_is_an_error_and_the_map_is_saved_without_the_split() {
    let dir = tempfile::tempdir().unwrap();
    let down = FakeLlm::replying(|n, _| {
        if n == 0 {
            Ok(ANSWER.to_string())
        } else {
            Err(retrodoc_llm::LlmError::Transport("down".to_string()))
        }
    });

    let result = build_context_map(
        dir.path(),
        &big_sources(),
        &seeds(),
        &ProductBrief::default(),
        &down,
        false,
    )
    .await;

    assert!(result.is_err());
    assert_eq!(ContextMap::load(dir.path()).unwrap().contexts.len(), 1);
}

#[tokio::test]
async fn a_part_does_not_take_the_slug_of_a_context_left_whole() {
    let dir = tempfile::tempdir().unwrap();
    // The split calls a part `base`, which the unsplit `base` context already is.
    let llm = FakeLlm::replying(|n, _| {
        Ok(match n {
            0 => r#"{"contexts":[{"slug":"contracts","name":"C","groups":["g1","g2"],"folders":["app/workflows/"]},
                {"slug":"base","name":"Base","groups":["g3"]}]}"#
                .to_string(),
            _ => r#"{"contexts":[{"slug":"base","name":"B2","groups":["g1"],"folders":["app/workflows/rules/"]},
                {"slug":"tasks","name":"T","groups":["g2"],"folders":["app/workflows/tasks/"]}]}"#
                .to_string(),
        })
    });

    let map = build_big(dir.path(), &llm).await;

    let slugs: Vec<&str> = map.contexts.iter().map(|c| c.slug.as_str()).collect();
    assert_eq!(slugs, ["base-2", "tasks", "base"]);
}

#[tokio::test]
async fn a_context_the_answer_calls_support_is_the_support() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(
        r#"{"contexts":[{"slug":"contracts","name":"C","groups":["g1","g2"]},
            {"slug":"Support","name":"Support","groups":["g3"],"folders":["db/migrate/"]}]}"#,
    );

    let map = build(dir.path(), &llm, false).await;

    let slugs: Vec<&str> = map.contexts.iter().map(|c| c.slug.as_str()).collect();
    assert_eq!(slugs, ["contracts"]);
    assert_eq!(map.support.groups, ["base"]);
    assert_eq!(map.support.folders, ["db/migrate/"]);
}

#[tokio::test]
async fn a_saved_split_is_reused_without_a_call() {
    let dir = tempfile::tempdir().unwrap();
    build_big(dir.path(), &mapping_then_splitting()).await;

    let silent = FakeLlm::answering("not called");
    let again = build_big(dir.path(), &silent).await;

    assert_eq!(silent.calls(), 0);
    assert_eq!(again.contexts.len(), 2);
}

#[test]
fn the_features_follow_the_order_of_the_map_and_a_removed_context_leaves_none() {
    let feature = |slug: &str, domain: &str| retrodoc_core::model::Feature {
        slug: slug.to_string(),
        domain_slug: domain.to_string(),
        sub_domain_slug: None,
        name: slug.to_string(),
        description: String::new(),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: None,
    };
    let map = ContextMap {
        contexts: vec![
            BoundedContext {
                slug: "a".to_string(),
                ..BoundedContext::default()
            },
            BoundedContext {
                slug: "b".to_string(),
                ..BoundedContext::default()
            },
        ],
        support: Support::default(),
    };
    // `b` was documented first, `gone` is no longer in the map.
    let mut features = vec![
        feature("b1", "b"),
        feature("gone1", "gone"),
        feature("b2", "b"),
        feature("a1", "a"),
    ];

    map.arrange(&mut features);

    let slugs: Vec<&str> = features.iter().map(|f| f.slug.as_str()).collect();
    assert_eq!(slugs, ["a1", "b1", "b2"]);
}
