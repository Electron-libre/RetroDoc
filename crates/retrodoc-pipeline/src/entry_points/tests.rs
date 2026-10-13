use super::*;
use crate::artifact::Retention;

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
    assert_eq!(entry.verb, "");
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

    let inventory = build_entry_points(
        dir.path(),
        &roles,
        &[],
        &ProductBrief::default(),
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();
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

    build_entry_points(
        dir.path(),
        &roles,
        &[],
        &ProductBrief::default(),
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();
    assert_eq!(llm.prompts().len(), 1);

    std::fs::write(
        dir.path().join("app/controllers/users_controller.rb"),
        "def show; x; end",
    )
    .unwrap();
    build_entry_points(
        dir.path(),
        &roles,
        &[],
        &ProductBrief::default(),
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();
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
        &[],
        &ProductBrief::default(),
        &FakeLlm::failing_after(1, |_| {
            r#"{"entry_points":[{"file":"a.rb","kind":"job","name":"AJob"}]}"#.to_string()
        }),
        Retention::OnlyRead,
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

    let inventory = build_entry_points(
        dir.path(),
        &roles,
        &[],
        &ProductBrief::default(),
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();

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

    let inventory = build_entry_points(
        dir.path(),
        &roles,
        &[],
        &ProductBrief::default(),
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();

    // Both files are in one batch: two failed attempts, then one call each.
    assert_eq!(llm.calls(), 4);
    assert_eq!(inventory.files.len(), 2);
    assert_eq!(inventory.iter().count(), 2);
}

#[tokio::test]
async fn the_brief_heads_the_prompt_and_a_changed_brief_reads_the_files_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rb"), "def show; end").unwrap();
    let roles = RoleMap {
        roles: [(PathBuf::from("a.rb"), FileRole::EntryPoint)]
            .into_iter()
            .collect(),
        ..RoleMap::default()
    };
    let llm =
        FakeLlm::answering(r#"{"entry_points":[{"file":"a.rb","kind":"job","name":"AJob"}]}"#);
    let brief = crate::testing::brief("Sells things to buyers.");

    build_entry_points(dir.path(), &roles, &[], &brief, &llm, Retention::OnlyRead)
        .await
        .unwrap();
    let prompt = &llm.prompts()[0];
    assert!(
        prompt.starts_with("Product brief of the application"),
        "{prompt}"
    );
    assert!(prompt.find("Purpose: Sells things").unwrap() < prompt.find("Files:").unwrap());

    build_entry_points(dir.path(), &roles, &[], &brief, &llm, Retention::OnlyRead)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 1);

    let other = crate::testing::brief("Rents things to renters.");
    build_entry_points(dir.path(), &roles, &[], &other, &llm, Retention::OnlyRead)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn another_model_reads_the_unchanged_files_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("app/controllers")).unwrap();
    std::fs::write(
        dir.path().join("app/controllers/users_controller.rb"),
        "def show; end",
    )
    .unwrap();
    let roles = RoleMap {
        roles: [(
            PathBuf::from("app/controllers/users_controller.rb"),
            FileRole::EntryPoint,
        )]
        .into_iter()
        .collect(),
        ..RoleMap::default()
    };
    let answer = r#"{"entry_points":[{"file":"app/controllers/users_controller.rb",
        "kind":"http_route","name":"GET /users/:id"}]}"#;
    let mut calls = Vec::new();
    for model in ["local", "local", "strong"] {
        let llm = FakeLlm::answering(answer).with_model(model);
        build_entry_points(
            dir.path(),
            &roles,
            &[],
            &ProductBrief::default(),
            &llm,
            Retention::OnlyRead,
        )
        .await
        .unwrap();
        calls.push(llm.calls());
    }
    assert_eq!(calls, [1, 0, 1]);
}

#[tokio::test]
async fn business_files_are_read_next_to_the_entrypoint_files_without_reading_one_twice() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    for file in ["router.rb", "order.rb", "helper.rb"] {
        std::fs::write(dir.path().join("lib").join(file), "def call; end").unwrap();
    }
    let roles = RoleMap {
        roles: [
            ("lib/router.rb", FileRole::EntryPoint),
            ("lib/order.rb", FileRole::Model),
            ("lib/helper.rb", FileRole::Logic),
        ]
        .into_iter()
        .map(|(p, r)| (PathBuf::from(p), r))
        .collect(),
        ..RoleMap::default()
    };
    // The router is in both lists; the helper is in neither.
    let business = [
        PathBuf::from("lib/order.rb"),
        PathBuf::from("lib/router.rb"),
    ];
    let llm = FakeLlm::answering(
        r#"{"entry_points":[
                {"file":"lib/router.rb","kind":"public_api","name":"Router#route"},
                {"file":"lib/order.rb","kind":"public_api","name":"Order.new"}]}"#,
    );

    let inventory = build_entry_points(
        dir.path(),
        &roles,
        &business,
        &ProductBrief::default(),
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();

    let files: Vec<_> = inventory.iter().map(|(file, _)| file).collect();
    assert_eq!(
        files,
        [Path::new("lib/order.rb"), Path::new("lib/router.rb")]
    );
    let prompt = &llm.prompts()[0];
    assert_eq!(prompt.matches("lib/router.rb").count(), 1);
    assert!(prompt.contains("lib/order.rb") && !prompt.contains("helper.rb"));
}

#[tokio::test]
async fn the_prompt_leaves_out_what_only_the_application_itself_calls_but_keeps_a_librarys_api() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rb"), "def call; end").unwrap();
    let roles = RoleMap {
        roles: [(PathBuf::from("a.rb"), FileRole::EntryPoint)].into(),
        ..RoleMap::default()
    };
    let llm = FakeLlm::answering(r#"{"entry_points":[]}"#);

    build_entry_points(
        dir.path(),
        &roles,
        &[],
        &ProductBrief::default(),
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();

    let (system, _) = &llm.prompt_pairs()[0];
    assert!(
        system.contains("only called by other code of the application"),
        "a service, query or form helper is not an entry point: {system}"
    );
    assert!(
        system.contains("methods a user of the library calls"),
        "the public API of a library still is: {system}"
    );
}

#[tokio::test]
async fn a_shorter_view_of_the_bodies_reads_the_files_again() {
    let dir = tempfile::tempdir().unwrap();
    let body = "    step_1\n    step_2\n    step_3\n    step_4\n    step_5\n    step_6\n    step_7\n    step_8\n";
    std::fs::write(
        dir.path().join("a_controller.rb"),
        format!("class AController\n  def sign\n{body}  end\nend\n"),
    )
    .unwrap();
    let mut roles = RoleMap {
        roles: [(PathBuf::from("a_controller.rb"), FileRole::EntryPoint)]
            .into_iter()
            .collect(),
        ..RoleMap::default()
    };
    let llm = FakeLlm::answering(r#"{"entry_points":[]}"#);
    let brief = ProductBrief::default();

    build_entry_points(dir.path(), &roles, &[], &brief, &llm, Retention::OnlyRead)
        .await
        .unwrap();
    build_entry_points(dir.path(), &roles, &[], &brief, &llm, Retention::OnlyRead)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 1, "unchanged: reused");
    assert!(llm.prompts()[0].contains("step_8"));

    roles.body_lines = Some(2);
    build_entry_points(dir.path(), &roles, &[], &brief, &llm, Retention::OnlyRead)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2, "another view: read again");
    let prompt = &llm.prompts()[1];
    assert!(prompt.contains("step_2") && !prompt.contains("step_8"));
    assert!(prompt.contains("lines omitted"));
}

fn inventory(files: &[(&str, &[(EntryKind, &str)])]) -> EntryPoints {
    EntryPoints {
        files: files
            .iter()
            .map(|(path, entries)| {
                (
                    PathBuf::from(path),
                    EntryFile {
                        content_hash: String::new(),
                        entry_points: entries
                            .iter()
                            .map(|(kind, name)| EntryPoint {
                                kind: *kind,
                                name: (*name).to_string(),
                                verb: String::new(),
                                resource: String::new(),
                                description: String::new(),
                                outputs: Vec::new(),
                            })
                            .collect(),
                    },
                )
            })
            .collect(),
    }
}

fn names(entries: &EntryPoints) -> Vec<String> {
    entries.iter().map(|(_, e)| e.name.clone()).collect()
}

#[test]
fn in_an_application_a_public_method_no_route_reaches_is_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let files = [
        ("a/contracts_controller.rb", "PdfBuilder.call(contract)"),
        ("a/pdf_builder.rb", "class PdfBuilder; FontLoader; end"),
        ("a/font_loader.rb", "class FontLoader; end"),
        ("a/directory_client.rb", "class DirectoryClient; end"),
    ];
    std::fs::create_dir_all(dir.path().join("a")).unwrap();
    for (path, content) in files {
        std::fs::write(dir.path().join(path), content).unwrap();
    }
    let entries = inventory(&[
        (
            "a/contracts_controller.rb",
            &[(EntryKind::HttpRoute, "GET /contracts")],
        ),
        ("a/pdf_builder.rb", &[(EntryKind::PublicApi, "build")]),
        ("a/font_loader.rb", &[(EntryKind::PublicApi, "load")]),
        (
            "a/directory_client.rb",
            &[(EntryKind::PublicApi, "lookup"), (EntryKind::Other, "misc")],
        ),
    ]);
    assert!(entries.is_application());

    let (kept, left_out) =
        entries.without_unreached_public_api(dir.path(), files.iter().map(|(p, _)| Path::new(p)));

    // The builder is called by the controller, the loader by the builder (two hops).
    assert_eq!(left_out, vec!["a/directory_client.rb#lookup"]);
    assert_eq!(names(&kept), ["GET /contracts", "misc", "load", "build"]);
}

#[test]
fn a_library_keeps_its_public_api() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("router.rb"), "class Router; end").unwrap();
    let entries = inventory(&[("router.rb", &[(EntryKind::PublicApi, "route")])]);
    assert!(!entries.is_application());

    let (kept, left_out) =
        entries.without_unreached_public_api(dir.path(), [Path::new("router.rb")]);

    assert_eq!(left_out, Vec::<String>::new());
    assert_eq!(names(&kept), ["route"]);
}

#[test]
fn the_reach_stops_after_two_hops_and_a_file_with_a_route_keeps_its_public_methods() {
    let dir = tempfile::tempdir().unwrap();
    let files = [
        ("a/orders_controller.rb", "OrderService.call"),
        ("a/order_service.rb", "OrderMailer.deliver"),
        ("a/order_mailer.rb", "MailTemplate.render"),
        ("a/mail_template.rb", "class MailTemplate; end"),
    ];
    std::fs::create_dir_all(dir.path().join("a")).unwrap();
    for (path, content) in files {
        std::fs::write(dir.path().join(path), content).unwrap();
    }
    let entries = inventory(&[
        (
            "a/orders_controller.rb",
            &[
                (EntryKind::HttpRoute, "GET /orders"),
                (EntryKind::PublicApi, "helper"),
            ],
        ),
        ("a/order_service.rb", &[(EntryKind::PublicApi, "call")]),
        ("a/order_mailer.rb", &[(EntryKind::PublicApi, "deliver")]),
        ("a/mail_template.rb", &[(EntryKind::PublicApi, "render")]),
    ]);

    let (kept, left_out) =
        entries.without_unreached_public_api(dir.path(), files.iter().map(|(p, _)| Path::new(p)));

    // Controller -> service (1) -> mailer (2) -> template (3, too far).
    assert_eq!(left_out, vec!["a/mail_template.rb#render"]);
    assert_eq!(
        names(&kept),
        ["deliver", "call", "GET /orders", "helper"],
        "path order"
    );
}

#[test]
fn a_library_with_one_command_keeps_its_public_api() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("cli.rb"), "Router.new").unwrap();
    std::fs::write(dir.path().join("router.rb"), "class Router; end").unwrap();
    std::fs::write(dir.path().join("other.rb"), "class Other; end").unwrap();
    let entries = inventory(&[
        ("cli.rb", &[(EntryKind::CliCommand, "lint")]),
        ("router.rb", &[(EntryKind::PublicApi, "route")]),
        ("other.rb", &[(EntryKind::PublicApi, "other")]),
    ]);
    assert!(!entries.is_application());

    let (kept, left_out) = entries.without_unreached_public_api(
        dir.path(),
        ["cli.rb", "router.rb", "other.rb"].map(Path::new),
    );

    assert_eq!(left_out, Vec::<String>::new());
    assert_eq!(names(&kept).len(), 3);
}
