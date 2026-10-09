use super::*;

use crate::testing::FakeLlm;

fn roles(entries: &[(&str, FileRole)]) -> RoleMap {
    RoleMap {
        roles: entries
            .iter()
            .map(|(p, r)| (PathBuf::from(p), *r))
            .collect(),
        ..RoleMap::default()
    }
}

#[test]
fn merges_entities_listed_under_several_files() {
    let entity =
        |name: &str, description: &str, attributes: &[&str], target: Option<&str>| Entity {
            name: name.to_string(),
            description: description.to_string(),
            attributes: attributes.iter().map(|a| (*a).to_string()).collect(),
            associations: target
                .map(|t| Association {
                    kind: "has_many".to_string(),
                    target: t.to_string(),
                })
                .into_iter()
                .collect(),
        };
    let file = |entities| ModelFile {
        content_hash: String::new(),
        entities,
    };
    let glossary = Glossary {
        models: BTreeMap::from([
            (
                PathBuf::from("app/models/actions/company_user_actions.rb"),
                file(vec![entity(
                    "Company",
                    "short",
                    &["name"],
                    Some("CompanyUser"),
                )]),
            ),
            (
                PathBuf::from("app/models/company.rb"),
                file(vec![entity(
                    "company",
                    "A business organization",
                    &["Name", "siret"],
                    Some("Worksite"),
                )]),
            ),
            (
                PathBuf::from("app/models/user.rb"),
                file(vec![entity("User", "A person", &[], None)]),
            ),
        ]),
        tests: Vec::new(),
    };

    let merged = glossary.merged_entities();
    assert_eq!(merged.len(), 2);
    let company = &merged[0];
    assert_eq!(company.name, "company");
    assert_eq!(company.description, "A business organization");
    assert_eq!(company.attributes, vec!["Name", "siret"]);
    assert_eq!(company.associations.len(), 2);
    assert_eq!(
        company.files,
        vec![
            PathBuf::from("app/models/company.rb"),
            PathBuf::from("app/models/actions/company_user_actions.rb"),
        ]
    );
    assert_eq!(merged[1].name, "User");
}

#[tokio::test]
async fn reads_entities_once_and_reuses_them_while_files_are_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("app/models")).unwrap();
    std::fs::create_dir_all(dir.path().join("spec")).unwrap();
    std::fs::write(
        dir.path().join("app/models/contract.rb"),
        "class Contract; end",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("app/models/company.rb"),
        "class Company; end",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("spec/contract_spec.rb"),
        "it \"is signed\" do",
    )
    .unwrap();
    let roles = roles(&[
        ("app/models/contract.rb", FileRole::Model),
        ("app/models/company.rb", FileRole::Model),
        ("spec/contract_spec.rb", FileRole::Test),
    ]);
    let llm = FakeLlm::answering(
        r#"{"entities":[
                {"file":"models/contract.rb","name":"Contract","description":"An agreement",
                 "attributes":["signed_at"],"associations":[{"kind":"belongs_to","target":"Company"}]},
                {"file":"app/models/company.rb","name":"Company"},
                {"file":"nowhere.rb","name":"Ghost"}]}"#,
    );

    let glossary = build_glossary(dir.path(), &roles, &[], &ProductBrief::default(), &llm)
        .await
        .unwrap();
    let entities: Vec<_> = glossary
        .entities()
        .map(|(p, e)| (p.to_string_lossy().into_owned(), e.name.clone()))
        .collect();
    assert_eq!(
        entities,
        vec![
            ("app/models/company.rb".to_string(), "Company".to_string()),
            ("app/models/contract.rb".to_string(), "Contract".to_string()),
        ]
    );
    assert_eq!(glossary.phrase_count(), 1);
    assert_eq!(llm.prompts().len(), 1);

    // Second run: nothing changed, no call.
    build_glossary(dir.path(), &roles, &[], &ProductBrief::default(), &llm)
        .await
        .unwrap();
    assert_eq!(llm.prompts().len(), 1);

    // A changed file is the only one sent again.
    std::fs::write(
        dir.path().join("app/models/company.rb"),
        "class Company; x; end",
    )
    .unwrap();
    build_glossary(dir.path(), &roles, &[], &ProductBrief::default(), &llm)
        .await
        .unwrap();
    let prompts = llm.prompts();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[1].contains("company.rb") && !prompts[1].contains("contract.rb"));
}

#[test]
fn entities_of_a_class_cut_in_two_are_merged() {
    let entity = |attributes: &[&str], target: &str| Entity {
        name: "Contract".to_string(),
        description: String::new(),
        attributes: attributes.iter().map(ToString::to_string).collect(),
        associations: vec![Association {
            kind: "has_many".to_string(),
            target: target.to_string(),
        }],
    };
    let merged = merge_chunk_entities(vec![
        entity(&["title"], "Signatory"),
        entity(&["title", "status"], "Folder"),
    ]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].attributes, vec!["title", "status"]);
    assert_eq!(merged[0].associations.len(), 2);
}

#[tokio::test]
async fn a_long_model_file_is_read_in_chunks_and_its_entity_saved_once() {
    let dir = tempfile::tempdir().unwrap();
    let method = "def sign\n  work\nend\n\n";
    let repeats = MAX_MODEL_FILE_CHARS * 4 / method.len();
    std::fs::write(dir.path().join("contract.rb"), method.repeat(repeats)).unwrap();
    let roles = roles(&[("contract.rb", FileRole::Model)]);
    let llm = FakeLlm::answering(r#"{"entities":[{"file":"contract.rb","name":"Contract"}]}"#);

    let glossary = build_glossary(dir.path(), &roles, &[], &ProductBrief::default(), &llm)
        .await
        .unwrap();

    let prompts = llm.prompts();
    assert!(prompts.len() >= 2, "several parts, several calls");
    assert!(prompts[0].contains("(part 1/"));
    assert_eq!(glossary.entities().count(), 1);
}

#[tokio::test]
async fn a_batch_that_cannot_be_answered_is_retried_file_by_file() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.rb", "b.rb"] {
        std::fs::write(dir.path().join(name), "class Thing; end").unwrap();
    }
    let roles = roles(&[("a.rb", FileRole::Model), ("b.rb", FileRole::Model)]);

    let glossary = build_glossary(
        dir.path(),
        &roles,
        &[],
        &ProductBrief::default(),
        &FakeLlm::replying(|_, request| {
            Ok(
                if request.messages[1].content.matches("\n--- ").count() > 1 {
                    "too long, cut".to_string()
                } else {
                    r#"{"entities":[{"file":"x","name":"Thing"}]}"#.to_string()
                },
            )
        }),
    )
    .await
    .unwrap();

    assert_eq!(glossary.models.len(), 2);
    assert_eq!(glossary.entities().count(), 2);
}

#[tokio::test]
async fn the_brief_heads_the_prompt_and_a_changed_brief_reads_the_files_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("order.rb"), "class Order; end").unwrap();
    let roles = roles(&[("order.rb", FileRole::Model)]);
    let llm = FakeLlm::answering(r#"{"entities":[{"file":"order.rb","name":"Order"}]}"#);
    let brief = crate::testing::brief("Sells things to buyers.");

    build_glossary(dir.path(), &roles, &[], &brief, &llm)
        .await
        .unwrap();
    let prompt = &llm.prompts()[0];
    assert!(
        prompt.starts_with("Product brief of the application"),
        "{prompt}"
    );
    assert!(prompt.find("Purpose: Sells things").unwrap() < prompt.find("Model files:").unwrap());

    build_glossary(dir.path(), &roles, &[], &brief, &llm)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 1);

    let other = crate::testing::brief("Rents things to renters.");
    build_glossary(dir.path(), &roles, &[], &other, &llm)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn the_business_files_are_read_instead_of_the_model_role() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("migrations")).unwrap();
    std::fs::write(dir.path().join("migrations/0001.py"), "create_table").unwrap();
    std::fs::write(dir.path().join("pricing.py"), "def price(order): ...").unwrap();
    let roles = roles(&[
        ("migrations/0001.py", FileRole::Model),
        ("pricing.py", FileRole::Logic),
    ]);
    let llm = FakeLlm::answering(
        r#"{"entities":[{"file":"pricing.py","name":"Price","description":"What an order costs"}]}"#,
    );

    let glossary = build_glossary(
        dir.path(),
        &roles,
        &[PathBuf::from("pricing.py")],
        &ProductBrief::default(),
        &llm,
    )
    .await
    .unwrap();

    assert_eq!(glossary.entities().count(), 1);
    let sent = llm.prompts().join("\n");
    assert!(sent.contains("pricing.py"), "{sent}");
    assert!(!sent.contains("0001.py"), "{sent}");
}

#[tokio::test]
async fn another_model_reads_the_unchanged_files_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("app/models")).unwrap();
    std::fs::write(
        dir.path().join("app/models/company.rb"),
        "class Company; end",
    )
    .unwrap();
    let roles = roles(&[("app/models/company.rb", FileRole::Model)]);
    let answer = r#"{"entities":[{"file":"app/models/company.rb","name":"Company"}]}"#;
    let mut calls = Vec::new();
    for model in ["local", "local", "strong"] {
        let llm = FakeLlm::answering(answer).with_model(model);
        build_glossary(dir.path(), &roles, &[], &ProductBrief::default(), &llm)
            .await
            .unwrap();
        calls.push(llm.calls());
    }
    assert_eq!(calls, [1, 0, 1]);
}
