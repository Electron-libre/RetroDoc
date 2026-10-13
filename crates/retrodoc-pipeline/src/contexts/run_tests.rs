use std::path::PathBuf;

use super::*;
use crate::contexts::{BoundedContext, Support};
use crate::roles::{FileRole, RoleMap};

fn context(slug: &str, groups: &[&str]) -> BoundedContext {
    BoundedContext {
        slug: slug.to_string(),
        name: slug.to_string(),
        groups: groups.iter().map(ToString::to_string).collect(),
        ..BoundedContext::default()
    }
}

/// Three contexts of 5, 4 and 6 files (`a` = contracts, `b` = invoices, `c` = users), each with
/// its controller; the files are written with content that makes a reference.
struct Repo {
    dir: tempfile::TempDir,
    sources: Vec<PathBuf>,
    seeds: Vec<PathBuf>,
    map: ContextMap,
    roles: RoleMap,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let mut sources = Vec::new();
    let mut seeds = Vec::new();
    let mut roles = RoleMap::default();
    let mut add = |path: &str, role: FileRole, content: &str, seed: bool| {
        let file = dir.path().join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, content).unwrap();
        sources.push(PathBuf::from(path));
        roles.roles.insert(PathBuf::from(path), role);
        if seed {
            seeds.push(PathBuf::from(path));
        }
    };
    for (name, n) in [("contract", 5), ("invoice", 4), ("user", 6)] {
        let reference = match name {
            // The contracts controller calls a service no name reaches, and an invoice model.
            "contract" => "PdfRenderer.new\nInvoice.find\n",
            _ => "",
        };
        add(
            &format!("app/controllers/{name}s_controller.rb"),
            FileRole::EntryPoint,
            &format!("class {}sController\n{reference}end\n", name.to_uppercase()),
            true,
        );
        for i in 1..n {
            let role = if i == 1 {
                FileRole::View
            } else {
                FileRole::Model
            };
            add(
                &format!("app/models/{name}_part_{i}.rb"),
                role,
                &format!("class {name}Part{i}\nend\n"),
                false,
            );
        }
    }
    // Models named after their object, so that the names place them.
    add(
        "app/models/invoice.rb",
        FileRole::Model,
        "class Invoice\nend\n",
        false,
    );
    add(
        "app/services/pdf_renderer.rb",
        FileRole::Logic,
        "class PdfRenderer\nend\n",
        false,
    );
    add(
        "config/initializers/pdf.rb",
        FileRole::Infra,
        "PDF = 1\n",
        false,
    );
    let map = ContextMap {
        contexts: vec![
            context("contracts", &["contract"]),
            context("invoices", &["invoice"]),
            context("users", &["user"]),
        ],
        support: Support::default(),
    };
    Repo {
        dir,
        sources,
        seeds,
        map,
        roles,
    }
}

fn plan(repo: &Repo, named: &[&str], budget: usize) -> Result<RunSlice, PipelineError> {
    let named: Vec<String> = named.iter().map(ToString::to_string).collect();
    plan_run(
        repo.dir.path(),
        &repo.map,
        &repo.sources,
        &repo.seeds,
        &repo.roles,
        &named,
        budget,
    )
}

#[test]
fn the_next_contexts_are_taken_in_the_order_of_the_map_within_the_budget() {
    let repo = repo();
    let resolved = repo.map.resolve(&repo.sources, &repo.seeds);
    let sizes: Vec<usize> = resolved.contexts.iter().map(|c| c.files.len()).collect();
    assert_eq!(
        sizes,
        [5, 5, 6],
        "the invoice model joins its context by its name"
    );

    let two = plan(&repo, &[], 10).unwrap();
    assert_eq!(two.contexts, ["contracts", "invoices"]);
    assert_eq!(two.pending, ["users"]);

    let one = plan(&repo, &[], 7).unwrap();
    assert_eq!(
        one.contexts,
        ["contracts"],
        "the next one would go over the budget"
    );

    let never_cut = plan(&repo, &[], 1).unwrap();
    assert_eq!(
        never_cut.contexts,
        ["contracts"],
        "a context is never cut: at least one is taken"
    );
}

#[test]
fn named_contexts_are_taken_whatever_the_budget_and_an_unknown_name_is_an_error() {
    let repo = repo();

    let named = plan(&repo, &["users", "contracts"], 1).unwrap();
    assert_eq!(
        named.contexts,
        ["contracts", "users"],
        "in the order of the map"
    );
    assert_eq!(named.pending, ["invoices"]);

    let error = plan(&repo, &["billing"], 100).unwrap_err().to_string();
    assert!(
        error.contains("billing") && error.contains("invoices"),
        "{error}"
    );
}

#[test]
fn a_context_is_done_until_one_of_its_files_changes_and_a_done_one_is_skipped() {
    let repo = repo();
    let first = plan(&repo, &[], 5).unwrap();
    assert_eq!(first.contexts, ["contracts"]);

    mark_done(
        repo.dir.path(),
        &repo.map,
        &repo.sources,
        &repo.seeds,
        &repo.roles,
        "contracts",
    )
    .unwrap();

    let next = plan(&repo, &[], 5).unwrap();
    assert_eq!(next.contexts, ["invoices"]);
    assert_eq!(next.done, ["contracts"]);
    let again = plan(&repo, &["contracts"], 5).unwrap();
    assert_eq!(
        again.contexts,
        ["contracts"],
        "a done context can still be named"
    );

    std::fs::write(
        repo.dir.path().join("app/models/contract_part_2.rb"),
        "class changed\nend\n",
    )
    .unwrap();
    let changed = plan(&repo, &[], 5).unwrap();
    assert_eq!(
        changed.contexts,
        ["contracts"],
        "an edited file makes it pending again"
    );
    assert_eq!(changed.done, Vec::<String>::new());
}

#[test]
fn a_file_read_for_a_context_but_outside_it_makes_it_pending_again_when_edited() {
    let repo = repo();
    mark_done(
        repo.dir.path(),
        &repo.map,
        &repo.sources,
        &repo.seeds,
        &repo.roles,
        "contracts",
    )
    .unwrap();
    assert_eq!(plan(&repo, &[], 5).unwrap().done, ["contracts"]);

    // The service no name places in the context, which its controller references.
    std::fs::write(
        repo.dir.path().join("app/services/pdf_renderer.rb"),
        "class PdfRenderer\n  def call\n  end\nend\n",
    )
    .unwrap();

    assert_eq!(plan(&repo, &[], 5).unwrap().done, Vec::<String>::new());
}

#[test]
fn a_context_the_map_no_longer_has_is_forgotten() {
    let mut repo = repo();
    mark_done(
        repo.dir.path(),
        &repo.map,
        &repo.sources,
        &repo.seeds,
        &repo.roles,
        "users",
    )
    .unwrap();
    repo.map.contexts.retain(|c| c.slug != "users");

    mark_done(
        repo.dir.path(),
        &repo.map,
        &repo.sources,
        &repo.seeds,
        &repo.roles,
        "contracts",
    )
    .unwrap();

    let kept: Vec<String> = Fingerprints::load(repo.dir.path())
        .contexts
        .into_keys()
        .collect();
    assert_eq!(kept, ["contracts"]);
}

#[test]
fn the_files_read_are_the_business_ones_and_what_the_entry_points_reference() {
    let repo = repo();

    let slice = plan(&repo, &["contracts"], 5).unwrap();

    let read: Vec<String> = slice
        .files
        .iter()
        .map(|f| f.display().to_string())
        .collect();
    assert!(
        read.contains(&"app/controllers/contracts_controller.rb".to_string()),
        "{read:?}"
    );
    assert!(
        read.contains(&"app/models/contract_part_2.rb".to_string()),
        "{read:?}"
    );
    assert!(
        !read.contains(&"app/models/contract_part_1.rb".to_string()),
        "a view is not read: {read:?}"
    );
    assert!(
        read.contains(&"app/services/pdf_renderer.rb".to_string()),
        "an orphan the controller references is read: {read:?}"
    );
    assert!(
        !read.contains(&"app/models/invoice.rb".to_string()),
        "a file another context owns is read with it: {read:?}"
    );
    assert!(
        !read.contains(&"config/initializers/pdf.rb".to_string()),
        "infra is not read: {read:?}"
    );
}

#[test]
fn an_orphan_two_contexts_reference_is_read_for_the_first_of_them_only() {
    let repo = repo();
    std::fs::write(
        repo.dir
            .path()
            .join("app/controllers/invoices_controller.rb"),
        "class InvoicesController\nPdfRenderer.new\nend\n",
    )
    .unwrap();

    let slice = plan(&repo, &[], 100).unwrap();

    let reads = |slug: &str| -> Vec<String> {
        slice.reads[slug]
            .iter()
            .map(|f| f.display().to_string())
            .collect()
    };
    assert!(reads("contracts").contains(&"app/services/pdf_renderer.rb".to_string()));
    assert!(!reads("invoices").contains(&"app/services/pdf_renderer.rb".to_string()));
}
