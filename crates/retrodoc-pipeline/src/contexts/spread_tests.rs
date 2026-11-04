//! The folders of a layer context go where their files' names are written (`specs/context-map.md`).

use std::path::{Path, PathBuf};

use super::*;
use crate::brief::ProductBrief;
use crate::roles::{FileRole, RoleMap};
use crate::testing::FakeLlm;

/// Three groups, `card` (`g1`, 3 files), `billing` (`g2`, 2) and `user` (`g3`, 2), and a folder of
/// UI components no name reaches. Each file is written with its `text`, empty when not given.
fn write_tree(dir: &Path, texts: &[(&str, &str)]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let sources = [
        "app/controllers/cards_controller.rb",
        "app/models/card.rb",
        "app/policies/card_policy.rb",
        "app/controllers/billings_controller.rb",
        "app/models/billing.rb",
        "app/controllers/users_controller.rb",
        "app/models/user.rb",
        "app/components/inputs/button_component.rb",
        "app/components/inputs/input_component.rb",
        "app/components/inputs/select_component.rb",
        "app/components/inputs/text_component.rb",
    ];
    for file in sources {
        let text = texts
            .iter()
            .find(|(f, _)| *f == file)
            .map_or("", |(_, t)| t);
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let seeds = sources
        .iter()
        .filter(|f| f.contains("controllers"))
        .map(PathBuf::from)
        .collect();
    (sources.iter().map(PathBuf::from).collect(), seeds)
}

/// The roles of the tree: the components are views when `views`, else the files have no role.
fn roles(sources: &[PathBuf], views: bool) -> RoleMap {
    let mut roles = RoleMap::default();
    for file in sources
        .iter()
        .filter(|f| views && f.starts_with("app/components"))
    {
        roles.roles.insert(file.clone(), FileRole::View);
    }
    roles
}

async fn map_of(
    dir: &Path,
    (sources, seeds): (&[PathBuf], &[PathBuf]),
    roles: &RoleMap,
    answer: &str,
) -> ContextMap {
    build_context_map(
        dir,
        sources,
        seeds,
        roles,
        &ProductBrief::default(),
        &FakeLlm::answering(answer),
        false,
    )
    .await
    .unwrap()
}

fn slugs(map: &ContextMap) -> Vec<&str> {
    map.contexts.iter().map(|c| c.slug.as_str()).collect()
}

// spec: CTX-5 (internal: the CLI test of the map would need an LLM server)
#[tokio::test]
async fn a_layer_folder_goes_to_the_context_that_writes_its_names_the_most() {
    let dir = tempfile::tempdir().unwrap();
    let (sources, seeds) = write_tree(
        dir.path(),
        &[
            (
                "app/controllers/billings_controller.rb",
                "render ButtonComponent.new\nrender InputComponent.new",
            ),
            ("app/models/billing.rb", "SelectComponent\nTextComponent"),
            ("app/controllers/users_controller.rb", "TextComponent"),
        ],
    );
    let answer = r#"{"contexts":[
        {"slug":"ui","name":"UI","groups":["g1"],"folders":["app/components/inputs/"]},
        {"slug":"billing","name":"Billing","groups":["g2"]},
        {"slug":"users","name":"Users","groups":["g3"]}]}"#;

    let map = map_of(
        dir.path(),
        (&sources, &seeds),
        &roles(&sources, true),
        answer,
    )
    .await;

    assert_eq!(slugs(&map), ["ui", "billing", "users"]);
    let resolved = map.resolve(&sources, &seeds);
    assert_eq!(resolved.contexts[0].files.len(), 3, "the group files stay");
    assert!(resolved.contexts[1]
        .files
        .contains(&PathBuf::from("app/components/inputs/text_component.rb")));
}

// spec: CTX-5 (internal: the CLI test of the map would need an LLM server)
#[tokio::test]
async fn a_layer_context_whose_names_no_one_writes_goes_to_the_support() {
    let dir = tempfile::tempdir().unwrap();
    let (sources, seeds) = write_tree(dir.path(), &[]);
    let answer = r#"{"contexts":[
        {"slug":"components","name":"Components","folders":["app/components/inputs/"]},
        {"slug":"billing","name":"Billing","groups":["g1","g2","g3"]}]}"#;

    let map = map_of(
        dir.path(),
        (&sources, &seeds),
        &roles(&sources, true),
        answer,
    )
    .await;

    assert_eq!(slugs(&map), ["billing"]);
    let resolved = map.resolve(&sources, &seeds);
    assert!(resolved
        .support
        .iter()
        .filter(|f| f.starts_with("app/components"))
        .eq(sources.iter().filter(|f| f.starts_with("app/components"))));
}

// spec: CTX-5 (internal: the CLI test of the map would need an LLM server)
#[tokio::test]
async fn a_layer_file_no_one_writes_stays_unless_a_view_or_a_model() {
    let dir = tempfile::tempdir().unwrap();
    let (sources, seeds) = write_tree(dir.path(), &[]);
    let answer = r#"{"contexts":[
        {"slug":"components","name":"Components","folders":["app/components/inputs/"]},
        {"slug":"billing","name":"Billing","groups":["g1","g2","g3"]}]}"#;

    // With no role, the components are like the tasks of a workflow: nothing says they are shared.
    let map = map_of(
        dir.path(),
        (&sources, &seeds),
        &roles(&sources, false),
        answer,
    )
    .await;

    assert_eq!(slugs(&map), ["components", "billing"]);
}

// spec: CTX-5 (internal: the CLI test of the map would need an LLM server)
#[tokio::test]
async fn a_context_half_made_of_folder_files_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let (sources, seeds) = write_tree(
        dir.path(),
        &[("app/models/card.rb", "ButtonComponent InputComponent")],
    );
    // `billing` and `user` make 4 group files, the components folder 4 more: half, not more.
    let answer = r#"{"contexts":[
        {"slug":"ui","name":"UI","groups":["g2","g3"],"folders":["app/components/inputs/"]},
        {"slug":"cards","name":"Cards","groups":["g1"]}]}"#;

    let map = map_of(
        dir.path(),
        (&sources, &seeds),
        &roles(&sources, true),
        answer,
    )
    .await;

    assert_eq!(map.contexts[0].folders, ["app/components/inputs/"]);
}

// spec: CTX-6 (internal: the CLI test of the map would need an LLM server)
#[tokio::test]
async fn a_catch_all_context_gives_its_groups_and_files_to_the_contexts_that_use_them() {
    let dir = tempfile::tempdir().unwrap();
    let (sources, seeds) = write_tree(dir.path(), &[("app/models/billing.rb", "Card.find(id)")]);
    let answer = r#"{"contexts":[
        {"slug":"billing","name":"Billing","groups":["g2"]},
        {"slug":"misc-business","name":"Misc","groups":["g1","g3"],
         "folders":["app/components/inputs/"]}]}"#;

    let map = map_of(
        dir.path(),
        (&sources, &seeds),
        &roles(&sources, false),
        answer,
    )
    .await;

    assert_eq!(slugs(&map), ["billing", "user"]);
    assert_eq!(map.contexts[0].groups, ["billing", "card"]);
    let resolved = map.resolve(&sources, &seeds);
    assert!(resolved
        .support
        .contains(&PathBuf::from("app/components/inputs/text_component.rb")));
}

// spec: CTX-6 (internal: the CLI test of the map would need an LLM server)
#[tokio::test]
async fn a_context_named_with_a_catch_all_word_among_others_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let (sources, seeds) = write_tree(dir.path(), &[("app/models/billing.rb", "Card")]);
    let answer = r#"{"contexts":[
        {"slug":"billing","name":"Billing","groups":["g2","g3"],"folders":["app/components/"]},
        {"slug":"general-settings","name":"Settings","groups":["g1"]}]}"#;

    let map = map_of(
        dir.path(),
        (&sources, &seeds),
        &roles(&sources, false),
        answer,
    )
    .await;

    assert_eq!(slugs(&map), ["billing", "general-settings"]);
}

// spec: CTX-5 (internal: the CLI test of the map would need an LLM server)
#[tokio::test]
async fn a_file_placed_once_is_listed_once_whatever_the_order_of_the_contexts() {
    let dir = tempfile::tempdir().unwrap();
    let (sources, seeds) = write_tree(
        dir.path(),
        &[
            (
                "app/models/billing.rb",
                "ButtonComponent InputComponent SelectComponent TextComponent",
            ),
            ("app/controllers/billings_controller.rb", "TextComponent"),
            ("app/controllers/users_controller.rb", "TextComponent"),
        ],
    );
    let answer = r#"{"contexts":[
        {"slug":"ui","name":"UI","groups":["g1"],"folders":["app/components/inputs/"]},
        {"slug":"users","name":"Users","groups":["g3"]},
        {"slug":"billing","name":"Billing","groups":["g2"]}]}"#;

    let map = map_of(
        dir.path(),
        (&sources, &seeds),
        &roles(&sources, true),
        answer,
    )
    .await;

    let mut listed: Vec<&String> = map
        .contexts
        .iter()
        .flat_map(|c| &c.files)
        .chain(&map.support.files)
        .collect();
    let all = listed.len();
    listed.sort();
    listed.dedup();
    assert_eq!(listed.len(), all, "{map:?}");
    let billing = map.contexts.iter().find(|c| c.slug == "billing").unwrap();
    assert_eq!(billing.files.len(), 4, "{map:?}");
}

// spec: CTX-5 (internal: the CLI test of the map would need an LLM server)
#[tokio::test]
async fn a_split_gives_each_listed_file_to_the_part_that_writes_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut sources = vec![
        PathBuf::from("app/controllers/contracts_controller.rb"),
        PathBuf::from("app/controllers/company_contracts_controller.rb"),
        PathBuf::from("app/models/contract.rb"),
        PathBuf::from("app/components/card_widget.rb"),
        PathBuf::from("app/components/list_widget.rb"),
    ];
    sources.extend((0..160).map(|i| PathBuf::from(format!("app/workflows/tasks/task_{i}.rb"))));
    sources.extend((0..5).map(|i| PathBuf::from(format!("app/workflows/rules/rule_{i}.rb"))));
    for file in &sources {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = if file.ends_with("task_0.rb") {
            "CardWidget ListWidget"
        } else {
            ""
        };
        std::fs::write(path, text).unwrap();
    }
    let seeds: Vec<PathBuf> = sources[..2].to_vec();
    // `g1` = contract (2 files), `g2` = company_contract (1).
    let llm = FakeLlm::replying(|n, _| {
        Ok(if n == 0 {
            r#"{"contexts":[
                {"slug":"contracts","name":"Contracts","groups":["g1","g2"],"folders":["app/workflows/"]},
                {"slug":"ui","name":"UI","folders":["app/components/"]}]}"#
        } else {
            r#"{"contexts":[
                {"slug":"approvals","name":"Approvals","groups":["g1"],"folders":["app/workflows/rules/"]},
                {"slug":"tasks","name":"Tasks","groups":["g2"],"folders":["app/workflows/tasks/"]}]}"#
        }
        .to_string())
    });

    let map = build_context_map(
        dir.path(),
        &sources,
        &seeds,
        &roles(&sources, true),
        &ProductBrief::default(),
        &llm,
        false,
    )
    .await
    .unwrap();

    assert_eq!(slugs(&map), ["approvals", "tasks"], "{map:?}");
    let resolved = map.resolve(&sources, &seeds);
    assert!(
        resolved.contexts[1]
            .files
            .contains(&PathBuf::from("app/components/card_widget.rb")),
        "{map:?}"
    );
}
