//! The built-in stack tables cover the reference languages of `specs/language-agnostic.md`: their
//! files are classified by `scan` with no LLM call.

use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=A", "-c", "user.email=a@example.com"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

/// A committed repo holding `files` (empty content) and a `retrodoc.toml`.
fn repo(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in files {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x\n").unwrap();
    }
    std::fs::write(
        dir.path().join("retrodoc.toml"),
        "[llm]\nmodel = \"m\"\nbase_url = \"http://127.0.0.1:9/chat/completions\"\n",
    )
    .unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);
    dir
}

fn scan(dir: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["scan", "--path"])
        .arg(dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// One file per extension of the reference set.
const SOURCES: &[&str] = &[
    "src/a.js",
    "src/a.jsx",
    "src/a.mjs",
    "src/a.cjs",
    "src/a.ts",
    "src/a.tsx",
    "src/a.mts",
    "src/a.cts",
    "src/a.py",
    "src/a.java",
    "src/a.kt",
    "src/main/kotlin/Order.kts",
    "src/a.cs",
    "src/a.php",
    "src/a.go",
    "src/a.rb",
    "src/a.rs",
    "src/a.c",
    "src/a.h",
    "src/a.cpp",
    "src/a.cc",
    "src/a.hpp",
    "src/a.swift",
    "src/a.scala",
    "src/main/java/shop/Contest.java",
];

// spec: LNG-5
#[test]
fn every_reference_extension_is_source() {
    let mut files = SOURCES.to_vec();
    // Gradle build scripts are manifests, not code of the application.
    files.extend(["build.gradle.kts", "settings.gradle.kts"]);
    let dir = repo(&files);

    let out = scan(dir.path());

    // retrodoc.toml and the two Gradle scripts are the other files.
    let expected = format!(
        "(source: {}, test: 0, markdown: 0, other: 3)",
        SOURCES.len()
    );
    assert!(out.contains(&expected), "{out}");
}

// spec: LNG-6
#[test]
fn every_reference_test_layout_is_test() {
    let tests = [
        "src/test/java/shop/Order.java",
        "src/main/java/shop/OrderTest.java",
        "Shop/OrderTests.cs",
        "pkg/order_test.go",
        "app/OrderTest.php",
        "app/test_order.py",
        "web/order.spec.ts",
        "lib/order_spec.rb",
    ];
    let dir = repo(&tests);

    let out = scan(dir.path());

    let expected = format!("(source: 0, test: {}, markdown: 0, other: 1)", tests.len());
    assert!(out.contains(&expected), "{out}");
}

/// A manifest of each reference language, each naming one dependency.
const MANIFESTS: &[(&str, &str)] = &[
    ("package.json", r#"{"dependencies":{"express":"4"}}"#),
    ("pyproject.toml", "[project]\nname = \"shop\"\n"),
    ("requirements.txt", "django\n"),
    (
        "pom.xml",
        "<project><artifactId>shop</artifactId></project>",
    ),
    (
        "build.gradle",
        "dependencies {\n    implementation 'org.springframework.boot:spring-boot-starter-web:3.2.0'\n}\n",
    ),
    (
        "build.gradle.kts",
        "dependencies {\n    implementation(\"io.ktor:ktor-server-core:2.3.0\")\n}\n",
    ),
    (
        "Shop.Api.csproj",
        "<Project><ItemGroup><PackageReference Include=\"Serilog\" Version=\"3\" /></ItemGroup></Project>",
    ),
    ("composer.json", r#"{"require":{"laravel/framework":"^10"}}"#),
    ("go.mod", "module example.com/shop\n"),
    ("Gemfile", "gem 'rails'\n"),
    ("Cargo.toml", "[package]\nname = \"shop\"\n"),
    (
        "Package.swift",
        "let package = Package(\n    name: \"Shop\",\n    dependencies: [\n        .package(url: \"https://github.com/vapor/vapor.git\", from: \"4.0.0\"),\n    ]\n)\n",
    ),
    (
        "build.sbt",
        "name := \"shop\"\nlibraryDependencies += \"com.typesafe.play\" %% \"play\" % \"2.9.0\"\n",
    ),
];

// spec: LNG-7
#[test]
fn every_reference_manifest_is_read() {
    let dir = repo(&[]);
    for (name, content) in MANIFESTS {
        std::fs::write(dir.path().join(name), content).unwrap();
    }

    assert_eq!(manifest_count(dir.path()), Some(MANIFESTS.len()));
}

/// The number of manifests `brief --signals` reads.
fn manifest_count(dir: &Path) -> Option<usize> {
    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["brief", "--signals", "--path"])
        .arg(dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("Manifest"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|count| count.parse().ok())
}

// spec: LNG-13
#[test]
fn nested_test_projects_and_source_sets_are_test() {
    let tests = [
        "Shop.Tests/Fakes/FakeClock.cs",
        "tests/Shop.UnitTests/Builders/OrderBuilder.cs",
        "ShopIntegrationTests/Db.cs",
        "src/integrationTest/kotlin/OrderIT.kt",
        "app/src/functionalTest/java/shop/Flow.java",
        "src/testFixtures/java/OrderBuilder.java",
    ];
    let sources = [
        "src/main/java/shop/Contests/Draw.java",
        "src/Shop.Api/Order.cs",
    ];
    let dir = repo(&[&tests[..], &sources[..]].concat());

    let out = scan(dir.path());

    let expected = format!(
        "(source: {}, test: {}, markdown: 0, other: 1)",
        sources.len(),
        tests.len()
    );
    assert!(out.contains(&expected), "{out}");
}

// spec: LNG-14
#[test]
fn the_sbt_project_directory_is_not_source() {
    let sbt = repo(&["build.sbt", "project/Dependencies.scala", "app/Shop.scala"]);
    let plain = repo(&["project/Dependencies.scala"]);

    // retrodoc.toml, build.sbt and the build code are the other files.
    let out = scan(sbt.path());
    assert!(
        out.contains("(source: 1, test: 0, markdown: 0, other: 3)"),
        "{out}"
    );
    let out = scan(plain.path());
    assert!(
        out.contains("(source: 1, test: 0, markdown: 0, other: 1)"),
        "{out}"
    );
}

// spec: LNG-11
#[test]
fn a_gradle_version_catalog_is_read() {
    let dir = repo(&[]);
    std::fs::write(
        dir.path().join("build.gradle.kts"),
        "dependencies {\n    implementation(libs.spring.web)\n}\n",
    )
    .unwrap();
    // The build script names no coordinates: only the catalog gives them.
    assert_eq!(manifest_count(dir.path()), None);

    std::fs::create_dir(dir.path().join("gradle")).unwrap();
    std::fs::write(
        dir.path().join("gradle/libs.versions.toml"),
        "[libraries]\nspring-web = { module = \"org.springframework:spring-web\", version = \"6\" }\n",
    )
    .unwrap();
    assert_eq!(manifest_count(dir.path()), Some(1));
}

const CSPROJ: &str =
    "<Project><ItemGroup><PackageReference Include=\"Serilog\" Version=\"3\" /></ItemGroup></Project>";

fn write(dir: &Path, file: &str, content: &str) {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

// spec: LNG-12
#[test]
fn the_projects_of_a_solution_are_read() {
    let dir = repo(&[]);
    for project in [
        "src/Shop.Api/Shop.Api.csproj",
        "tests/Shop.Tests/Shop.Tests.csproj",
        "src/Unlisted/Unlisted.csproj",
    ] {
        write(dir.path(), project, CSPROJ);
    }
    write(
        dir.path(),
        "Shop.sln",
        "Project(\"{FAE04EC0}\") = \"Shop.Api\", \"src\\Shop.Api\\Shop.Api.csproj\", \"{1}\"\n\
         EndProject\n\
         Project(\"{FAE04EC0}\") = \"Shop.Tests\", \"tests\\Shop.Tests\\Shop.Tests.csproj\", \"{2}\"\n\
         EndProject\n",
    );

    // The unlisted project is left out, and so is the test project.
    assert_eq!(manifest_count(dir.path()), Some(1));
}

// spec: LNG-12
#[test]
fn with_no_solution_the_projects_two_levels_down_are_read() {
    let dir = repo(&[]);
    for project in [
        "Shop.Web/Shop.Web.csproj",
        "src/Shop.Api/Shop.Api.csproj",
        "src/Shop.Tests/Shop.Tests.csproj",
        "src/a/Deep/Deep.csproj",
    ] {
        write(dir.path(), project, CSPROJ);
    }

    assert_eq!(manifest_count(dir.path()), Some(2));
}
