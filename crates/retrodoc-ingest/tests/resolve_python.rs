//! The resolution of the references of Python files (`specs/resolved-references.md`), through the public API of the
//! crate on temporary repositories.
#![cfg(feature = "python")]

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

// spec: RES-H1, RES-1, RES-4
#[test]
fn a_view_resolves_to_the_modules_it_imports_and_not_to_a_file_named_like_a_method() {
    let (_dir, resolved) = repo(&[
        ("shop/__init__.py", ""),
        ("shop/models.py", "class Order:\n    pass\n"),
        ("shop/queries.py", "def open_orders():\n    return []\n"),
        ("shop/filter.py", "def filter(items):\n    return items\n"),
        (
            "shop/views.py",
            "from .models import Order\nfrom shop import queries\n\n\
             def index(request):\n    orders = queries.open_orders()\n    \
             return Order.objects.filter(id__in=orders)\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "shop/views.py"),
        set(&["shop/models.py", "shop/queries.py"])
    );
}

// spec: RES-3
#[test]
fn imports_resolve_to_the_file_of_their_module_relative_or_absolute() {
    let (_dir, resolved) = repo(&[
        ("shop/__init__.py", ""),
        ("shop/models.py", "class Order:\n    pass\n"),
        ("shop/orders.py", "def place():\n    pass\n"),
        ("shop/api/__init__.py", ""),
        (
            "shop/api/views.py",
            "import json\nfrom ..models import Order\nfrom shop import orders\n\n\
             def create(data):\n    orders.place()\n    return Order(json.loads(data))\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "shop/api/views.py"),
        set(&["shop/models.py", "shop/orders.py"])
    );
}

// spec: RES-3
#[test]
fn a_name_imported_from_a_package_with_no_module_of_that_name_resolves_to_its_init() {
    let (_dir, resolved) = repo(&[
        ("shop/__init__.py", "def setup():\n    pass\n"),
        ("app.py", "from shop import setup\n\nsetup()\n"),
    ]);
    assert_eq!(targets(&resolved, "app.py"), set(&["shop/__init__.py"]));
}

// spec: RES-3
#[test]
fn a_package_under_a_source_directory_is_imported_by_its_own_name() {
    let (_dir, resolved) = repo(&[
        ("src/shop/__init__.py", ""),
        ("src/shop/orders.py", "def place():\n    pass\n"),
        (
            "src/shop/cli.py",
            "import shop.orders\n\nshop.orders.place()\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "src/shop/cli.py"),
        set(&["src/shop/orders.py"])
    );
}

// spec: RES-4
#[test]
fn an_aliased_import_used_as_a_parent_class_resolves_to_its_module() {
    let (_dir, resolved) = repo(&[
        ("shop/__init__.py", ""),
        ("shop/models.py", "class Order:\n    pass\n"),
        (
            "shop/refunds.py",
            "from shop.models import Order as O\n\nclass Refund(O):\n    pass\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "shop/refunds.py"),
        set(&["shop/models.py"])
    );
}

// spec: RES-7, RES-9
#[test]
fn a_call_on_self_or_on_a_class_resolves_in_the_class_then_its_ancestors() {
    let (_dir, resolved) = repo(&[
        ("shop/__init__.py", ""),
        (
            "shop/base.py",
            "class Base:\n    def helper(self):\n        pass\n\n    @classmethod\n    def find(cls, id):\n        pass\n",
        ),
        (
            "shop/order.py",
            "from shop.base import Base\n\nclass Order(Base):\n    def own(self):\n        pass\n",
        ),
        (
            "shop/special.py",
            "from shop.order import Order\n\nclass Special(Order):\n    def run(self):\n        \
             self.own()\n        self.helper()\n",
        ),
        (
            "shop/own.py",
            "class Lonely:\n    def a(self):\n        self.b()\n\n    def b(self):\n        pass\n",
        ),
        ("shop/lookup.py", "from shop.order import Order\n\nOrder.find(3)\n"),
    ]);
    // `own` is in `Order`, `helper` in `Base`: each resolves to the file of the class that defines it.
    assert_eq!(
        targets(&resolved, "shop/special.py"),
        set(&["shop/base.py", "shop/order.py"])
    );
    // Defined in its own class: the file itself, so no file.
    assert_eq!(targets(&resolved, "shop/own.py"), set(&[]));
    // A class method found in an ancestor: the class's file and the ancestor's.
    assert_eq!(
        targets(&resolved, "shop/lookup.py"),
        set(&["shop/base.py", "shop/order.py"])
    );
}

// spec: RES-8
#[test]
fn a_call_on_an_unknown_receiver_resolves_to_no_file() {
    let (_dir, resolved) = repo(&[
        ("a.py", "def archive():\n    pass\n"),
        ("b.py", "def archive():\n    pass\n"),
        (
            "c.py",
            "class Bookmark:\n    def archive(self):\n        pass\n",
        ),
        (
            "views.py",
            "def run(get_bookmark, id):\n    bookmark = get_bookmark(id)\n    bookmark.archive()\n",
        ),
    ]);
    assert_eq!(targets(&resolved, "views.py"), set(&[]));
}

// spec: RES-10
#[test]
fn a_file_of_a_language_with_no_resolver_gets_no_resolved_reference() {
    let (_dir, resolved) = repo(&[
        (
            "cmd/main.go",
            "package main\n\nfunc main() { RunOrder() }\n",
        ),
        ("cmd/run.go", "package main\n\nfunc RunOrder() {}\n"),
    ]);
    assert_eq!(targets(&resolved, "cmd/main.go"), set(&[]));
}

// spec: RES-11
#[test]
fn the_targets_come_most_referenced_first() {
    let (_dir, resolved) = repo(&[
        ("shop/__init__.py", ""),
        ("shop/models.py", "class Order:\n    pass\n"),
        ("shop/queries.py", "def one():\n    pass\n"),
        (
            "shop/views.py",
            "from shop import models, queries\n\nmodels.Order()\nmodels.Order()\nmodels.Order()\nqueries.one()\n",
        ),
    ]);
    assert_eq!(
        resolved.targets(Path::new("shop/views.py")),
        vec![
            PathBuf::from("shop/models.py"),
            PathBuf::from("shop/queries.py")
        ]
    );
}

// spec: RES-3
#[test]
fn a_repository_with_an_init_at_its_root_still_resolves() {
    let (_dir, resolved) = repo(&[
        ("__init__.py", ""),
        ("models.py", "class Order:\n    pass\n"),
        ("views.py", "from .models import Order\n\nOrder()\n"),
    ]);
    assert_eq!(targets(&resolved, "views.py"), set(&["models.py"]));
}

// spec: RES-9
#[test]
fn a_file_of_a_directory_that_is_no_package_is_not_imported_by_its_bare_name() {
    let (_dir, resolved) = repo(&[
        ("scripts/logging.py", "def log():\n    pass\n"),
        ("worker/models.py", "class Job:\n    pass\n"),
        ("app/models.py", "class Order:\n    pass\n"),
        (
            "worker/tasks.py",
            "import logging\nimport models\n\nlogging.info('x')\nmodels.Job()\n",
        ),
        (
            "worker/report.py",
            "import scripts.logging\n\nscripts.logging.log()\n",
        ),
    ]);
    // `logging` is the standard library, `models` no module of a package: neither names a file of a directory
    // that is no package.
    assert_eq!(targets(&resolved, "worker/tasks.py"), set(&[]));
    // By its path from the root, it is one.
    assert_eq!(
        targets(&resolved, "worker/report.py"),
        set(&["scripts/logging.py"])
    );
}

// spec: RES-3, RES-4
#[test]
fn a_wildcard_import_resolves_to_its_module_and_the_classes_it_brings() {
    let (_dir, resolved) = repo(&[
        ("shop/__init__.py", ""),
        ("shop/models.py", "class Order:\n    pass\n"),
        ("shop/queries.py", "def one():\n    pass\n"),
        (
            "shop/views.py",
            "from .models import *\nfrom shop import queries\n\nqueries.one()\nOrder.objects.all()\n",
        ),
    ]);
    assert_eq!(
        targets(&resolved, "shop/views.py"),
        set(&["shop/models.py", "shop/queries.py"])
    );
}

// spec: RES-3
#[test]
fn a_module_of_a_namespace_subpackage_keeps_the_name_of_its_top_level_package() {
    let (_dir, resolved) = repo(&[
        ("src/shop/__init__.py", ""),
        ("src/shop/models.py", "class Order:\n    pass\n"),
        (
            "src/shop/api/views.py",
            "from ..models import Order\n\nOrder()\n",
        ),
        ("src/cli.py", "import shop.api.views\n"),
    ]);
    assert_eq!(
        targets(&resolved, "src/shop/api/views.py"),
        set(&["src/shop/models.py"])
    );
    assert_eq!(
        targets(&resolved, "src/cli.py"),
        set(&["src/shop/api/views.py"])
    );
}
