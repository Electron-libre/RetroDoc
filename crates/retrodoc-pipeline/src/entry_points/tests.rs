use super::*;

use crate::testing::FakeLlm;

#[test]
fn unknown_kinds_parse_as_other_and_missing_fields_default() {
    let parsed: EntryPointsResponse = serde_json::from_str(
        r#"{"entry_points":[{"file":"a","kind":"carrier_pigeon","name":"Send",
                "outputs":[{"kind":"smoke_signal"}]}]}"#,
    )
    .unwrap();
    let entry = &parsed.entry_points[0].entry;
    assert_eq!(entry.kind, EntryKind::Other);
    assert_eq!(entry.outputs[0].kind, OutputKind::Other);
    assert!(entry.verb.is_empty());
}

#[tokio::test]
async fn reads_entry_points_once_and_reuses_them_while_files_are_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("app/controllers")).unwrap();
    std::fs::write(
        dir.path().join("app/controllers/contracts_controller.rb"),
        "def sign; end",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("app/controllers/users_controller.rb"),
        "def show; end",
    )
    .unwrap();
    std::fs::write(dir.path().join("app/models/user.rb"), "class User; end").ok();
    let roles = RoleMap {
        roles: [
            (
                "app/controllers/contracts_controller.rb",
                FileRole::EntryPoint,
            ),
            ("app/controllers/users_controller.rb", FileRole::EntryPoint),
            ("app/models/user.rb", FileRole::Model),
        ]
        .into_iter()
        .map(|(p, r)| (PathBuf::from(p), r))
        .collect(),
        ..RoleMap::default()
    };
    let llm = FakeLlm::answering(
        r#"{"entry_points":[
                {"file":"controllers/contracts_controller.rb","kind":"http_route",
                 "name":"POST /contracts/:id/sign","verb":"sign","resource":"contract",
                 "description":"A signatory signs a contract",
                 "outputs":[{"kind":"email","description":"confirmation to the parties"},
                            {"kind":"db_write","description":"contract marked signed"}]},
                {"file":"nowhere.rb","kind":"job","name":"Ghost"}]}"#,
    );

    let inventory = build_entry_points(dir.path(), &roles, &llm).await.unwrap();
    let all: Vec<_> = inventory.iter().collect();
    assert_eq!(all.len(), 1);
    assert_eq!(
        all[0].0,
        Path::new("app/controllers/contracts_controller.rb")
    );
    assert_eq!(all[0].1.outputs.len(), 2);
    assert_eq!(inventory.distribution()[&EntryKind::HttpRoute], 1);
    // The model file is never sent.
    assert!(!llm.prompts()[0].contains("user.rb"));
    assert_eq!(llm.prompts().len(), 1);

    build_entry_points(dir.path(), &roles, &llm).await.unwrap();
    assert_eq!(llm.prompts().len(), 1);

    std::fs::write(
        dir.path().join("app/controllers/users_controller.rb"),
        "def show; x; end",
    )
    .unwrap();
    build_entry_points(dir.path(), &roles, &llm).await.unwrap();
    let prompts = llm.prompts();
    assert_eq!(prompts.len(), 2);
    assert!(
        prompts[1].contains("users_controller") && !prompts[1].contains("contracts_controller")
    );
}

#[tokio::test]
async fn a_failure_in_a_later_batch_keeps_the_earlier_ones_saved() {
    let dir = tempfile::tempdir().unwrap();
    let names = ["a.rb", "b.rb", "c.rb"];
    for name in names {
        // 5,000 chars each: two files fill a batch, the third starts another.
        std::fs::write(dir.path().join(name), "x".repeat(MAX_ENTRY_FILE_CHARS)).unwrap();
    }
    let roles = RoleMap {
        roles: names
            .iter()
            .map(|n| (PathBuf::from(n), FileRole::EntryPoint))
            .collect(),
        ..RoleMap::default()
    };

    let result = build_entry_points(
        dir.path(),
        &roles,
        &FakeLlm::failing_after(1, |_| {
            r#"{"entry_points":[{"file":"a.rb","kind":"job","name":"AJob"}]}"#.to_string()
        }),
    )
    .await;

    assert!(result.is_err());
    let saved = EntryPoints::load(dir.path()).expect("first batch saved");
    assert_eq!(saved.files.len(), 2);
    assert_eq!(saved.iter().count(), 1);
}

#[tokio::test]
async fn a_long_file_is_read_in_chunks_and_saved_once_all_are_read() {
    let dir = tempfile::tempdir().unwrap();
    let method = "def action\n  work\nend\n\n";
    let repeats = MAX_ENTRY_FILE_CHARS * 4 / method.len();
    std::fs::write(dir.path().join("big.rb"), method.repeat(repeats)).unwrap();
    let roles = RoleMap {
        roles: [(PathBuf::from("big.rb"), FileRole::EntryPoint)]
            .into_iter()
            .collect(),
        ..RoleMap::default()
    };
    let llm = FakeLlm::answering(
        r#"{"entry_points":[{"file":"big.rb","kind":"http_route","name":"GET /a"}]}"#,
    );

    let inventory = build_entry_points(dir.path(), &roles, &llm).await.unwrap();

    let prompts = llm.prompts();
    assert!(prompts.len() >= 2, "several parts, several calls");
    assert!(prompts[0].contains("(part 1/"));
    assert!(prompts[1].contains("(part 3/"));
    // The same entry point seen in every part is kept once.
    assert_eq!(inventory.iter().count(), 1);
}

#[tokio::test]
async fn a_batch_that_cannot_be_answered_is_retried_file_by_file() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.rb", "b.rb"] {
        std::fs::write(dir.path().join(name), "def run; end").unwrap();
    }
    let roles = RoleMap {
        roles: ["a.rb", "b.rb"]
            .iter()
            .map(|n| (PathBuf::from(n), FileRole::EntryPoint))
            .collect(),
        ..RoleMap::default()
    };
    let llm = FakeLlm::replying(|_, request| {
        Ok(
            if request.messages[1].content.matches("\n--- ").count() > 1 {
                "too long, cut".to_string()
            } else {
                r#"{"entry_points":[{"file":"x","kind":"job","name":"AJob"}]}"#.to_string()
            },
        )
    });

    let inventory = build_entry_points(dir.path(), &roles, &llm).await.unwrap();

    // Both files are in one batch: two failed attempts, then one call each.
    assert_eq!(llm.calls(), 4);
    assert_eq!(inventory.files.len(), 2);
    assert_eq!(inventory.iter().count(), 2);
}
