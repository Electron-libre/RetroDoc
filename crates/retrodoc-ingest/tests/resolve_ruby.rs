//! The resolution of the references of Ruby files (`specs/resolved-references.md`), through the public API of the
//! crate on temporary repositories.
#![cfg(feature = "ruby")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use retrodoc_ingest::resolve::{resolve, Resolved};

/// Writes `files` (path, content) under a new temporary repository and resolves them all.
fn repo(files: &[(&str, &str)]) -> (tempfile::TempDir, Resolved) {
    let dir = tempfile::tempdir().expect("temp dir");
    for (path, content) in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("dirs");
        std::fs::write(full, content).expect("write");
    }
    let paths: Vec<PathBuf> = files.iter().map(|(p, _)| PathBuf::from(p)).collect();
    let resolved = resolve(dir.path(), &paths);
    (dir, resolved)
}

fn targets(resolved: &Resolved, file: &str) -> BTreeSet<String> {
    resolved
        .targets(Path::new(file))
        .into_iter()
        .map(|p| p.display().to_string())
        .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| (*s).to_string()).collect()
}

// spec: RES-H2, RES-1, RES-6, RES-7, RES-8
#[test]
fn a_controller_resolves_to_its_parent_and_the_models_it_names_not_to_a_file_defining_its_method() {
    let (_dir, resolved) = repo(&[
        (
            "app/controllers/application_controller.rb",
            "class ApplicationController\n  def require_login\n  end\nend\n",
        ),
        (
            "app/models/story.rb",
            "class Story\n  def self.find(id)\n  end\nend\n",
        ),
        (
            "app/models/draft.rb",
            "class Draft\n  def save\n  end\nend\n",
        ),
        (
            "app/controllers/stories_controller.rb",
            "class StoriesController < ApplicationController\n  def update\n    require_login\n    \
             story = Story.find(params[:id])\n    story.save\n  end\nend\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "app/controllers/stories_controller.rb"),
        set(&[
            "app/controllers/application_controller.rb",
            "app/models/story.rb"
        ])
    );
}

// spec: RES-5
#[test]
fn requires_resolve_to_the_files_they_name() {
    let (_dir, resolved) = repo(&[
        ("lib/pricing.rb", "module Pricing\nend\n"),
        ("lib/shop/tax.rb", "module Tax\nend\n"),
        (
            "app/services/checkout.rb",
            "require \"json\"\nrequire_relative \"../../lib/pricing\"\nrequire \"shop/tax\"\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "app/services/checkout.rb"),
        set(&["lib/pricing.rb", "lib/shop/tax.rb"])
    );
}

// spec: RES-6
#[test]
fn a_constant_resolves_from_the_innermost_namespace_outwards() {
    let files = [
        ("app/models/order.rb", "class Order\nend\n"),
        (
            "app/models/shop/order.rb",
            "module Shop\n  class Order\n  end\nend\n",
        ),
        (
            "app/services/shop/checkout.rb",
            "module Shop\n  class Checkout\n    def run\n      Order.new\n    end\n  end\nend\n",
        ),
        (
            "app/services/report.rb",
            "class Report\n  def run\n    Order.new\n    Shop::Order.new\n  end\nend\n",
        ),
    ];
    let (_dir, resolved) = repo(&files);
    assert_eq!(
        targets(&resolved, "app/services/shop/checkout.rb"),
        set(&["app/models/shop/order.rb"])
    );
    assert_eq!(
        targets(&resolved, "app/services/report.rb"),
        set(&["app/models/order.rb", "app/models/shop/order.rb"])
    );
    // With no `Shop::Order`, the top-level one.
    let (_dir, resolved) = repo(&[files[0], files[2]]);
    assert_eq!(
        targets(&resolved, "app/services/shop/checkout.rb"),
        set(&["app/models/order.rb"])
    );
}

// spec: RES-6
#[test]
fn a_class_reopened_in_two_files_resolves_to_both() {
    let (_dir, resolved) = repo(&[
        ("app/models/order.rb", "class Order\nend\n"),
        ("lib/ext/order.rb", "class Order\n  def extra\n  end\nend\n"),
        (
            "app/jobs/sync.rb",
            "class Sync\n  def run\n    Order.all\n  end\nend\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "app/jobs/sync.rb"),
        set(&["app/models/order.rb", "lib/ext/order.rb"])
    );
}

// spec: RES-7
#[test]
fn an_implicit_call_resolves_in_the_class_hierarchy_and_its_modules() {
    let (_dir, resolved) = repo(&[
        (
            "app/models/user.rb",
            "class User\n  include Auditable\n  def save_role\n  end\nend\n",
        ),
        (
            "app/models/group.rb",
            "class Group\n  def save_role\n  end\n  def audit\n  end\nend\n",
        ),
        (
            "app/models/concerns/auditable.rb",
            "module Auditable\n  def audit\n  end\nend\n",
        ),
        (
            "app/models/admin.rb",
            "class Admin < User\n  def promote\n    save_role\n    self.audit\n  end\nend\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "app/models/admin.rb"),
        set(&["app/models/concerns/auditable.rb", "app/models/user.rb"])
    );
}

// spec: RES-8, RES-10
#[test]
fn a_local_named_like_a_method_and_a_rake_task_resolve_to_no_file() {
    let (_dir, resolved) = repo(&[
        ("app/models/base.rb", "class Base\n  include Account\nend\n"),
        (
            "app/models/concerns/account.rb",
            "module Account\n  def user\n  end\nend\n",
        ),
        (
            "app/models/item.rb",
            "class Item < Base\n  def run(user)\n    user.touch\n    owner = nil\n    owner\n  end\nend\n",
        ),
        ("lib/tasks/sync.rake", "task :sync do\n  Base.new\nend\n"),
    ]);
    // `user` is the parameter, not the method `Account` gives `Base`: only the parent class.
    assert_eq!(
        targets(&resolved, "app/models/item.rb"),
        set(&["app/models/base.rb"])
    );
    assert_eq!(targets(&resolved, "lib/tasks/sync.rake"), set(&[]));
}

// spec: RES-7, RES-8
#[test]
fn locals_of_a_multiple_assignment_are_not_calls_and_block_parameters_end_with_their_block() {
    let (_dir, resolved) = repo(&[
        (
            "app/models/base.rb",
            "class Base\n  include Naming\n  include Paging\nend\n",
        ),
        (
            "app/models/concerns/naming.rb",
            "module Naming\n  def first\n  end\nend\n",
        ),
        (
            "app/models/concerns/paging.rb",
            "module Paging\n  def page\n  end\nend\n",
        ),
        (
            "app/models/list.rb",
            "class List < Base\n  def run(pair)\n    first, last = pair\n    first\n    \
             pair.each { |page| page }\n    page\n  end\nend\n",
        ),
    ]);
    // `first` is a local of the multiple assignment; `page` is a block parameter, then a call after the block.
    assert_eq!(
        targets(&resolved, "app/models/list.rb"),
        set(&["app/models/base.rb", "app/models/concerns/paging.rb"])
    );
}

// spec: RES-8
#[test]
fn rescued_errors_loop_variables_and_destructured_parameters_are_locals() {
    let (_dir, resolved) = repo(&[
        ("app/models/base.rb", "class Base\n  include Errors\nend\n"),
        (
            "app/models/concerns/errors.rb",
            "module Errors\n  def error\n  end\n  def order\n  end\n  def item\n  end\nend\n",
        ),
        (
            "app/models/sync.rb",
            "class Sync < Base\n  def run(pairs)\n    pairs.each { |(id, order)| order }\n    \
             for item in pairs\n      item\n    end\n  rescue => error\n    error\n  end\nend\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "app/models/sync.rb"),
        set(&["app/models/base.rb"])
    );
}

// spec: RES-5
#[test]
fn a_require_of_a_path_from_the_current_directory_resolves() {
    let (_dir, resolved) = repo(&[
        ("lib/pricing.rb", "module Pricing\nend\n"),
        ("script/run.rb", "require \"./lib/pricing\"\n"),
    ]);
    assert_eq!(
        targets(&resolved, "script/run.rb"),
        set(&["lib/pricing.rb"])
    );
}
