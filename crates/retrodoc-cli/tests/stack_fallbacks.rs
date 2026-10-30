//! Each reader of one stack (the schema readers, the manifest readers, the grammars) has a generic
//! fallback: its pass still gives its output on a repository with no file that reader knows.

mod support;

use support::{generate, repo, App, FakeLlm};

fn cache(dir: &tempfile::TempDir, file: &str) -> String {
    std::fs::read_to_string(dir.path().join(".retrodoc/cache").join(file)).unwrap_or_default()
}

// spec: LNG-10
#[test]
fn the_glossary_is_given_from_the_business_files_with_a_schema_no_reader_knows() {
    let app = App {
        stack: "TypeScript, Express, Prisma",
        source_extensions: vec!["ts"],
        chunk_boundaries: Vec::new(),
        business: "src/",
        entry: "src/routes/orders.ts",
        model: "src/orders/order.ts",
    };
    let llm = FakeLlm::start(move |call| app.answer(call));
    let dir = repo(
        &[
            ("package.json", r#"{"name": "shop", "dependencies": {"@prisma/client": "5"}}"#),
            ("prisma/schema.prisma", "model Order {\n  id    Int @id\n  total Int\n}\n"),
            (
                "src/routes/orders.ts",
                "router.get('/orders', async (req, res) => res.json(await prisma.order.findMany()));\n",
            ),
            ("src/orders/order.ts", "export interface Order {\n  total: number;\n}\n"),
        ],
        &llm,
    );

    let output = generate(dir.path());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "{stdout}");
    assert!(cache(&dir, "glossary.yaml").contains("Order"), "{stdout}");
}

// spec: LNG-10
#[test]
fn the_brief_and_the_entry_points_need_no_known_manifest_nor_grammar() {
    let app = App {
        stack: "Clojure, Ring",
        source_extensions: vec!["clj"],
        chunk_boundaries: Vec::new(),
        business: "src/",
        entry: "src/shop/handler.clj",
        model: "src/shop/order.clj",
    };
    let llm = FakeLlm::start(move |call| app.answer(call));
    let dir = repo(
        &[
            (
                "deps.edn",
                "{:deps {ring/ring-core {:mvn/version \"1.12.0\"}}}\n",
            ),
            ("package.json", "{ this is not json"),
            (
                "src/shop/handler.clj",
                "(ns shop.handler)\n\n(defn orders [request]\n  {:status 200 :body []})\n",
            ),
            (
                "src/shop/order.clj",
                "(ns shop.order)\n\n(defrecord Order [total])\n",
            ),
        ],
        &llm,
    );

    let output = generate(dir.path());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "{stdout}");
    assert!(
        cache(&dir, "product.yaml").contains("Sells products to customers."),
        "{stdout}"
    );
    assert!(
        cache(&dir, "entry-points.yaml").contains("GET /orders"),
        "{stdout}"
    );
}
