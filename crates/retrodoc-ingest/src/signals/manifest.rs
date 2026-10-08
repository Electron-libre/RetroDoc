//! Manifest metadata: what the project calls itself and which libraries it
//! stands on, read from the manifests at the root of the repo.

use std::path::Path;

use super::{Signal, SignalKind};

/// Manifests read at the repo root.
const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "Gemfile",
    "package.json",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "composer.json",
];
/// Dependency names kept per manifest.
const MAX_DEPENDENCIES: usize = 40;

/// One signal per manifest found at the root of the repo: name, description
/// and dependency names. A manifest that can't be read or parsed is skipped.
#[must_use]
pub fn manifest_metadata(repo_root: &Path) -> Vec<Signal> {
    let mut signals = Vec::new();
    for name in MANIFESTS {
        let Ok(bytes) = std::fs::read(repo_root.join(name)) else {
            continue;
        };
        let content = String::from_utf8_lossy(&bytes);
        let Some(summary) = summarize(name, &content) else {
            tracing::debug!("manifest {name} has no usable metadata");
            continue;
        };
        signals.push(Signal {
            kind: SignalKind::Manifest,
            origin: (*name).to_string(),
            text: summary.render(),
        });
    }
    signals
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

fn pom_manifest(content: &str) -> Summary {
    let tag = |name: &str, from: &str| {
        let open = format!("<{name}>");
        let start = from.find(&open)? + open.len();
        let end = from[start..].find(&format!("</{name}>"))?;
        Some(from[start..start + end].trim().to_string())
    };
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

    fn text_of(file: &str, content: &str) -> String {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(file), content).unwrap();
        let signals = manifest_metadata(dir.path());
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
    fn an_unparseable_or_missing_manifest_gives_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(manifest_metadata(dir.path()), vec![]);
        fs::write(dir.path().join("package.json"), "{not json").unwrap();
        assert_eq!(manifest_metadata(dir.path()), vec![]);
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
