//! The human reviews of the docs (`specs/section-review.md`): `.retrodoc/reviews.json` is outside
//! the cache, so no `generate` touches it.

mod support;

use support::{generate, run, App, FakeLlm};

// spec: REV-8
#[test]
fn generate_and_generate_force_keep_the_reviews() {
    let app = App {
        stack: "Ruby",
        source_extensions: vec!["rb"],
        chunk_boundaries: Vec::new(),
        business: "app/",
        entry: "app/orders_controller.rb",
        model: "app/order.rb",
    };
    let llm = FakeLlm::start(move |call| app.answer(call));
    let dir = support::repo(
        &[
            (
                "app/orders_controller.rb",
                "get '/orders' do\n  Order.all\nend\n",
            ),
            ("app/order.rb", "class Order\nend\n"),
        ],
        &llm,
    );
    let reviews = dir.path().join(".retrodoc/reviews.json");
    std::fs::create_dir_all(reviews.parent().unwrap()).unwrap();
    let saved = r#"{
  "version": 1,
  "reviews": [
    {
      "kind": "domain",
      "id": "orders",
      "status": "validated",
      "comment": "Checked with the team.",
      "author": "A",
      "reviewed_at": "2026-11-04T10:00:00Z",
      "fingerprint": "0f"
    }
  ]
}
"#;
    std::fs::write(&reviews, saved).unwrap();

    for output in [
        generate(dir.path()),
        run(dir.path(), &["generate", "--force"]),
    ] {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    assert_eq!(std::fs::read_to_string(&reviews).unwrap(), saved);
}
