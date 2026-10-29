//! A repository documented in two runs, one bounded context each: what the first run produced is
//! neither redone nor changed by the second.

use std::path::PathBuf;

use super::*;
use crate::artifact::Retention;
use crate::brief::{Evidence, ProductBrief};
use crate::entry_points::{EntryFile, EntryKind, EntryPoint, EntryPoints};
use crate::features::{build_features_from_surface, load_features};
use crate::glossary::{Entity, Glossary, ModelFile};
use crate::roles::{FileRole, RoleMap};
use crate::slices::CodeIndex;
use crate::surface::Surface;
use crate::testing::{reading, FakeLlm};
use crate::use_cases::{build_use_cases, UseCaseContext};

struct Fixture {
    dir: tempfile::TempDir,
    sources: Vec<PathBuf>,
    seeds: Vec<PathBuf>,
    roles: RoleMap,
    map: ContextMap,
    entry_points: EntryPoints,
    glossary: Glossary,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let (mut sources, mut seeds) = (Vec::new(), Vec::new());
    let mut roles = RoleMap::default();
    let mut entry_points = EntryPoints::default();
    let mut glossary = Glossary::default();
    for (name, resource) in [("order", "order"), ("rider", "rider")] {
        let controller = format!("app/controllers/{name}s_controller.rb");
        let model = format!("app/models/{name}.rb");
        for (path, role) in [
            (&controller, FileRole::EntryPoint),
            (&model, FileRole::Model),
        ] {
            let file = dir.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, format!("class {name}\nend\n")).unwrap();
            sources.push(PathBuf::from(path));
            roles.roles.insert(PathBuf::from(path), role);
        }
        seeds.push(PathBuf::from(&controller));
        entry_points.files.insert(
            PathBuf::from(&controller),
            EntryFile {
                content_hash: "h".to_string(),
                entry_points: vec![EntryPoint {
                    kind: EntryKind::HttpRoute,
                    name: format!("GET /{name}s"),
                    verb: "get".to_string(),
                    resource: resource.to_string(),
                    description: format!("lists the {name}s"),
                    outputs: Vec::new(),
                }],
            },
        );
        glossary.models.insert(
            PathBuf::from(&model),
            ModelFile {
                content_hash: "h".to_string(),
                entities: vec![Entity {
                    name: name.to_uppercase(),
                    description: String::new(),
                    attributes: Vec::new(),
                    associations: Vec::new(),
                }],
            },
        );
    }
    let context = |slug: &str, group: &str| BoundedContext {
        slug: slug.to_string(),
        name: slug.to_string(),
        description: "d".to_string(),
        groups: vec![group.to_string()],
        ..BoundedContext::default()
    };
    Fixture {
        dir,
        sources,
        seeds,
        roles,
        map: ContextMap {
            contexts: vec![context("orders", "order"), context("riders", "rider")],
            support: Support::default(),
        },
        entry_points,
        glossary,
    }
}

fn feature_answers() -> FakeLlm {
    FakeLlm::replying(|_, request| {
        let (slug, entry) = if request.messages[1].content.contains("GET /riders") {
            (
                "riders-list",
                "app/controllers/riders_controller.rb#GET /riders",
            )
        } else {
            (
                "orders-list",
                "app/controllers/orders_controller.rb#GET /orders",
            )
        };
        Ok(format!(
            r#"{{"features":[{{"slug":"{slug}","name":"{slug}","description":"d","entry_points":["{entry}"]}}]}}"#
        ))
    })
}

fn use_case_answers() -> FakeLlm {
    FakeLlm::replying(|n, _| {
        Ok(format!(
            r#"{{"use_cases":[{{"slug":"uc{n}","name":"U","description":"d","steps":[
            {{"description":"s","actor":{{"name":"A","kind":"human"}},"action":"x","source_refs":[]}}]}}]}}"#
        ))
    })
}

/// One run on the next context: read nothing (the inventories are the fixture's), derive the
/// features then the use cases of every feature.
async fn run(
    f: &Fixture,
    features_llm: &FakeLlm,
    use_cases_llm: &FakeLlm,
) -> (
    RunSlice,
    Vec<retrodoc_core::model::Feature>,
    Vec<retrodoc_core::model::UseCase>,
) {
    let slice = plan_run(f.dir.path(), &f.map, &f.sources, &f.seeds, &f.roles, &[], 1).unwrap();
    let resolved = f.map.resolve(&f.sources, &f.seeds);
    let all: Vec<String> = f.map.contexts.iter().map(|c| c.slug.clone()).collect();
    let in_run = domains_from_contexts(
        &resolved,
        &slice.reads,
        &f.glossary,
        &f.entry_points,
        &slice.contexts,
    );
    let everywhere =
        domains_from_contexts(&resolved, &slice.reads, &f.glossary, &f.entry_points, &all);

    let features = build_features_from_surface(
        f.dir.path(),
        &in_run,
        &f.entry_points,
        &Surface::default(),
        &ProductBrief::default(),
        &Evidence::default(),
        features_llm,
        Retention::Others,
    )
    .await
    .unwrap();
    let context = UseCaseContext {
        entry_points: f.entry_points.clone(),
        index: CodeIndex::new(f.sources.iter().map(PathBuf::as_path)),
        // The vocabulary of the whole application grows with every context read.
        vocabulary: vec![format!("{} contexts known", slice.done.len())],
        domain_vocabulary: domain_vocabulary(&everywhere, &f.glossary, 5),
        ..reading(f.dir.path())
    };
    let use_cases = build_use_cases(f.dir.path(), &features, &context, use_cases_llm)
        .await
        .unwrap();
    for slug in &slice.contexts {
        mark_done(f.dir.path(), &f.map, &f.sources, &f.seeds, &f.roles, slug).unwrap();
    }
    (slice, features, use_cases)
}

#[tokio::test]
async fn the_second_run_documents_the_next_context_and_leaves_the_first_as_it_was() {
    let f = fixture();
    let (features_llm, use_cases_llm) = (feature_answers(), use_case_answers());

    let (first, features_1, use_cases_1) = run(&f, &features_llm, &use_cases_llm).await;
    assert_eq!(first.contexts, ["orders"]);
    assert_eq!(features_1.len(), 1);
    assert_eq!(use_cases_1.len(), 1);
    let saved_use_cases =
        std::fs::read_to_string(crate::Artifact::UseCases.path(f.dir.path())).unwrap();

    let (second, features_2, use_cases_2) = run(&f, &features_llm, &use_cases_llm).await;

    assert_eq!(second.contexts, ["riders"]);
    assert_eq!(second.done, ["orders"]);
    assert_eq!(features_2.len(), 2, "the first context's feature is kept");
    assert_eq!(use_cases_2.len(), 2);
    assert_eq!(
        features_llm.calls(),
        2,
        "one call for each context, none to redo the first"
    );
    assert_eq!(
        use_cases_llm.calls(),
        2,
        "the first feature's use cases were reused"
    );
    let yaml = |value: &dyn erased_serde_free::Yaml| value.yaml();
    assert_eq!(
        yaml(&features_2[0]),
        yaml(&features_1[0]),
        "and left as they were"
    );
    assert_eq!(yaml(&use_cases_2[0]), yaml(&use_cases_1[0]));
    let saved_after =
        std::fs::read_to_string(crate::Artifact::UseCases.path(f.dir.path())).unwrap();
    assert!(
        saved_after.starts_with(saved_use_cases.trim_end()),
        "the first use cases come first and are written as before"
    );
    assert_eq!(load_features(f.dir.path()).unwrap().len(), 2);

    // Nothing is left to document, and asking for everything again calls nothing.
    let third = plan_run(
        f.dir.path(),
        &f.map,
        &f.sources,
        &f.seeds,
        &f.roles,
        &[],
        10,
    )
    .unwrap();
    assert_eq!(third.contexts, Vec::<String>::new());
    assert_eq!(third.done, ["orders", "riders"]);
}

/// A value as YAML, to compare what the models do not compare by themselves.
mod erased_serde_free {
    pub trait Yaml {
        fn yaml(&self) -> String;
    }

    impl<T: serde::Serialize> Yaml for T {
        fn yaml(&self) -> String {
            serde_yaml::to_string(self).unwrap()
        }
    }
}
