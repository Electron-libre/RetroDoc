//! Code slices (PLAN.md §7.1, phase 7 step 4b): the part of the code an entry
//! point traverses (controller → service → model), found by the identifiers
//! it references, so that a use case is derived from the code it actually
//! runs rather than from every file of its feature.
//!
//! This is a heuristic, not call tracing (hard in dynamic languages): an
//! identifier designates a file when it matches the file's name, ignoring
//! case, underscores and a plural `s` (`ContractSigner`, `contract_signer`
//! and `contract_signers` all designate `contract_signer.rb`). A name shared
//! by a few files designates the ones closest in the tree to the referencing
//! file (`app/models/contract.rb` rather than `lib/pdf/contract.rb` from
//! `app/controllers/contracts_controller.rb`); one shared by many designates
//! none (too ambiguous: `base`, `index`).
//!
//! [`CodeIndex::slice_with`] can follow the parse instead ([`Follow`]): the names a file calls (ADR 0027;
//! Ruby's query also gives the constants it reads) go to the files of the same language that define them,
//! under the same limits on shared names. Only the `parsed_slices` example uses it so far: the passes
//! cut their slices with the lexical follow.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use retrodoc_ingest::structure;

use crate::naming::normalize;
use crate::text::read_file_lossy;

/// A name shared by more files than this designates none of them.
const MAX_FILES_PER_NAME: usize = 6;
/// Of the files sharing a name, how many it designates (the closest ones).
const DESIGNATED_PER_NAME: usize = 2;

/// How a slice goes from a file to the files it uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Follow {
    /// The identifiers of the text that match a file's name (the heuristic above).
    Lexical,
    /// The names the parse sees called (ADR 0027), resolved to the files that define them; a file this
    /// build cannot parse leads nowhere.
    Parsed,
    /// The parsed calls, plus the lexical follow for the identifiers no call resolves (constants,
    /// files the build cannot parse).
    ParsedThenLexical,
}

/// Source files of the repo by normalized file name, and by the names they define once
/// [`CodeIndex::with_definitions`] has parsed them.
#[derive(Debug, Clone, Default)]
pub struct CodeIndex {
    by_name: BTreeMap<String, Vec<PathBuf>>,
    /// Exact name of a definition -> the files that define it.
    definitions: BTreeMap<String, Vec<PathBuf>>,
}

/// Number of leading directories two paths have in common.
fn shared_prefix(a: &Path, b: &Path) -> usize {
    let dirs = |p: &Path| -> Vec<String> {
        p.parent()
            .map(|d| {
                d.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    dirs(a)
        .iter()
        .zip(dirs(b).iter())
        .take_while(|(x, y)| x == y)
        .count()
}

/// Identifiers of `content` with their number of occurrences.
fn identifiers(content: &str) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for word in content.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if word.len() >= 4 {
            *counts.entry(word).or_insert(0) += 1;
        }
    }
    counts
}

/// Of the files a name designates, those it counts for: none when they are too many, else the closest to
/// `from` (then the shallowest, then in path order), `exclude`d files left out.
fn designated<'a>(
    files: impl IntoIterator<Item = &'a PathBuf>,
    from: &Path,
    exclude: &BTreeSet<PathBuf>,
) -> Vec<&'a PathBuf> {
    let files: Vec<&PathBuf> = files.into_iter().collect();
    if files.len() > MAX_FILES_PER_NAME {
        return Vec::new();
    }
    let mut candidates: Vec<&PathBuf> = files
        .into_iter()
        .filter(|f| !exclude.contains(*f))
        .collect();
    candidates.sort_by(|a, b| {
        shared_prefix(from, b)
            .cmp(&shared_prefix(from, a))
            .then_with(|| a.components().count().cmp(&b.components().count()))
            .then_with(|| a.cmp(b))
    });
    candidates.truncate(DESIGNATED_PER_NAME);
    candidates
}

impl CodeIndex {
    pub fn new<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Self {
        let mut by_name: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for path in paths {
            if let Some(stem) = path.file_stem() {
                by_name
                    .entry(normalize(&stem.to_string_lossy()))
                    .or_default()
                    .push(path.to_path_buf());
            }
        }
        Self {
            by_name,
            definitions: BTreeMap::new(),
        }
    }

    /// Parses the indexed files this build can parse and records the names they define, for
    /// [`Follow::Parsed`] and [`Follow::ParsedThenLexical`]. An unreadable file defines nothing.
    #[must_use]
    pub fn with_definitions(mut self, repo_root: &Path) -> Self {
        let mut definitions: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();
        for path in self.by_name.values().flatten() {
            if !structure::is_supported(path) {
                continue;
            }
            let Ok(content) = read_file_lossy(repo_root, path) else {
                continue;
            };
            for definition in structure::parse(path, &content)
                .map(|s| s.definitions)
                .unwrap_or_default()
            {
                definitions
                    .entry(definition.name)
                    .or_default()
                    .insert(path.clone());
            }
        }
        self.definitions = definitions
            .into_iter()
            .map(|(name, files)| (name, files.into_iter().collect()))
            .collect();
        self
    }

    /// Files designated by `content` (the content of `from`) as `follow` says, most referenced first
    /// (then in path order), `exclude`d files left out.
    fn referenced(
        &self,
        content: &str,
        from: &Path,
        exclude: &BTreeSet<PathBuf>,
        follow: Follow,
    ) -> Vec<PathBuf> {
        let mut scores: BTreeMap<&PathBuf, usize> = BTreeMap::new();
        // Exact names of the calls that resolved to a file: the lexical follow leaves them alone.
        let mut resolved: BTreeSet<String> = BTreeSet::new();
        let parsed = match follow {
            Follow::Lexical => None,
            Follow::Parsed | Follow::ParsedThenLexical => structure::parse(from, content),
        };
        if let Some(parsed) = parsed {
            let mut calls: BTreeMap<String, usize> = BTreeMap::new();
            for reference in parsed.references {
                *calls.entry(reference.name).or_insert(0) += 1;
            }
            for (name, count) in calls {
                let Some(files) = self.definitions.get(&name) else {
                    continue;
                };
                let same_language: Vec<&PathBuf> = files
                    .iter()
                    .filter(|f| structure::same_language(from, f))
                    .collect();
                // Defined in the repo in this language and not too shared: the parse has the answer, even
                // when every definition is already seen or is `from` itself.
                if same_language.is_empty() || same_language.len() > MAX_FILES_PER_NAME {
                    continue;
                }
                resolved.insert(name);
                for file in designated(same_language, from, exclude) {
                    *scores.entry(file).or_insert(0) += count;
                }
            }
        }
        if follow != Follow::Parsed {
            for (word, count) in identifiers(content) {
                if resolved.contains(word) {
                    continue;
                }
                let Some(files) = self.by_name.get(&normalize(word)) else {
                    continue;
                };
                for file in designated(files, from, exclude) {
                    *scores.entry(file).or_insert(0) += count;
                }
            }
        }
        let mut ranked: Vec<(&PathBuf, usize)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        ranked.into_iter().map(|(file, _)| file.clone()).collect()
    }

    /// The files reachable from `starts` by following referenced
    /// identifiers up to `depth` hops, at most `max_files` of them (the
    /// starts are not included). Nearer files come first, and within a hop
    /// the most referenced ones.
    #[must_use]
    pub fn slice(
        &self,
        repo_root: &Path,
        starts: &[PathBuf],
        depth: usize,
        max_files: usize,
    ) -> Vec<PathBuf> {
        self.slice_with(repo_root, starts, depth, max_files, Follow::Lexical)
    }

    /// [`CodeIndex::slice`], going from a file to the next as `follow` says.
    #[must_use]
    pub fn slice_with(
        &self,
        repo_root: &Path,
        starts: &[PathBuf],
        depth: usize,
        max_files: usize,
        follow: Follow,
    ) -> Vec<PathBuf> {
        let mut seen: BTreeSet<PathBuf> = starts.iter().cloned().collect();
        let mut frontier: Vec<PathBuf> = starts.to_vec();
        let mut slice: Vec<PathBuf> = Vec::new();

        for _ in 0..depth {
            let mut hop: BTreeMap<PathBuf, usize> = BTreeMap::new();
            for file in &frontier {
                let Ok(content) = read_file_lossy(repo_root, file) else {
                    continue;
                };
                for (rank, found) in self
                    .referenced(&content, file, &seen, follow)
                    .into_iter()
                    .enumerate()
                {
                    // Best (lowest) rank across the files of the frontier.
                    let entry = hop.entry(found).or_insert(usize::MAX);
                    *entry = (*entry).min(rank);
                }
            }
            let mut next: Vec<(PathBuf, usize)> = hop.into_iter().collect();
            next.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
            frontier = Vec::new();
            for (file, _) in next {
                if slice.len() >= max_files {
                    return slice;
                }
                seen.insert(file.clone());
                slice.push(file.clone());
                frontier.push(file);
            }
        }
        slice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, content: &str) {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }

    fn index(paths: &[&str]) -> CodeIndex {
        CodeIndex::new(paths.iter().map(Path::new))
    }

    #[test]
    fn names_match_across_naming_conventions_and_plurals() {
        assert_eq!(
            normalize("ContractSigner"),
            normalize("contract_signer.rb".trim_end_matches(".rb"))
        );
        assert_eq!(normalize("contract_signers"), normalize("ContractSigner"));
        assert_eq!(normalize("bus"), "bus");
    }

    #[test]
    fn follows_references_hop_by_hop_and_ignores_ambiguous_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "app/controllers/contracts_controller.rb",
            "def sign\n  ContractSigner.new(contract).call\n  Base.log\nend",
        );
        write(
            root,
            "app/services/contract_signer.rb",
            "class ContractSigner\n  def call\n    Signature.create!\n  end\nend",
        );
        write(root, "app/models/signature.rb", "class Signature; end");
        write(root, "app/models/contract.rb", "class Contract; end");
        for dir in ["a", "b", "c", "d", "e", "f", "g"] {
            write(root, &format!("app/{dir}/base.rb"), "x");
        }
        let paths = [
            "app/controllers/contracts_controller.rb",
            "app/services/contract_signer.rb",
            "app/models/signature.rb",
            "app/models/contract.rb",
        ];
        let mut paths = paths.to_vec();
        let bases: Vec<String> = ["a", "b", "c", "d", "e", "f", "g"]
            .iter()
            .map(|d| format!("app/{d}/base.rb"))
            .collect();
        paths.extend(bases.iter().map(String::as_str));
        let index = index(&paths);
        let starts = [PathBuf::from("app/controllers/contracts_controller.rb")];

        let one_hop = index.slice(root, &starts, 1, 10);
        assert_eq!(
            one_hop,
            vec![
                PathBuf::from("app/models/contract.rb"),
                PathBuf::from("app/services/contract_signer.rb"),
            ]
        );

        let two_hops = index.slice(root, &starts, 2, 10);
        assert!(two_hops.contains(&PathBuf::from("app/models/signature.rb")));
        assert!(!two_hops.iter().any(|p| p.ends_with("base.rb")));

        assert_eq!(index.slice(root, &starts, 2, 1).len(), 1);
    }

    /// Whether this build parses Ruby (the grammars are features of `retrodoc-ingest`).
    fn parses_ruby() -> bool {
        structure::is_supported(Path::new("a.rb"))
    }

    #[test]
    fn the_parsed_follow_reaches_a_called_method_whatever_its_file_name_and_skips_comments() {
        if !parses_ruby() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let paths = [
            "app/controllers/orders_controller.rb",
            "app/services/checkout.rb",
            "app/models/invoice.rb",
            "app/models/payment.rb",
        ];
        write(
            root,
            paths[0],
            "class OrdersController\n  # Invoice is done elsewhere\n  def create\n    place_order(Payment)\n  end\nend\n",
        );
        write(
            root,
            paths[1],
            "module Checkout\n  def place_order(x)\n  end\nend\n",
        );
        write(root, paths[2], "class Invoice\nend\n");
        write(root, paths[3], "class Payment\nend\n");
        let index = index(&paths).with_definitions(root);
        let starts = [PathBuf::from(paths[0])];
        let slice = |follow| index.slice_with(root, &starts, 1, 10, follow);

        assert_eq!(
            slice(Follow::Lexical),
            [PathBuf::from(paths[2]), PathBuf::from(paths[3])]
        );
        // Ruby's query also gives the constants it reads; the name in the comment is no reference.
        assert_eq!(
            slice(Follow::Parsed),
            [PathBuf::from(paths[3]), PathBuf::from(paths[1])]
        );
        // The lexical follow adds what the parse does not resolve, the comment included.
        assert_eq!(
            slice(Follow::ParsedThenLexical),
            [
                PathBuf::from(paths[2]),
                PathBuf::from(paths[3]),
                PathBuf::from(paths[1]),
            ]
        );
    }

    #[test]
    fn a_resolved_call_never_falls_back_on_the_file_name() {
        if !parses_ruby() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let paths = [
            "app/jobs/sync_job.rb",
            "app/services/pipeline.rb",
            "app/models/refresh.rb",
        ];
        // `refresh` is defined here and in the service, not in `refresh.rb`, which defines `Refresh`.
        write(
            root,
            paths[0],
            "class SyncJob\n  def refresh\n  end\n  def go\n    refresh\n  end\nend\n",
        );
        write(
            root,
            paths[1],
            "class Pipeline\n  def refresh\n  end\nend\n",
        );
        write(root, paths[2], "class Refresh\nend\n");
        let index = index(&paths).with_definitions(root);
        let starts = [PathBuf::from(paths[0])];
        assert_eq!(
            index.slice_with(root, &starts, 1, 10, Follow::ParsedThenLexical),
            [PathBuf::from(paths[1])]
        );
        // Its only other definition already in the slice: still resolved, no lexical guess.
        let seen: BTreeSet<PathBuf> = [paths[0], paths[1]].map(PathBuf::from).into();
        let content = std::fs::read_to_string(root.join(paths[0])).unwrap();
        assert_eq!(
            index.referenced(
                &content,
                Path::new(paths[0]),
                &seen,
                Follow::ParsedThenLexical
            ),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn a_called_name_defined_in_many_files_resolves_to_none() {
        if !parses_ruby() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut paths = vec!["app/jobs/run.rb".to_string()];
        write(
            root,
            &paths[0],
            "def go
  call
end
",
        );
        for n in 0..=MAX_FILES_PER_NAME {
            let path = format!("app/services/s{n}.rb");
            write(
                root,
                &path,
                "def call
end
",
            );
            paths.push(path);
        }
        let index =
            CodeIndex::new(paths.iter().map(|p| Path::new(p.as_str()))).with_definitions(root);
        let starts = [PathBuf::from(&paths[0])];
        assert_eq!(
            index.slice_with(root, &starts, 1, 10, Follow::Parsed),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn a_call_resolves_to_definitions_of_its_own_language_only() {
        if !parses_ruby() || !structure::is_supported(Path::new("a.js")) {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "app/jobs/run.rb", "def go\n  autosize\nend\n");
        write(root, "vendor/sizer.js", "function autosize() {}\n");
        let index = index(&["app/jobs/run.rb", "vendor/sizer.js"]).with_definitions(root);
        let starts = [PathBuf::from("app/jobs/run.rb")];
        assert_eq!(
            index.slice_with(root, &starts, 1, 10, Follow::Parsed),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn a_file_the_build_cannot_parse_keeps_the_lexical_follow() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "cmd/main.go", "func main() { RunOrder() }");
        write(root, "cmd/run_order.go", "func RunOrder() {}");
        let index = index(&["cmd/main.go", "cmd/run_order.go"]).with_definitions(root);
        let starts = [PathBuf::from("cmd/main.go")];
        assert_eq!(
            index.slice_with(root, &starts, 1, 10, Follow::ParsedThenLexical),
            [PathBuf::from("cmd/run_order.go")]
        );
        assert_eq!(
            index.slice_with(root, &starts, 1, 10, Follow::Parsed),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn a_shared_name_designates_the_closest_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "app/controllers/contracts_controller.rb",
            "Contract.find",
        );
        for path in [
            "app/models/contract.rb",
            "lib/pdf/contract.rb",
            "lib/xml/contract.rb",
        ] {
            write(root, path, "x");
        }
        let index = index(&[
            "app/controllers/contracts_controller.rb",
            "app/models/contract.rb",
            "lib/pdf/contract.rb",
            "lib/xml/contract.rb",
        ]);
        let starts = [PathBuf::from("app/controllers/contracts_controller.rb")];
        let slice = index.slice(root, &starts, 1, 1);
        assert_eq!(slice, vec![PathBuf::from("app/models/contract.rb")]);
    }
}
