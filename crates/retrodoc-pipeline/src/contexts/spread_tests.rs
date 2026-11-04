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
