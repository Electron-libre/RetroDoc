//! The code a use case is derived from follows the resolved references of its entry point
//! (`specs/resolved-references.md`): `generate` with a fake LLM, the files of the use-cases prompt read.

mod support;

use support::{generate, repo, App, FakeLlm};

/// The paths of the files the use-cases prompts show (`=== [F1] path ===`) after a `generate` of `files`, the
/// fake LLM finding one entry point in `entry` and the entity of `model`.
fn files_of_use_cases(
    (stack, extension, business): (&'static str, &'static str, &'static str),
    (entry, model): (&'static str, &'static str),
    files: &[(&str, &str)],
) -> Vec<String> {
    let app = App {
        stack,
        source_extensions: vec![extension],
        chunk_boundaries: Vec::new(),
        business,
        entry,
        model,
    };
    let llm = FakeLlm::start(move |call| app.answer(call));
    let dir = repo(files, &llm);
    let output = generate(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let calls = llm.calls_of("use-cases");
    assert!(!calls.is_empty(), "no use-cases call");
    calls
        .iter()
        .flat_map(|call| call.user.lines().map(str::to_string).collect::<Vec<_>>())
        .filter_map(|line| {
            let rest = line.strip_prefix("=== [F")?;
            let (_, path) = rest.split_once("] ")?;
            Some(path.strip_suffix(" ===")?.to_string())
        })
        .collect()
}

const DJANGO: (&str, &str, &str) = ("Python, Django", "py", "shop/");

// spec: RES-H1
#[test]
fn a_django_view_is_read_with_the_modules_it_imports_and_not_a_file_named_like_a_method() {
    let files = files_of_use_cases(
        DJANGO,
        ("shop/views.py", "shop/models.py"),
        &[
            ("requirements.txt", "django\n"),
            ("shop/__init__.py", ""),
            ("shop/models.py", "class Order:\n    total = 0\n"),
            ("shop/queries.py", "def open_orders():\n    return []\n"),
            ("shop/filter.py", "def filter(items):\n    return items\n"),
            (
                "shop/views.py",
                "from .models import Order\nfrom shop import queries\n\n\
                 def index(request):\n    orders = queries.open_orders()\n    \
                 return Order.objects.filter(id__in=orders)\n",
            ),
        ],
    );
    assert!(files.contains(&"shop/queries.py".to_string()), "{files:?}");
    assert!(files.contains(&"shop/models.py".to_string()), "{files:?}");
    assert!(!files.contains(&"shop/filter.py".to_string()), "{files:?}");
}

// spec: RES-H2
#[test]
fn a_rails_controller_is_read_with_its_parent_and_its_models_and_not_a_file_named_like_a_method() {
    let files = files_of_use_cases(
        ("Ruby, Rails", "rb", "app/"),
        (
            "app/controllers/stories_controller.rb",
            "app/models/story.rb",
        ),
        &[
            ("Gemfile", "source \"https://rubygems.org\"\ngem \"rails\"\n"),
            (
                "app/controllers/application_controller.rb",
                "class ApplicationController\n  def require_login\n  end\nend\n",
            ),
            (
                "app/models/story.rb",
                "class Story\n  def self.find(id)\n  end\nend\n",
            ),
            (
                "app/services/save.rb",
                "class Saver\n  def save\n  end\nend\n",
            ),
            (
                "app/controllers/stories_controller.rb",
                "class StoriesController < ApplicationController\n  def update\n    require_login\n    \
                 story = Story.find(params[:id])\n    story.save\n  end\nend\n",
            ),
        ],
    );
    assert!(
        files.contains(&"app/controllers/application_controller.rb".to_string()),
        "{files:?}"
    );
    assert!(
        files.contains(&"app/models/story.rb".to_string()),
        "{files:?}"
    );
    assert!(
        !files.contains(&"app/services/save.rb".to_string()),
        "{files:?}"
    );
}

// spec: RES-H3
#[test]
fn a_command_linked_by_a_string_only_is_read_with_its_lexical_slice() {
    let files = files_of_use_cases(
        DJANGO,
        ("shop/commands/sync.py", "shop/models.py"),
        &[
            ("requirements.txt", "django\n"),
            ("shop/models.py", "class Order:\n    total = 0\n"),
            ("shop/payment.py", "def pay():\n    pass\n"),
            (
                "shop/commands/sync.py",
                "def run(call_command):\n    call_command('payment')\n",
            ),
        ],
    );
    assert!(files.contains(&"shop/payment.py".to_string()), "{files:?}");
}
