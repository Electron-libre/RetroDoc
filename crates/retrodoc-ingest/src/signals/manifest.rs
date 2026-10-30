//! Manifest metadata: what the project calls itself and which libraries it
//! stands on, read from the manifests at the root of the repo, the Gradle
//! version catalog and the .NET projects of the root solution.

use std::path::{Component, Path};

use super::{Signal, SignalKind};
use crate::walker::{is_dotnet_test_project, is_test_path, FileEntry};

/// Manifests read at these paths from the repo root.
const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "Gemfile",
    "package.json",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "composer.json",
    "build.gradle",
    "build.gradle.kts",
    "Package.swift",
    "build.sbt",
    GRADLE_CATALOG,
];
/// The Gradle version catalog: a build script that names its dependencies
/// through it (`implementation(libs.spring.web)`) quotes no coordinates.
const GRADLE_CATALOG: &str = "gradle/libs.versions.toml";
/// .NET projects read: a solution of a large product lists dozens.
const MAX_PROJECTS: usize = 20;
/// Dependency names kept per manifest.
const MAX_DEPENDENCIES: usize = 40;

/// One signal per manifest found: those of [`MANIFESTS`], then the .NET
/// projects among the walked `files` (see [`dotnet_projects`]), at most
/// [`MAX_PROJECTS`] of them giving metadata. Each signal holds the name,
/// description and dependency names; a manifest that can't be read or parsed
/// is skipped.
#[must_use]
pub fn manifest_metadata(repo_root: &Path, files: &[FileEntry]) -> Vec<Signal> {
    let mut signals: Vec<Signal> = MANIFESTS
        .iter()
        .filter_map(|name| read_manifest(repo_root, name))
        .collect();
    signals.extend(
        dotnet_projects(repo_root, files)
            .iter()
            .filter_map(|name| read_manifest(repo_root, name))
            .take(MAX_PROJECTS),
    );
    signals
}

/// The walked `.csproj` files, sorted: those at the root, plus those the root
/// solutions (`.sln`, `.slnx`) list, or, when they list none, those one or two
/// levels down. Test projects are left out.
fn dotnet_projects(repo_root: &Path, files: &[FileEntry]) -> Vec<String> {
    let walked = |extension: &str| -> Vec<String> {
        files
            .iter()
            .filter(|f| f.path.extension().is_some_and(|e| e == extension))
            .map(|f| f.path.to_string_lossy().into_owned())
            .collect()
    };
    let projects = walked("csproj");
    let depth = |path: &str| path.matches('/').count();
    let solutions: Vec<String> = ["sln", "slnx"]
        .iter()
        .flat_map(|extension| walked(extension))
        .filter(|path| depth(path) == 0)
        .collect();
    let listed: Vec<String> = solutions
        .iter()
        .filter_map(|solution| std::fs::read(repo_root.join(solution)).ok())
        .flat_map(|bytes| solution_projects(&String::from_utf8_lossy(&bytes)))
        .filter_map(|listed| {
            // A solution written on Windows may not match the case on disk.
            projects
                .iter()
                .find(|p| p.eq_ignore_ascii_case(&listed))
                .cloned()
        })
        .collect();
    let max_depth = if listed.is_empty() { 2 } else { 0 };
    let mut kept: Vec<String> = projects
        .iter()
        .filter(|p| depth(p) <= max_depth)
        .cloned()
        .chain(listed)
        .filter(|p| {
            let path = Path::new(p);
            !is_test_path(path)
                && !path
                    .file_stem()
                    .is_some_and(|stem| is_dotnet_test_project(&stem.to_string_lossy()))
        })
        .collect();
    kept.sort();
    kept.dedup();
    kept
}

/// The `.csproj` paths a solution lists, `/`-separated, those reaching out of
/// its directory left out.
fn solution_projects(content: &str) -> Vec<String> {
    quoted_strings(content, &['"'])
        .into_iter()
        .map(|(_, _, path)| path.replace('\\', "/"))
        .filter(|path| Path::new(path).extension().is_some_and(|e| e == "csproj"))
        .filter_map(|path| {
            let mut parts = Vec::new();
            for component in Path::new(&path).components() {
                match component {
                    Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
                    Component::CurDir => {}
                    _ => return None,
                }
            }
            Some(parts.join("/"))
        })
        .collect()
}

fn read_manifest(repo_root: &Path, name: &str) -> Option<Signal> {
    let bytes = std::fs::read(repo_root.join(name)).ok()?;
    let content = String::from_utf8_lossy(&bytes);
    let Some(summary) = summarize(name, &content) else {
        tracing::debug!("manifest {name} has no usable metadata");
        return None;
    };
    Some(Signal {
        kind: SignalKind::Manifest,
        origin: name.to_string(),
        text: summary.render(),
    })
}

#[derive(Default)]
struct Summary {
    name: Option<String>,
    description: Option<String>,
    dependencies: Vec<String>,
}

impl Summary {
    fn render(&self) -> String {
        let mut lines = Vec::new();
        if let Some(name) = &self.name {
            lines.push(format!("name: {name}"));
        }
        if let Some(description) = &self.description {
            lines.push(format!("description: {description}"));
        }
        if !self.dependencies.is_empty() {
            let kept = self.dependencies.iter().take(MAX_DEPENDENCIES);
            lines.push(format!(
                "dependencies: {}",
                kept.map(String::as_str).collect::<Vec<_>>().join(", ")
            ));
        }
        lines.join("\n")
    }

    fn is_empty(&self) -> bool {
        self.name.is_none() && self.description.is_none() && self.dependencies.is_empty()
    }
}

fn summarize(file: &str, content: &str) -> Option<Summary> {
    let summary = match file {
        "package.json" => json_manifest(content, &["dependencies"]),
        "composer.json" => json_manifest(content, &["require"]),
        "Cargo.toml" => cargo_manifest(content),
        "pyproject.toml" => pyproject_manifest(content),
        "Gemfile" => Some(Summary {
            dependencies: line_values(content, |line| {
                let rest = line.strip_prefix("gem ")?;
                quoted(rest)
            }),
            ..Summary::default()
        }),
        "requirements.txt" => Some(Summary {
            dependencies: content.lines().filter_map(requirement_name).collect(),
            ..Summary::default()
        }),
        "go.mod" => Some(go_manifest(content)),
        "pom.xml" => Some(pom_manifest(content)),
        "build.gradle" | "build.gradle.kts" => Some(gradle_manifest(content)),
        "Package.swift" => Some(swift_manifest(content)),
        "build.sbt" => Some(sbt_manifest(content)),
        GRADLE_CATALOG => gradle_catalog(content),
        _ if file.ends_with(".csproj") => Some(csproj_manifest(file, content)),
        _ => None,
    }?;
    (!summary.is_empty()).then_some(summary)
}

fn json_manifest(content: &str, dependency_keys: &[&str]) -> Option<Summary> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let text = |key: &str| json.get(key)?.as_str().map(str::to_string);
    let mut dependencies = Vec::new();
    for key in dependency_keys {
        if let Some(map) = json.get(key).and_then(|v| v.as_object()) {
            dependencies.extend(map.keys().cloned());
        }
    }
    Some(Summary {
        name: text("name"),
        description: text("description"),
        dependencies,
    })
}

fn cargo_manifest(content: &str) -> Option<Summary> {
    let toml: toml::Value = content.parse().ok()?;
    let package = toml.get("package");
    let text = |key: &str| package?.get(key)?.as_str().map(str::to_string);
    let dependencies = toml
        .get("dependencies")
        .and_then(|d| d.as_table())
        .map(|d| d.keys().cloned().collect())
        .unwrap_or_default();
    Some(Summary {
        name: text("name"),
        description: text("description"),
        dependencies,
    })
}

fn pyproject_manifest(content: &str) -> Option<Summary> {
    let toml: toml::Value = content.parse().ok()?;
    let project = toml.get("project");
    let poetry = toml.get("tool").and_then(|t| t.get("poetry"));
    let text = |key: &str| {
        project
            .and_then(|p| p.get(key))
            .or_else(|| poetry.and_then(|p| p.get(key)))?
            .as_str()
            .map(str::to_string)
    };
    let mut dependencies: Vec<String> = project
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
        .map(|d| {
            d.iter()
                .filter_map(|v| requirement_name(v.as_str()?))
                .collect()
        })
        .unwrap_or_default();
    if let Some(table) = poetry
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_table())
    {
        dependencies.extend(table.keys().filter(|k| *k != "python").cloned());
    }
    Some(Summary {
        name: text("name"),
        description: text("description"),
        dependencies,
    })
}

fn go_manifest(content: &str) -> Summary {
    let mut dependencies = Vec::new();
    let mut in_block = false;
    for line in content
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
    {
        if line.starts_with("require (") {
            in_block = true;
        } else if in_block && line == ")" {
            in_block = false;
        } else if let Some(rest) = line.strip_prefix("require ").filter(|_| !in_block) {
            dependencies.extend(rest.split_whitespace().next().map(str::to_string));
        } else if in_block {
            dependencies.extend(line.split_whitespace().next().map(str::to_string));
        }
    }
    Summary {
        name: content
            .lines()
            .find_map(|l| l.trim().strip_prefix("module "))
            .map(|m| m.trim().to_string()),
        description: None,
        dependencies,
    }
}

/// The text of the first `<name>...</name>` element of `from`.
fn tag(name: &str, from: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = from.find(&open)? + open.len();
    let end = from[start..].find(&format!("</{name}>"))?;
    Some(from[start..start + end].trim().to_string())
}

fn pom_manifest(content: &str) -> Summary {
    // The project's own tags come before its <dependencies> block.
    let (head, tail) = content
        .split_once("<dependencies>")
        .unwrap_or((content, ""));
    // The parent's coordinates are not the project's.
    let head = match (head.find("<parent>"), head.find("</parent>")) {
        (Some(start), Some(end)) if start < end => {
            format!("{}{}", &head[..start], &head[end + "</parent>".len()..])
        }
        _ => head.to_string(),
    };
    let head = head.as_str();
    let mut dependencies = Vec::new();
    let mut rest = tail;
    while let Some(artifact) = tag("artifactId", rest) {
        let at = rest.find("</artifactId>").unwrap_or(rest.len());
        dependencies.push(artifact);
        rest = &rest[(at + "</artifactId>".len()).min(rest.len())..];
    }
    Summary {
        name: tag("name", head).or_else(|| tag("artifactId", head)),
        description: tag("description", head),
        dependencies,
    }
}

/// Gradle scripts are code: the dependencies are the `group:artifact` of the
/// coordinates quoted on a dependency configuration line, test
/// configurations left out.
fn gradle_manifest(content: &str) -> Summary {
    let dependencies = line_values(content, |line| {
        let configuration: String = line
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_lowercase();
        let is_dependency = ["implementation", "api", "compileonly", "runtimeonly"]
            .iter()
            .any(|c| configuration.ends_with(c))
            || matches!(
                configuration.as_str(),
                "compile" | "runtime" | "annotationprocessor" | "kapt" | "ksp"
            );
        if !is_dependency || configuration.contains("test") {
            return None;
        }
        let (_, _, coordinates) = quoted_strings(line, &['"', '\'']).into_iter().next()?;
        let mut parts = coordinates.split(':');
        let (group, artifact) = (parts.next()?, parts.next()?);
        let plain = |p: &str| !p.is_empty() && !p.contains(['/', ' ', '$']);
        (plain(group) && plain(artifact)).then(|| format!("{group}:{artifact}"))
    });
    Summary {
        dependencies,
        ..Summary::default()
    }
}

/// `gradle/libs.versions.toml`: the `group:artifact` of each `[libraries]`
/// entry, written `"group:artifact:version"`, `{ module = "group:artifact" }`
/// or `{ group = "group", name = "artifact" }`. The catalog does not say which
/// configuration uses a library: test libraries are kept.
fn gradle_catalog(content: &str) -> Option<Summary> {
    let toml: toml::Value = content.parse().ok()?;
    let dependencies = toml
        .get("libraries")
        .and_then(|l| l.as_table())
        .map(|libraries| libraries.values().filter_map(catalog_library).collect())
        .unwrap_or_default();
    Some(Summary {
        dependencies,
        ..Summary::default()
    })
}

fn catalog_library(library: &toml::Value) -> Option<String> {
    let text = |key: &str| library.get(key)?.as_str();
    let Some(module) = library.as_str().or_else(|| text("module")) else {
        return Some(format!("{}:{}", text("group")?, text("name")?));
    };
    let mut parts = module.split(':');
    Some(format!("{}:{}", parts.next()?, parts.next()?))
}

/// `Package.swift`: the package name, and the repository name of each
/// `.package(url: ...)` dependency.
fn swift_manifest(content: &str) -> Summary {
    let name = content.find("Package(").and_then(|at| {
        let rest = &content[at..];
        let value = &rest[rest.find("name:")? + "name:".len()..];
        quoted(value.trim_start())
    });
    let dependencies = content
        .match_indices(".package(")
        .filter_map(|(at, _)| {
            let call = &content[at..];
            let call = &call[..call.find(')').unwrap_or(call.len())];
            let url = quoted(call[call.find("url:")? + "url:".len()..].trim_start())?;
            let repo = url.trim_end_matches('/').rsplit('/').next()?;
            Some(repo.trim_end_matches(".git").to_string())
        })
        .collect();
    Summary {
        name,
        description: None,
        dependencies,
    }
}

/// `build.sbt`: `name := "x"`, and the artifact of each `"org" % "artifact"`
/// (or `%%`) dependency.
fn sbt_manifest(content: &str) -> Summary {
    let name = content.lines().map(str::trim).find_map(|line| {
        let rest = line.strip_prefix("name")?.trim_start().strip_prefix(":=")?;
        quoted(rest.trim_start())
    });
    // Scala strings are double-quoted (`'a'` is a character), and a comment
    // could hold an odd quote.
    let content: String = content
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let content = content.as_str();
    let strings = quoted_strings(content, &['"']);
    let is_percent = |between: &str| matches!(between.trim(), "%" | "%%" | "%%%");
    let mut dependencies = Vec::new();
    // `% Test`, `% "test"`, `% IntegrationTest` after the version.
    let is_test_scoped = |after: &str| {
        let mut in_string = false;
        let end = after
            .char_indices()
            .find(|&(_, c)| {
                in_string ^= c == '"';
                !in_string && matches!(c, ',' | ')' | '\n')
            })
            .map_or(after.len(), |(at, _)| at);
        after[..end]
            .split('%')
            .map(str::trim)
            .map(|scope| scope.trim_matches('"'))
            .any(|scope| scope.ends_with("Test") || scope.split(',').any(|s| s.trim() == "test"))
    };
    for (i, pair) in strings.windows(2).enumerate() {
        let (_, org_end, _) = &pair[0];
        let (art_start, art_end, artifact) = &pair[1];
        let follows_percent = i > 0 && is_percent(&content[strings[i - 1].1..strings[i].0]);
        if is_percent(&content[*org_end..*art_start])
            && !follows_percent
            && !is_test_scoped(&content[*art_end..])
        {
            dependencies.push(artifact.clone());
        }
    }
    Summary {
        name,
        description: None,
        dependencies,
    }
}

/// A `.csproj`: the `AssemblyName` (else the file name), the `Description`
/// and the `Include` of each `PackageReference`.
fn csproj_manifest(file: &str, content: &str) -> Summary {
    let marker = "<PackageReference";
    let dependencies = content
        .match_indices(marker)
        .filter_map(|(at, _)| {
            let element = &content[at + marker.len()..];
            let element = &element[..element.find('>').unwrap_or(element.len())];
            // `Include`, not `IncludeAssets`.
            element.match_indices("Include").find_map(|(at, _)| {
                let value = element[at + "Include".len()..].trim_start();
                quoted(value.strip_prefix('=')?.trim_start())
            })
        })
        .collect();
    // `$(MSBuildProjectName)` and the like are MSBuild properties.
    let assembly = tag("AssemblyName", content).filter(|n| !n.contains("$("));
    Summary {
        name: assembly.or_else(|| {
            Path::new(file)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        }),
        description: tag("Description", content),
        dependencies,
    }
}

/// Every string of `text` quoted by one of `quotes`, with the byte offsets
/// of its opening quote and past its closing one.
fn quoted_strings(text: &str, quotes: &[char]) -> Vec<(usize, usize, String)> {
    let mut strings = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find(quotes) {
        let start = from + offset;
        let quote = &text[start..=start];
        let Some(len) = text[start + 1..].find(quote) else {
            break;
        };
        let end = start + 1 + len;
        strings.push((start, end + 1, text[start + 1..end].to_string()));
        from = end + 1;
    }
    strings
}

fn line_values(content: &str, f: impl Fn(&str) -> Option<String>) -> Vec<String> {
    content.lines().filter_map(|l| f(l.trim())).collect()
}

/// The first quoted string (`'x'` or `"x"`) of a line.
fn quoted(text: &str) -> Option<String> {
    let quote = text.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let body = &text[1..];
    body.find(quote).map(|end| body[..end].to_string())
}

/// The package name of a `requirements.txt` / PEP 508 line (`django>=4`).
fn requirement_name(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(['#', '-']) {
        return None;
    }
    let name: String = line
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-'))
        .collect();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn metadata(root: &Path) -> Vec<Signal> {
        manifest_metadata(root, &crate::walker::walk_repo(root, &[]).unwrap())
    }

    fn text_of(file: &str, content: &str) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
        let signals = metadata(dir.path());
        assert_eq!(signals.len(), 1, "{file}");
        assert_eq!(signals[0].origin, file);
        assert_eq!(signals[0].kind, SignalKind::Manifest);
        signals[0].text.clone()
    }

    #[test]
    fn reads_package_json() {
        let text = text_of(
            "package.json",
            r#"{"name":"shop","description":"A web shop","dependencies":{"express":"4","pg":"8"},"devDependencies":{"jest":"29"}}"#,
        );
        assert_eq!(
            text,
            "name: shop\ndescription: A web shop\ndependencies: express, pg"
        );
    }

    #[test]
    fn reads_cargo_and_pyproject() {
        let text = text_of(
            "Cargo.toml",
            "[package]\nname = \"tool\"\ndescription = \"Does things\"\n[dependencies]\nserde = \"1\"\n",
        );
        assert_eq!(
            text,
            "name: tool\ndescription: Does things\ndependencies: serde"
        );
        let text = text_of(
            "pyproject.toml",
            "[project]\nname = \"app\"\ndependencies = [\"django>=4\", \"celery\"]\n",
        );
        assert_eq!(text, "name: app\ndependencies: django, celery");
    }

    #[test]
    fn reads_line_based_manifests() {
        assert_eq!(
            text_of(
                "Gemfile",
                "source 'https://rubygems.org'\ngem 'rails', '~> 7'\ngem \"devise\"\n"
            ),
            "dependencies: rails, devise"
        );
        assert_eq!(
            text_of(
                "requirements.txt",
                "# deps\nflask==2.0\n-r other.txt\nrequests\n"
            ),
            "dependencies: flask, requests"
        );
        assert_eq!(
            text_of(
                "go.mod",
                "module example.com/billing\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9\n)\nrequire github.com/lib/pq v1\n"
            ),
            "name: example.com/billing\ndependencies: github.com/gin-gonic/gin, github.com/lib/pq"
        );
        assert_eq!(
            text_of(
                "go.mod",
                "module m\nrequire (\n\t// indirect\n\ta.com/x v1 // indirect\n)\n"
            ),
            "name: m\ndependencies: a.com/x"
        );
    }

    #[test]
    fn reads_pom_xml() {
        let text = text_of(
            "pom.xml",
            "<project><artifactId>shop</artifactId><name>Shop</name><description>Sells</description><dependencies><dependency><artifactId>spring-web</artifactId></dependency></dependencies></project>",
        );
        assert_eq!(
            text,
            "name: Shop\ndescription: Sells\ndependencies: spring-web"
        );
    }

    #[test]
    fn a_pom_parent_is_not_the_project() {
        let text = text_of(
            "pom.xml",
            "<project><parent><artifactId>starter</artifactId></parent><artifactId>shop</artifactId></project>",
        );
        assert_eq!(text, "name: shop");
    }

    #[test]
    fn reads_gradle_scripts() {
        let groovy = "plugins {\n    id 'org.springframework.boot' version '3.2.0'\n}\n\
            dependencies {\n    implementation 'org.springframework.boot:spring-boot-starter-web'\n\
            runtimeOnly \"org.postgresql:postgresql:42.7\"\n\
            testImplementation 'org.junit:junit:4'\n    // implementation 'a:b'\n\
            androidTestImplementation 'androidx.test:runner:1.5'\n}\n\
            flyway {\n    url = 'jdbc:postgresql://localhost:5432/shop'\n}\n";
        assert_eq!(
            text_of("build.gradle", groovy),
            "dependencies: org.springframework.boot:spring-boot-starter-web, org.postgresql:postgresql"
        );
        let kotlin = "dependencies {\n    implementation(platform(\"io.ktor:ktor-bom:2.3\"))\n\
            api(\"io.ktor:ktor-server-core:$ktor\")\n    implementation(project(\":core\"))\n}\n";
        assert_eq!(
            text_of("build.gradle.kts", kotlin),
            "dependencies: io.ktor:ktor-bom, io.ktor:ktor-server-core"
        );
    }

    #[test]
    fn reads_csproj_under_any_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Shop.Api.csproj"),
            "<Project><PropertyGroup><Description>Sells</Description></PropertyGroup>\
             <ItemGroup><PackageReference Include=\"Serilog\" Version=\"3\" />\
             <PackageReference IncludeAssets=\"all\" Include=\"Dapper\" />\
             <PackageReference Version=\"8\"\n    Include = \"Npgsql\" /></ItemGroup></Project>",
        )
        .unwrap();
        fs::write(
            dir.path().join("Tools.csproj"),
            "<Project><PropertyGroup><AssemblyName>$(MSBuildProjectName)</AssemblyName></PropertyGroup></Project>",
        )
        .unwrap();
        let signals = metadata(dir.path());
        let read: Vec<_> = signals
            .iter()
            .map(|s| (s.origin.as_str(), s.text.as_str()))
            .collect();
        assert_eq!(
            read,
            vec![
                (
                    "Shop.Api.csproj",
                    "name: Shop.Api\ndescription: Sells\ndependencies: Serilog, Dapper, Npgsql"
                ),
                ("Tools.csproj", "name: Tools"),
            ]
        );
    }

    // spec: LNG-11 (internal: the brief prints only a count of manifests, not their dependencies)
    #[test]
    fn reads_the_gradle_version_catalog() {
        let text = text_of(
            "gradle/libs.versions.toml",
            "[versions]\nboot = \"3.2\"\n[libraries]\n\
             web = { module = \"org.springframework.boot:spring-boot-starter-web\", version.ref = \"boot\" }\n\
             pg = { group = \"org.postgresql\", name = \"postgresql\", version = \"42\" }\n\
             flyway = \"org.flywaydb:flyway-core:10.0\"\n\
             broken = { version = \"1\" }\n\
             [plugins]\nboot = { id = \"org.springframework.boot\", version.ref = \"boot\" }\n",
        );
        assert_eq!(
            text,
            "dependencies: org.flywaydb:flyway-core, org.postgresql:postgresql, org.springframework.boot:spring-boot-starter-web"
        );
    }

    // spec: LNG-12 (internal: the brief prints only a count of manifests, not their origins)
    #[test]
    fn reads_the_projects_of_a_solution_inside_the_repo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let csproj =
            "<Project><ItemGroup><PackageReference Include=\"Dapper\" /></ItemGroup></Project>";
        for project in [
            "repo/src/Shop.Api/Shop.Api.csproj",
            "repo/deep/er/Web/Web.csproj",
            "repo/Shop.Tests.csproj",
            "Other/Other.csproj",
        ] {
            let path = dir.path().join(project);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, csproj).unwrap();
        }
        fs::write(
            root.join("Shop.slnx"),
            "<Solution><Project Path=\"./src/shop.api/Shop.Api.csproj\" />\
             <Project Path=\"deep/er/Web/Web.csproj\" />\
             <Project Path=\"../Other/Other.csproj\" /></Solution>",
        )
        .unwrap();
        let origins =
            |root: &Path| -> Vec<String> { metadata(root).into_iter().map(|s| s.origin).collect() };
        assert_eq!(
            origins(&root),
            vec!["deep/er/Web/Web.csproj", "src/Shop.Api/Shop.Api.csproj"]
        );
        let text = metadata(&root).remove(1).text;
        assert_eq!(text, "name: Shop.Api\ndependencies: Dapper");

        // A solution listing no C# project falls back to the shallow ones.
        fs::write(
            root.join("Shop.slnx"),
            b"<Solution><Project Path=\"App/App.fsproj\" /> \xE9</Solution>",
        )
        .unwrap();
        assert_eq!(origins(&root), vec!["src/Shop.Api/Shop.Api.csproj"]);
    }

    #[test]
    fn reads_package_swift() {
        let text = text_of(
            "Package.swift",
            "// swift-tools-version:5.9\nlet package = Package(\n    name: \"Shop\",\n    dependencies: [\n\
             .package(url: \"https://github.com/vapor/vapor.git\", from: \"4.0.0\"),\n\
             .package(url: \"https://github.com/vapor/fluent\", from: \"4.0.0\"),\n    ],\n\
             targets: [.target(name: \"App\")]\n)\n",
        );
        assert_eq!(text, "name: Shop\ndependencies: vapor, fluent");
    }

    #[test]
    fn reads_build_sbt() {
        let text = text_of(
            "build.sbt",
            "name := \"shop\"\n// don't upgrade play\nval sep = ','\nlibraryDependencies ++= Seq(\n\
             guice,\n  \"com.typesafe.play\" %% \"play-slick\" % \"5.1\",\n\
             \"org.postgresql\" % \"postgresql\" % \"42.7\",\n\
             \"org.scalatest\" %% \"scalatest\" % \"3.2\" % Test,\n\
             \"org.mockito\" % \"mockito-core\" % \"5\" % \"test\",\n\
             \"org.h2\" % \"h2\" % \"2\" % \"it,test\"\n)\n",
        );
        assert_eq!(text, "name: shop\ndependencies: play-slick, postgresql");
    }

    #[test]
    fn an_unparseable_or_missing_manifest_gives_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(metadata(dir.path()), vec![]);
        fs::write(dir.path().join("package.json"), "{not json").unwrap();
        assert_eq!(metadata(dir.path()), vec![]);
    }

    #[test]
    fn dependencies_are_capped() {
        let deps: Vec<String> = (0..60).map(|i| format!("\"d{i}\":\"1\"")).collect();
        let text = text_of(
            "package.json",
            &format!("{{\"dependencies\":{{{}}}}}", deps.join(",")),
        );
        assert_eq!(text.matches(", ").count(), MAX_DEPENDENCIES - 1);
    }
}
