#!/usr/bin/env rust-script
//! Validate `specs/*.md` against `specs/README.md` and trace them to the tests (`// spec: ID`).
//! Usage: `just check-specs` (from the repo root; an optional argument is another root; `--strict` also flags the
//! citations of managed tests (under `.claude/`) whose spec the project lacks: for the repository that ships them).
//! Prints one line per problem, exit 1 if any.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const SECTIONS: [&str; 4] = ["## Intent", "## Requirements", "## Scenarios", "## Non-goals"];

struct Requirement {
    id: String,
    /// Needs no test: withdrawn, or of the former `quality` kind.
    exempt: bool,
}

struct Spec {
    file: String,
    id: String,
    status: String,
    requirements: Vec<Requirement>,
}

fn is_id_prefix(s: &str) -> bool {
    s.len() == 3 && s.chars().all(|c| c.is_ascii_uppercase())
}

fn is_req_id(s: &str) -> bool {
    s.split_once('-')
        .is_some_and(|(p, n)| is_id_prefix(p) && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

fn parse(root: &Path, file: &str, text: &str, out: &mut Vec<String>) -> Spec {
    let mut problem = |m: String| out.push(format!("{file}: {m}"));
    let mut spec = Spec { file: file.to_string(), id: String::new(), status: String::new(), requirements: Vec::new() };
    let lines: Vec<&str> = text.lines().collect();
    let close = lines.iter().skip(1).position(|l| l.trim() == "---").map(|p| p + 1);
    if lines.first().map(|l| l.trim()) != Some("---") || close.is_none() {
        problem("no front matter (`---` ... `---`)".to_string());
        return spec;
    }
    let close = close.unwrap_or(0);
    for line in &lines[1..close] {
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.split('#').next().unwrap_or("").trim();
        match key.trim() {
            "id" => spec.id = value.to_string(),
            "status" => spec.status = value.to_string(),
            "adrs" => {
                for n in value.trim_matches(|c| c == '[' || c == ']').split(',').map(str::trim).filter(|n| !n.is_empty()) {
                    let found = std::fs::read_dir(root.join("docs/adr"))
                        .into_iter()
                        .flatten()
                        .flatten()
                        .any(|e| e.file_name().to_string_lossy().starts_with(&format!("{n}-")));
                    if !found {
                        problem(format!("adr {n} not found in docs/adr"));
                    }
                }
            }
            _ => {}
        }
    }
    if !is_id_prefix(&spec.id) {
        problem(format!("front matter `id` must be three capital letters, got `{}`", spec.id));
    }
    // `implemented` was a status of its own, enforced like `accepted`: still read as such so older specs keep passing
    if spec.status == "implemented" {
        eprintln!("warning: {file}: status `implemented` is deprecated, use `accepted`");
        spec.status = "accepted".to_string();
    }
    if !["draft", "accepted"].contains(&spec.status.as_str()) {
        problem(format!("front matter `status` must be draft or accepted, got `{}`", spec.status));
    }
    for section in SECTIONS {
        if !lines.iter().any(|l| l.trim() == section) {
            problem(format!("missing section `{section}`"));
        }
    }
    let mut seen = BTreeSet::new();
    let mut in_requirements = false;
    for line in &lines {
        if line.starts_with("## ") {
            in_requirements = line.trim() == "## Requirements";
        }
        if !in_requirements || !line.starts_with("- ") {
            continue;
        }
        let parsed = line.strip_prefix("- **").and_then(|r| r.split_once("**")).filter(|(id, _)| is_req_id(id));
        let Some((id, after)) = parsed else {
            problem(format!("requirement line must start with `- **{}-<n>**`: `{line}`", spec.id));
            continue;
        };
        if !id.starts_with(&format!("{}-", spec.id)) {
            problem(format!("requirement {id} does not start with the spec id `{}`", spec.id));
        }
        if !seen.insert(id.to_string()) {
            problem(format!("duplicate requirement {id}"));
        }
        let kind = after.trim().strip_prefix('(').and_then(|a| a.split_once(')'));
        let (kind, statement) = kind.map_or((String::new(), ""), |(k, rest)| (k.to_string(), rest.trim()));
        let withdrawn = statement.starts_with("withdrawn:");
        // `quality` was a kind proved by a benchmark: still read, untraced, so older specs keep passing
        if kind == "quality" && !withdrawn {
            eprintln!("warning: {file}: requirement {id}: kind `quality` is deprecated, reword it for a test or withdraw it");
        } else if kind != "deterministic" && kind != "quality" {
            problem(format!("requirement {id}: kind must be `(deterministic)`, got `({kind})`"));
        }
        spec.requirements.push(Requirement { id: id.to_string(), exempt: withdrawn || kind == "quality" });
    }
    spec
}

/// Every `.rs` file under `dir`, except under the `skipped` directories (build output and agent worktrees, which hold
/// no tests of their own): matched by full path, so a module named `target` or `worktrees` is still scanned.
fn rust_files(dir: &Path, skipped: &[PathBuf], found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if !skipped.contains(&path) {
                rust_files(&path, skipped, found);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// Ids cited by `// spec: A-1, B-2` comments, with the file that cites them. A citation sits right above a
/// test: only attributes and other comments may come between it and the `fn`; any other place is a problem.
fn citations(root: &Path, out: &mut Vec<String>) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    // the tests of the crates, and those of the harness scripts, hooks and skills
    let skipped = [root.join("target"), root.join(".claude/worktrees")];
    for dir in ["crates", ".claude"] {
        rust_files(&root.join(dir), &skipped, &mut files);
    }
    let mut cited = BTreeMap::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let name = file.strip_prefix(root).unwrap_or(&file).display().to_string();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let Some(list) = line.trim().strip_prefix("// spec:") else { continue };
            let next = lines[i + 1..].iter().map(|l| l.trim()).find(|l| !l.starts_with("#[") && !l.starts_with("//"));
            if !next.is_some_and(|l| l.contains("fn ")) {
                out.push(format!("{name}:{}: `// spec:` must sit right above a test `fn`", i + 1));
            }
            for id in list.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()) {
                cited.entry(id.to_string()).or_insert_with(|| name.clone());
            }
        }
    }
    cited
}

fn problems(root: &Path, strict: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut names: Vec<String> = std::fs::read_dir(root.join("specs"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".md") && n != "README.md" && n != "TEMPLATE.md")
        .collect();
    names.sort();
    let mut specs = Vec::new();
    for name in names {
        let file = format!("specs/{name}");
        let text = std::fs::read_to_string(root.join(&file)).unwrap_or_default();
        specs.push(parse(root, &file, &text, &mut out));
    }
    let mut ids: BTreeMap<&str, &str> = BTreeMap::new();
    for spec in specs.iter().filter(|s| !s.id.is_empty()) {
        if let Some(other) = ids.insert(&spec.id, &spec.file) {
            out.push(format!("{}: spec id `{}` is already used by {other}", spec.file, spec.id));
        }
    }
    let cited = citations(root, &mut out);
    let known: BTreeSet<&str> = specs.iter().flat_map(|s| s.requirements.iter().map(|r| r.id.as_str())).collect();
    for (id, file) in &cited {
        // a managed test (under `.claude/`) may cite a spec that stays in the harness repository
        let foreign = !strict
            && file.replace('\\', "/").starts_with(".claude/") && !ids.contains_key(id.split('-').next().unwrap_or(""));
        if !known.contains(id.as_str()) && !foreign {
            out.push(format!("{file}: cites `{id}`, which no spec defines"));
        }
    }
    for spec in specs.iter().filter(|s| s.status == "accepted") {
        for req in spec.requirements.iter().filter(|r| !r.exempt) {
            if !cited.contains_key(&req.id) {
                out.push(format!("{}: requirement {}: no test cites `// spec: {}`", spec.file, req.id, req.id));
            }
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(flag) = args.iter().find(|a| a.starts_with('-') && *a != "--strict") {
        eprintln!("unknown option `{flag}` (usage: check_specs.rs [--strict] [root])");
        std::process::exit(2);
    }
    if args.iter().filter(|a| *a != "--strict").count() > 1 {
        eprintln!("usage: check_specs.rs [--strict] [root]");
        std::process::exit(2);
    }
    let strict = args.iter().any(|a| a == "--strict");
    let root = PathBuf::from(args.iter().find(|a| *a != "--strict").map_or(".", String::as_str));
    let found = problems(&root, strict);
    for p in &found {
        println!("{p}");
    }
    std::process::exit(i32::from(!found.is_empty()));
}
