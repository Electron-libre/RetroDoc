use super::*;

use crate::testing::FakeLlm;
use crate::{build_entry_points, build_glossary, Retention};
use std::path::Path;

const ANSWER: &str = r#"{"entities":[{"file":"lib/order.rb","name":"Order",
  "description":"A customer order","attributes":["status"],"associations":[]}],
 "entry_points":[{"file":"lib/order.rb","kind":"public_api","name":"Order#cancel",
  "verb":"cancel","resource":"order","description":"Cancels the order",
  "outputs":[{"kind":"db_write","description":"order marked cancelled"}]}]}"#;

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    std::fs::write(
        dir.path().join("lib/order.rb"),
        "class Order\n  def cancel\n    self.status = :cancelled\n  end\nend\n",
    )
    .unwrap();
    dir
}

fn business() -> Vec<PathBuf> {
    vec![PathBuf::from("lib/order.rb")]
}

#[tokio::test]
async fn one_call_fills_both_artifacts_and_the_two_passes_then_read_nothing() {
    let dir = repo();
    let llm = FakeLlm::answering(ANSWER);
    let (roles, brief) = (RoleMap::default(), ProductBrief::default());

    read_business_files(dir.path(), &roles, &business(), &brief, &llm, &llm)
        .await
        .unwrap();

    assert_eq!(llm.calls(), 1);
    let glossary = Glossary::load(dir.path()).unwrap();
    assert_eq!(
        glossary.models[Path::new("lib/order.rb")].entities[0].name,
        "Order"
    );
    let entries = EntryPoints::load(dir.path()).unwrap();
    assert_eq!(
        entries.files[Path::new("lib/order.rb")].entry_points[0].name,
        "Order#cancel"
    );

    // The passes find the file up to date: no call, same results.
    let glossary = build_glossary(
        dir.path(),
        &roles,
        &business(),
        &brief,
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();
    let entries = build_entry_points(
        dir.path(),
        &roles,
        &business(),
        &brief,
        &llm,
        Retention::OnlyRead,
    )
    .await
    .unwrap();
    assert_eq!(llm.calls(), 1);
    assert_eq!(glossary.entities().count(), 1);
    assert_eq!(entries.iter().count(), 1);

    // A rerun is a no-op too.
    read_business_files(dir.path(), &roles, &business(), &brief, &llm, &llm)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 1);
}

#[tokio::test]
async fn a_changed_file_or_another_model_is_read_again() {
    let dir = repo();
    let (roles, brief) = (RoleMap::default(), ProductBrief::default());
    let llm = FakeLlm::answering(ANSWER).with_model("local");
    read_business_files(dir.path(), &roles, &business(), &brief, &llm, &llm)
        .await
        .unwrap();

    std::fs::write(dir.path().join("lib/order.rb"), "class Order; end\n").unwrap();
    read_business_files(dir.path(), &roles, &business(), &brief, &llm, &llm)
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2, "changed file");

    let strong = FakeLlm::answering(ANSWER).with_model("strong");
    read_business_files(dir.path(), &roles, &business(), &brief, &strong, &strong)
        .await
        .unwrap();
    assert_eq!(strong.calls(), 1, "another model");
}

#[tokio::test]
async fn two_models_or_no_business_file_leave_the_reading_to_the_passes() {
    let dir = repo();
    let (roles, brief) = (RoleMap::default(), ProductBrief::default());
    let a = FakeLlm::answering(ANSWER).with_model("a");
    let b = FakeLlm::answering(ANSWER).with_model("b");

    read_business_files(dir.path(), &roles, &business(), &brief, &a, &b)
        .await
        .unwrap();
    read_business_files(dir.path(), &roles, &[], &brief, &a, &a)
        .await
        .unwrap();

    assert_eq!((a.calls(), b.calls()), (0, 0));
    assert!(Glossary::load(dir.path()).is_none());
}

#[tokio::test]
async fn an_unusable_answer_saves_nothing_and_the_passes_read_the_file() {
    let dir = repo();
    let (roles, brief) = (RoleMap::default(), ProductBrief::default());
    let bad = FakeLlm::answering("not json");
    read_business_files(dir.path(), &roles, &business(), &brief, &bad, &bad)
        .await
        .unwrap();
    assert!(Glossary::load(dir.path()).is_none_or(|g| g.models.is_empty()));

    let good = FakeLlm::answering(r#"{"entities":[],"entry_points":[]}"#);
    build_glossary(
        dir.path(),
        &roles,
        &business(),
        &brief,
        &good,
        Retention::OnlyRead,
    )
    .await
    .unwrap();
    assert!(good.calls() >= 1);
}

/// Reads one file in each answer: an entity and an entry point named after the file in the prompt.
fn one_file_per_answer() -> FakeLlm {
    FakeLlm::replying(|_, request| {
        let file = if request.messages[1].content.contains("lib/invoice.rb") {
            ("lib/invoice.rb", "Invoice")
        } else {
            ("lib/order.rb", "Order")
        };
        Ok(format!(
            r#"{{"entities":[{{"file":"{0}","name":"{1}","description":"d","attributes":[],"associations":[]}}],
            "entry_points":[{{"file":"{0}","kind":"public_api","name":"{1}#run","verb":"run",
            "resource":"{1}","description":"d","outputs":[]}}]}}"#,
            file.0, file.1
        ))
    })
}

/// Two readings of two sets of files, as two runs on two contexts would make them.
async fn read_in_two_runs(dir: &Path, keep: Retention) -> (Glossary, EntryPoints, usize) {
    std::fs::write(
        dir.join("lib/invoice.rb"),
        "class Invoice\n  def run\n  end\nend\n",
    )
    .unwrap();
    let llm = one_file_per_answer();
    let (roles, brief) = (RoleMap::default(), ProductBrief::default());
    let mut last = (Glossary::default(), EntryPoints::default());
    for business in [
        vec![PathBuf::from("lib/order.rb")],
        vec![PathBuf::from("lib/invoice.rb")],
    ] {
        read_business_files(dir, &roles, &business, &brief, &llm, &llm)
            .await
            .unwrap();
        last = (
            build_glossary(dir, &roles, &business, &brief, &llm, keep)
                .await
                .unwrap(),
            build_entry_points(dir, &roles, &business, &brief, &llm, keep)
                .await
                .unwrap(),
        );
    }
    (last.0, last.1, llm.calls())
}

#[tokio::test]
async fn a_second_reading_keeps_the_entries_of_the_first_when_asked_to() {
    let dir = repo();

    let (glossary, entries, calls) = read_in_two_runs(dir.path(), Retention::Others).await;

    assert_eq!(
        calls, 2,
        "each file is read once, the second run does not read the first again"
    );
    let entities: Vec<&str> = glossary.entities().map(|(_, e)| e.name.as_str()).collect();
    assert_eq!(entities, ["Invoice", "Order"]);
    assert_eq!(entries.iter().count(), 2);
    assert_eq!(Glossary::load(dir.path()).unwrap().entities().count(), 2);
    assert_eq!(EntryPoints::load(dir.path()).unwrap().iter().count(), 2);
}

#[tokio::test]
async fn by_default_only_the_files_read_stay_in_the_inventories() {
    let dir = repo();

    let (glossary, entries, _) = read_in_two_runs(dir.path(), Retention::OnlyRead).await;

    assert_eq!(glossary.entities().count(), 1);
    assert_eq!(entries.iter().count(), 1);
}

#[tokio::test]
async fn a_deleted_file_does_not_stay_in_the_inventories() {
    let dir = repo();
    read_in_two_runs(dir.path(), Retention::Others).await;
    std::fs::remove_file(dir.path().join("lib/order.rb")).unwrap();
    let llm = one_file_per_answer();

    let glossary = build_glossary(
        dir.path(),
        &RoleMap::default(),
        &[PathBuf::from("lib/invoice.rb")],
        &ProductBrief::default(),
        &llm,
        Retention::Others,
    )
    .await
    .unwrap();

    assert_eq!(glossary.entities().count(), 1);
    assert_eq!(llm.calls(), 0);
}
