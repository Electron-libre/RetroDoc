//! A minimal application of each framework of the reference set of `specs/language-agnostic.md`
//! (one entry point, one model, one test) goes through `generate` with a fake LLM.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use support::{generate, repo, warnings, App, FakeLlm};

/// A minimal application: its name, what the fake LLM says of it, its files.
struct Fixture {
    framework: &'static str,
    app: App,
    files: Vec<(&'static str, &'static str)>,
}

/// `stack`, the source extension, the business folder, then the manifest, entry point, model and test
/// files with their text.
fn fixture(
    framework: &'static str,
    (stack, extension, business): (&'static str, &'static str, &'static str),
    files: [(&'static str, &'static str); 4],
) -> Fixture {
    Fixture {
        framework,
        app: App {
            stack,
            source_extensions: vec![extension],
            chunk_boundaries: Vec::new(),
            business,
            entry: files[1].0,
            model: files[2].0,
        },
        files: files.to_vec(),
    }
}

const PACKAGE_JSON: &str = r#"{"name": "shop", "version": "1.0.0", "description": "A shop"}"#;
const GO_MOD: &str = "module example.com/shop\n\ngo 1.22\n";

#[allow(clippy::too_many_lines)] // one fixture per framework
fn fixtures() -> Vec<Fixture> {
    vec![
        fixture("Express", ("JavaScript, Express", "js", "src/"), [
            ("package.json", PACKAGE_JSON),
            ("src/routes/orders.js", "const router = require('express').Router();\nrouter.get('/orders', (req, res) => res.json([]));\nmodule.exports = router;\n"),
            ("src/models/order.js", "class Order {\n  constructor(total) { this.total = total; }\n}\nmodule.exports = Order;\n"),
            ("test/orders.test.js", "test('lists the orders', () => {});\n"),
        ]),
        fixture("NestJS", ("TypeScript, NestJS", "ts", "src/"), [
            ("package.json", PACKAGE_JSON),
            ("src/orders/orders.controller.ts", "@Controller('orders')\nexport class OrdersController {\n  @Get()\n  findAll() { return []; }\n}\n"),
            ("src/orders/order.entity.ts", "export class Order {\n  total: number;\n}\n"),
            ("src/orders/orders.controller.spec.ts", "describe('OrdersController', () => {});\n"),
        ]),
        fixture("Next.js", ("TypeScript, Next.js", "ts", "src/"), [
            ("package.json", PACKAGE_JSON),
            ("src/app/api/orders/route.ts", "export async function GET() {\n  return Response.json([]);\n}\n"),
            ("src/lib/order.ts", "export interface Order {\n  total: number;\n}\n"),
            ("src/lib/order.test.ts", "test('order', () => {});\n"),
        ]),
        fixture("Django", ("Python, Django", "py", "shop/"), [
            ("requirements.txt", "Django==5.0\n"),
            ("shop/views.py", "from django.http import JsonResponse\n\ndef orders(request):\n    return JsonResponse([], safe=False)\n"),
            ("shop/models.py", "from django.db import models\n\nclass Order(models.Model):\n    total = models.IntegerField()\n"),
            ("shop/tests/test_views.py", "def test_orders():\n    pass\n"),
        ]),
        fixture("Flask", ("Python, Flask", "py", "app/"), [
            ("pyproject.toml", "[project]\nname = \"shop\"\ndependencies = [\"flask\"]\n"),
            ("app/routes.py", "from flask import Flask\n\napp = Flask(__name__)\n\n@app.get(\"/orders\")\ndef orders():\n    return []\n"),
            ("app/models.py", "class Order:\n    def __init__(self, total):\n        self.total = total\n"),
            ("tests/test_routes.py", "def test_orders():\n    pass\n"),
        ]),
        fixture("FastAPI", ("Python, FastAPI", "py", "app/"), [
            ("pyproject.toml", "[project]\nname = \"shop\"\ndependencies = [\"fastapi\"]\n"),
            ("app/main.py", "from fastapi import FastAPI\n\napp = FastAPI()\n\n@app.get(\"/orders\")\ndef orders():\n    return []\n"),
            ("app/models.py", "from pydantic import BaseModel\n\nclass Order(BaseModel):\n    total: int\n"),
            ("tests/test_main.py", "def test_orders():\n    pass\n"),
        ]),
        fixture("Spring Boot (Java)", ("Java, Spring Boot", "java", "src/main/java/"), [
            ("pom.xml", "<project><groupId>com.example</groupId><artifactId>shop</artifactId></project>\n"),
            ("src/main/java/shop/OrderController.java", "@RestController\npublic class OrderController {\n  @GetMapping(\"/orders\")\n  public List<Order> list() { return List.of(); }\n}\n"),
            ("src/main/java/shop/Order.java", "@Entity\npublic class Order {\n  private long total;\n}\n"),
            ("src/test/java/shop/OrderControllerTest.java", "class OrderControllerTest {\n  @Test void lists() {}\n}\n"),
        ]),
        fixture("Spring Boot (Kotlin)", ("Kotlin, Spring Boot", "kt", "src/main/kotlin/"), [
            ("build.gradle.kts", "plugins {\n  id(\"org.springframework.boot\") version \"3.3.0\"\n}\n"),
            ("src/main/kotlin/shop/OrderController.kt", "@RestController\nclass OrderController {\n  @GetMapping(\"/orders\")\n  fun list(): List<Order> = listOf()\n}\n"),
            ("src/main/kotlin/shop/Order.kt", "@Entity\nclass Order(val total: Long)\n"),
            ("src/test/kotlin/shop/OrderControllerTest.kt", "class OrderControllerTest {\n  @Test fun lists() {}\n}\n"),
        ]),
        fixture("Ktor", ("Kotlin, Ktor", "kt", "src/main/kotlin/"), [
            ("build.gradle.kts", "plugins {\n  id(\"io.ktor.plugin\") version \"2.3.0\"\n}\n"),
            ("src/main/kotlin/shop/Routing.kt", "fun Application.routing() {\n  routing {\n    get(\"/orders\") { call.respond(listOf<Order>()) }\n  }\n}\n"),
            ("src/main/kotlin/shop/Order.kt", "data class Order(val total: Long)\n"),
            ("src/test/kotlin/shop/RoutingTest.kt", "class RoutingTest {\n  @Test fun lists() {}\n}\n"),
        ]),
        fixture("ASP.NET Core", ("C#, ASP.NET Core", "cs", "Shop/"), [
            ("Shop.csproj", "<Project Sdk=\"Microsoft.NET.Sdk.Web\"><PropertyGroup><AssemblyName>Shop</AssemblyName></PropertyGroup></Project>\n"),
            ("Shop/Controllers/OrdersController.cs", "[ApiController]\n[Route(\"orders\")]\npublic class OrdersController : ControllerBase {\n  [HttpGet] public IEnumerable<Order> List() => new List<Order>();\n}\n"),
            ("Shop/Models/Order.cs", "public class Order {\n  public long Total { get; set; }\n}\n"),
            ("Shop.Tests/OrdersControllerTests.cs", "public class OrdersControllerTests {\n  [Fact] public void Lists() {}\n}\n"),
        ]),
        fixture("Laravel", ("PHP, Laravel", "php", "app/"), [
            ("composer.json", r#"{"name": "example/shop", "require": {"laravel/framework": "^11.0"}}"#),
            ("app/Http/Controllers/OrderController.php", "<?php\nclass OrderController extends Controller {\n    public function index() { return Order::all(); }\n}\n"),
            ("app/Models/Order.php", "<?php\nclass Order extends Model {\n    protected $fillable = ['total'];\n}\n"),
            ("tests/Feature/OrderTest.php", "<?php\nclass OrderTest extends TestCase {\n    public function test_lists() {}\n}\n"),
        ]),
        fixture("Symfony", ("PHP, Symfony", "php", "src/"), [
            ("composer.json", r#"{"name": "example/shop", "require": {"symfony/framework-bundle": "^7.0"}}"#),
            ("src/Controller/OrderController.php", "<?php\nclass OrderController extends AbstractController {\n    #[Route('/orders')]\n    public function list(): Response { return $this->json([]); }\n}\n"),
            ("src/Entity/Order.php", "<?php\n#[ORM\\Entity]\nclass Order {\n    private int $total;\n}\n"),
            ("tests/Controller/OrderControllerTest.php", "<?php\nclass OrderControllerTest extends WebTestCase {\n    public function testList() {}\n}\n"),
        ]),
        fixture("net/http", ("Go, net/http", "go", "internal/"), [
            ("go.mod", GO_MOD),
            ("internal/shop/handler.go", "package shop\n\nfunc Orders(w http.ResponseWriter, r *http.Request) {\n\tjson.NewEncoder(w).Encode([]Order{})\n}\n"),
            ("internal/shop/order.go", "package shop\n\ntype Order struct {\n\tTotal int\n}\n"),
            ("internal/shop/handler_test.go", "package shop\n\nfunc TestOrders(t *testing.T) {}\n"),
        ]),
        fixture("Gin", ("Go, Gin", "go", "internal/"), [
            ("go.mod", "module example.com/shop\n\ngo 1.22\n\nrequire github.com/gin-gonic/gin v1.10.0\n"),
            ("internal/shop/routes.go", "package shop\n\nfunc Routes(r *gin.Engine) {\n\tr.GET(\"/orders\", func(c *gin.Context) { c.JSON(200, []Order{}) })\n}\n"),
            ("internal/shop/order.go", "package shop\n\ntype Order struct {\n\tTotal int\n}\n"),
            ("internal/shop/routes_test.go", "package shop\n\nfunc TestRoutes(t *testing.T) {}\n"),
        ]),
        fixture("Rails", ("Ruby, Rails", "rb", "app/"), [
            ("Gemfile", "source \"https://rubygems.org\"\ngem \"rails\", \"~> 7.1\"\n"),
            ("app/controllers/orders_controller.rb", "class OrdersController < ApplicationController\n  def index\n    render json: Order.all\n  end\nend\n"),
            ("app/models/order.rb", "class Order < ApplicationRecord\n  validates :total, presence: true\nend\n"),
            ("spec/models/order_spec.rb", "RSpec.describe Order do\n  it \"lists\" do\n  end\nend\n"),
        ]),
        fixture("Axum", ("Rust, Axum", "rs", "src/"), [
            ("Cargo.toml", "[package]\nname = \"shop\"\nversion = \"0.1.0\"\n\n[dependencies]\naxum = \"0.7\"\n"),
            ("src/main.rs", "async fn orders() -> Json<Vec<Order>> {\n    Json(Vec::new())\n}\n\nfn app() -> Router {\n    Router::new().route(\"/orders\", get(orders))\n}\n"),
            ("src/order.rs", "pub struct Order {\n    pub total: u64,\n}\n"),
            ("tests/orders.rs", "#[test]\nfn lists() {}\n"),
        ]),
        fixture("Actix", ("Rust, Actix", "rs", "src/"), [
            ("Cargo.toml", "[package]\nname = \"shop\"\nversion = \"0.1.0\"\n\n[dependencies]\nactix-web = \"4\"\n"),
            ("src/main.rs", "#[get(\"/orders\")]\nasync fn orders() -> impl Responder {\n    web::Json(Vec::<Order>::new())\n}\n"),
            ("src/order.rs", "pub struct Order {\n    pub total: u64,\n}\n"),
            ("tests/orders.rs", "#[test]\nfn lists() {}\n"),
        ]),
        fixture("Vapor", ("Swift, Vapor", "swift", "Sources/"), [
            ("Package.swift", "// swift-tools-version:5.9\nimport PackageDescription\nlet package = Package(name: \"Shop\")\n"),
            ("Sources/App/Controllers/OrderController.swift", "struct OrderController: RouteCollection {\n    func boot(routes: RoutesBuilder) throws {\n        routes.get(\"orders\") { _ in [Order]() }\n    }\n}\n"),
            ("Sources/App/Models/Order.swift", "final class Order: Model {\n    @Field(key: \"total\") var total: Int\n}\n"),
            ("Tests/AppTests/OrderControllerTests.swift", "final class OrderControllerTests: XCTestCase {\n    func testList() {}\n}\n"),
        ]),
        fixture("Play", ("Scala, Play", "scala", "app/"), [
            ("build.sbt", "name := \"shop\"\nversion := \"1.0\"\nlazy val root = (project in file(\".\")).enablePlugins(PlayScala)\n"),
            ("app/controllers/OrderController.scala", "class OrderController @Inject()(cc: ControllerComponents) extends AbstractController(cc) {\n  def list = Action { Ok(Json.toJson(Seq.empty[Order])) }\n}\n"),
            ("app/models/Order.scala", "case class Order(total: Long)\n"),
            ("test/controllers/OrderControllerSpec.scala", "class OrderControllerSpec extends PlaySpec {\n  \"list\" in {}\n}\n"),
        ]),
    ]
}

/// The run of `generate` on a fixture: its output and the requests of the fake LLM.
struct Run {
    framework: &'static str,
    output: std::process::Output,
    llm: FakeLlm,
}

fn run(fixture: Fixture) -> Run {
    let app = fixture.app;
    let llm = FakeLlm::start(move |call| app.answer(call));
    let dir = repo(&fixture.files, &llm);
    let output = generate(dir.path());
    Run {
        framework: fixture.framework,
        output,
        llm,
    }
}

/// Runs the fixtures of `frameworks` (all of them when empty) side by side.
fn run_all(frameworks: &[&str]) -> Vec<Run> {
    let picked = fixtures()
        .into_iter()
        .filter(|f| frameworks.is_empty() || frameworks.contains(&f.framework));
    let threads: Vec<_> = picked.map(|f| std::thread::spawn(move || run(f))).collect();
    threads.into_iter().map(|t| t.join().unwrap()).collect()
}

// spec: LNG-9
#[test]
fn each_reference_framework_goes_through_generate_with_no_warning() {
    let runs = run_all(&[]);
    assert_eq!(runs.len(), 19);

    let failures: Vec<String> = runs
        .iter()
        .filter_map(|run| {
            let warned = warnings(&run.output);
            let documented = run.llm.calls_of("use-cases").len();
            (!run.output.status.success() || !warned.is_empty() || documented == 0).then(|| {
                format!(
                    "{}: {}, {documented} use-cases call(s), warnings {warned:#?}\n{}",
                    run.framework,
                    run.output.status,
                    String::from_utf8_lossy(&run.output.stderr)
                )
            })
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The system prompts of a run, pass by pass.
fn instructions(run: &Run) -> BTreeMap<String, BTreeSet<String>> {
    let mut by_pass: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for call in run.llm.calls() {
        by_pass.entry(call.pass).or_default().insert(call.system);
    }
    by_pass
}

// spec: LNG-4
#[test]
fn the_instructions_are_the_same_text_for_every_stack() {
    let runs = run_all(&["Django", "Spring Boot (Java)"]);
    let (django, spring) = (instructions(&runs[0]), instructions(&runs[1]));

    assert!(django.len() > 5, "{:?}", django.keys());
    assert_eq!(django, spring);
}
