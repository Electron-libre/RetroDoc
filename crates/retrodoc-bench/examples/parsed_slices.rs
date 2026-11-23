//! Compares, with no LLM call, the slices of the entry points of saved runs built three ways
//! (`issues/done/parsed_references_for_ranking_and_slices.md`, step 1): the lexical follow of today, the parsed calls
//! with the lexical follow for what they do not resolve (what the use cases pass would use), and the parsed calls
//! alone (what the parser sees).
//!
//! Usage: `cargo run -q -p retrodoc-bench --example parsed_slices -- [--detail] <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache/entry-points.yaml`. The slice of an
//! entry-point file is cut with the depth and budget of the use cases pass (depth 2, 8 files), from that file
//! alone (the pass starts from all the files of a feature). One row per run, Markdown; with
//! `--detail`, the files each way adds or loses, per entry-point file whose slices differ.
//!
//! With `--codegraph` (`issues/done/references_resolved_by_codegraph.md`), a fourth slice follows the edges
//! codegraph resolved between files, and a second table compares the files its routes lead to with the
//! entry-point files. Both read two TSV files in `<run dir>/.codegraph/`, exported from its index
//! (`codegraph init -y --local-only`, telemetry off) by `sqlite3 .codegraph/codegraph.db`:
//!
//! ```text
//! .separator "\t"
//! .output .codegraph/edges.tsv
//! SELECT s.file_path, t.file_path, e.kind, count(*) FROM edges e JOIN nodes s ON s.id = e.source
//!   JOIN nodes t ON t.id = e.target WHERE s.file_path <> t.file_path AND e.kind <> 'contains' GROUP BY 1, 2, 3;
//! .output .codegraph/routes.tsv
//! SELECT n.id, n.name, n.file_path, coalesce(t.file_path, '') FROM nodes n
//!   LEFT JOIN edges e ON e.source = n.id AND e.kind = 'references' LEFT JOIN nodes t ON t.id = e.target
//!   WHERE n.kind = 'route';
//! ```
//!
//! A fifth slice, the mix (`issues/resolved_references_in_structure.md`), follows codegraph's edges from a file
//! they link to other source files and the lexical follow from any other file.
//!
//! The schema of that index is codegraph's own: this example reads only these exports. Their paths are relative
//! to the repository root, as the walker's (a leading `./` is dropped).

#![allow(clippy::cast_precision_loss)] // counts of files: far below 2^52

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::walker::walk_repo;
use retrodoc_ingest::{structure, FileEntry, FileKind};
use retrodoc_pipeline::{CodeIndex, EntryPoints, Follow, RoleRules};

/// As in the use cases pass.
const SLICE_DEPTH: usize = 2;
const MAX_SLICE_FILES: usize = 8;

/// What two slices of one file share and what each has alone.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Diff {
    common: Vec<PathBuf>,
    only_before: Vec<PathBuf>,
    only_after: Vec<PathBuf>,
}

fn diff(before: &[PathBuf], after: &[PathBuf]) -> Diff {
    let before: BTreeSet<&PathBuf> = before.iter().collect();
    let after: BTreeSet<&PathBuf> = after.iter().collect();
    let owned = |set: BTreeSet<&&PathBuf>| set.into_iter().map(|p| (*p).clone()).collect();
    Diff {
        common: owned(before.intersection(&after).collect()),
        only_before: owned(before.difference(&after).collect()),
        only_after: owned(after.difference(&before).collect()),
    }
}

/// Sums over the entry-point files of a run.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Totals {
    files: usize,
    parsed_files: usize,
    lexical: usize,
    mixed: usize,
    parsed: usize,
    /// Lexical vs mixed.
    mixed_diff: (usize, usize, usize),
    /// Lexical vs parsed alone.
    parsed_diff: (usize, usize, usize),
    /// Files whose mixed slice is the lexical one.
    mixed_same: usize,
    /// Sum of the codegraph slices, with `--codegraph`.
    codegraph: usize,
    /// Lexical vs codegraph.
    codegraph_diff: (usize, usize, usize),
    /// Sum of the slices of the mix (codegraph, else lexical, per file), with `--codegraph`.
    mix: usize,
    /// Lexical vs the mix.
    mix_diff: (usize, usize, usize),
}

fn sum_diff(acc: &mut (usize, usize, usize), d: &Diff) {
    acc.0 += d.common.len();
    acc.1 += d.only_before.len();
    acc.2 += d.only_after.len();
}

impl Totals {
    fn add_codegraph(&mut self, lexical: &[PathBuf], codegraph: &[PathBuf], mix: &[PathBuf]) {
        self.codegraph += codegraph.len();
        sum_diff(&mut self.codegraph_diff, &diff(lexical, codegraph));
        self.mix += mix.len();
        sum_diff(&mut self.mix_diff, &diff(lexical, mix));
    }

    /// The four cells of the codegraph slices and of the mix, appended to [`Totals::row`].
    fn codegraph_cells(&self) -> String {
        let mean = |n: usize| n as f64 / self.files.max(1) as f64;
        let (c, l, g) = self.codegraph_diff;
        let (mc, ml, mm) = self.mix_diff;
        format!(
            " {:.1} | {c} / {l} / {g} | {:.1} | {mc} / {ml} / {mm} |",
            mean(self.codegraph),
            mean(self.mix)
        )
    }

    fn add(&mut self, parsable: bool, lexical: &[PathBuf], mixed: &[PathBuf], parsed: &[PathBuf]) {
        self.files += 1;
        self.parsed_files += usize::from(parsable);
        self.lexical += lexical.len();
        self.mixed += mixed.len();
        self.parsed += parsed.len();
        let to_mixed = diff(lexical, mixed);
        self.mixed_same +=
            usize::from(to_mixed.only_before.is_empty() && to_mixed.only_after.is_empty());
        sum_diff(&mut self.mixed_diff, &to_mixed);
        sum_diff(&mut self.parsed_diff, &diff(lexical, parsed));
    }

    fn row(&self, run: &str) -> String {
        let mean = |n: usize| n as f64 / self.files.max(1) as f64;
        let (mc, ml, mm) = self.mixed_diff;
        let (pc, pl, pp) = self.parsed_diff;
        format!(
            "| {run} | {} | {} | {:.1} | {:.1} | {:.1} | {mc} / {ml} / {mm} | {} | {pc} / {pl} / {pp} |",
            self.files,
            self.parsed_files,
            mean(self.lexical),
            mean(self.mixed),
            mean(self.parsed),
            self.mixed_same,
        )
    }
}

/// The edges codegraph resolved between two files, all kinds summed (`edges.tsv`).
#[derive(Debug, Default)]
struct Graph {
    edges: BTreeMap<PathBuf, BTreeMap<PathBuf, usize>>,
}

impl Graph {
    /// Lines `source \t target \t kind \t count`; a malformed line is skipped.
    fn parse(text: &str) -> Self {
        let mut graph = Self::default();
        for line in text.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            let [source, target, _kind, count] = cols[..] else {
                continue;
            };
            let Ok(count) = count.trim().parse::<usize>() else {
                continue;
            };
            *graph
                .edges
                .entry(relative(source))
                .or_default()
                .entry(relative(target))
                .or_default() += count;
        }
        graph
    }

    fn load(repo_root: &Path) -> Option<Self> {
        std::fs::read_to_string(repo_root.join(".codegraph/edges.tsv"))
            .ok()
            .map(|text| Self::parse(&text))
    }

    /// The files `file` links to, most edges first, then by path.
    fn next(&self, file: &Path) -> Vec<PathBuf> {
        let Some(targets) = self.edges.get(file) else {
            return Vec::new();
        };
        let mut next: Vec<(&PathBuf, usize)> = targets.iter().map(|(t, n)| (t, *n)).collect();
        next.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        next.into_iter().map(|(t, _)| t.clone()).collect()
    }

    /// The slice along codegraph's edges alone.
    fn slice(
        &self,
        starts: &[PathBuf],
        sources: &BTreeSet<PathBuf>,
        depth: usize,
        max_files: usize,
    ) -> Vec<PathBuf> {
        slice_by(starts, sources, depth, max_files, |file| self.next(file))
    }

    /// The mix: codegraph's edges from a file they link to other source files, `lexical` from any other.
    fn mixed_next(
        &self,
        file: &Path,
        sources: &BTreeSet<PathBuf>,
        lexical: impl Fn(&Path) -> Vec<PathBuf>,
    ) -> Vec<PathBuf> {
        let linked: Vec<PathBuf> = self
            .next(file)
            .into_iter()
            .filter(|t| sources.contains(t))
            .collect();
        if linked.is_empty() {
            lexical(file)
        } else {
            linked
        }
    }
}

/// As `CodeIndex::slice_with`, going from a file to the files `next` gives (best first): hop by hop, a file ranked
/// by its best rank across the frontier, among the source files only, at most `max_files`.
fn slice_by(
    starts: &[PathBuf],
    sources: &BTreeSet<PathBuf>,
    depth: usize,
    max_files: usize,
    next: impl Fn(&Path) -> Vec<PathBuf>,
) -> Vec<PathBuf> {
    let mut seen: BTreeSet<PathBuf> = starts.iter().cloned().collect();
    let mut frontier: Vec<PathBuf> = starts.to_vec();
    let mut slice: Vec<PathBuf> = Vec::new();
    for _ in 0..depth {
        let mut hop: BTreeMap<PathBuf, usize> = BTreeMap::new();
        for file in &frontier {
            let found = next(file)
                .into_iter()
                .filter(|t| sources.contains(t) && !seen.contains(t));
            for (rank, target) in found.enumerate() {
                let entry = hop.entry(target).or_insert(usize::MAX);
                *entry = (*entry).min(rank);
            }
        }
        let mut ranked: Vec<(PathBuf, usize)> = hop.into_iter().collect();
        ranked.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        frontier = Vec::new();
        for (file, _) in ranked {
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

/// A path of an export as the walker writes it.
fn relative(path: &str) -> PathBuf {
    PathBuf::from(path.strip_prefix("./").unwrap_or(path))
}

/// The number of routes of `routes.tsv` (lines `id \t name \t route file \t target file`, one per reference of
/// a route) and the files they lead to: a target among the source files, else the file declaring the route.
fn route_files(text: &str, sources: &BTreeSet<PathBuf>) -> (usize, BTreeSet<PathBuf>) {
    // Per route id: the file declaring it, and its targets among the sources.
    let mut routes: BTreeMap<&str, (PathBuf, BTreeSet<PathBuf>)> = BTreeMap::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        let [id, _name, file, target] = cols[..] else {
            continue;
        };
        let route = routes
            .entry(id)
            .or_insert_with(|| (relative(file), BTreeSet::new()));
        let target = relative(target);
        if sources.contains(&target) {
            route.1.insert(target);
        }
    }
    let count = routes.len();
    let files = routes
        .into_values()
        .flat_map(|(file, targets)| {
            if targets.is_empty() {
                BTreeSet::from([file])
            } else {
                targets
            }
        })
        .collect();
    (count, files)
}

fn names(files: &BTreeSet<&PathBuf>) -> String {
    list(&files.iter().map(|f| (*f).clone()).collect::<Vec<_>>())
}

/// Our entry-point files against the files codegraph's routes lead to.
fn route_row(run: &str, ours: &BTreeSet<PathBuf>, routes: &(usize, BTreeSet<PathBuf>)) -> String {
    let (count, theirs) = routes;
    let both = ours.intersection(theirs).count();
    let ours_only: BTreeSet<&PathBuf> = ours.difference(theirs).collect();
    let theirs_only: BTreeSet<&PathBuf> = theirs.difference(ours).collect();
    format!(
        "| {run} | {} | {count} | {} | {both} / {} / {} | {} | {} |",
        ours.len(),
        theirs.len(),
        ours_only.len(),
        theirs_only.len(),
        names(&ours_only),
        names(&theirs_only),
    )
}

fn sources_of(repo_root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<FileEntry> = walk_repo(repo_root, &[]).expect("tree is walkable");
    if let Some(rules) = RoleRules::load(repo_root) {
        rules.promote_sources(&mut files);
    }
    files
        .into_iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path)
        .collect()
}

fn list(files: &[PathBuf]) -> String {
    let names: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join(", ")
    }
}

/// What one run adds to the output: its row of the slices table, its row of the routes table, its details.
struct RunReport {
    line: String,
    route: Option<String>,
    details: String,
}

/// The slices of `starts` along codegraph's edges alone, and along the mix.
fn codegraph_slices(
    graph: &Graph,
    index: &CodeIndex,
    repo_root: &Path,
    starts: &[PathBuf],
    sources: &BTreeSet<PathBuf>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let lexical = |from: &Path| {
        index.slice_with(
            repo_root,
            &[from.to_path_buf()],
            1,
            usize::MAX,
            Follow::Lexical,
        )
    };
    let mix = slice_by(starts, sources, SLICE_DEPTH, MAX_SLICE_FILES, |from| {
        graph.mixed_next(from, sources, lexical)
    });
    (
        graph.slice(starts, sources, SLICE_DEPTH, MAX_SLICE_FILES),
        mix,
    )
}

fn row(repo_root: &Path, detail: bool, codegraph: bool) -> Option<RunReport> {
    let entry_points = EntryPoints::load(repo_root)?;
    let sources = sources_of(repo_root);
    let graph = if codegraph {
        Graph::load(repo_root).or_else(|| {
            eprintln!("{}: no .codegraph/edges.tsv", repo_root.display());
            None
        })
    } else {
        None
    };
    let source_set: BTreeSet<PathBuf> = sources.iter().cloned().collect();
    let index = CodeIndex::new(sources.iter().map(PathBuf::as_path)).with_definitions(repo_root);
    let mut totals = Totals::default();
    let mut details = String::new();
    for (file, entries) in &entry_points.files {
        if entries.entry_points.is_empty() {
            continue;
        }
        let starts = std::slice::from_ref(file);
        let slice =
            |follow| index.slice_with(repo_root, starts, SLICE_DEPTH, MAX_SLICE_FILES, follow);
        let (lexical, mixed, parsed) = (
            slice(Follow::Lexical),
            slice(Follow::ParsedThenLexical),
            slice(Follow::Parsed),
        );
        totals.add(structure::is_supported(file), &lexical, &mixed, &parsed);
        let codegraph_slices = graph
            .as_ref()
            .map(|g| codegraph_slices(g, &index, repo_root, starts, &source_set));
        if let Some((slice, mix)) = &codegraph_slices {
            totals.add_codegraph(&lexical, slice, mix);
        }
        let d = diff(&lexical, &mixed);
        let differs = |d: &Diff| !(d.only_before.is_empty() && d.only_after.is_empty());
        let codegraph_differs = codegraph_slices.as_ref().is_some_and(|(slice, mix)| {
            differs(&diff(&lexical, slice)) || differs(&diff(&lexical, mix))
        });
        if detail && (differs(&d) || codegraph_differs) {
            let _ = write!(
                details,
                "\n### {}\n- common: {}\n- lexical only: {}\n- mixed only: {}\n- parsed alone: {}\n",
                file.display(),
                list(&d.common),
                list(&d.only_before),
                list(&d.only_after),
                list(&parsed),
            );
            if let Some((slice, mix)) = &codegraph_slices {
                for (name, other) in [("codegraph", slice), ("mix", mix)] {
                    let g = diff(&lexical, other);
                    let _ = write!(
                        details,
                        "- {name}: {}\n  - not in lexical: {}\n  - lexical not in it: {}\n",
                        list(other),
                        list(&g.only_after),
                        list(&g.only_before),
                    );
                }
            }
        }
    }
    let run = repo_root.file_name()?.to_string_lossy().into_owned();
    let mut line = totals.row(&run);
    let mut route = None;
    if codegraph {
        line += &if graph.is_some() {
            totals.codegraph_cells()
        } else {
            " - | - | - | - |".to_string()
        };
        let ours: BTreeSet<PathBuf> = entry_points
            .files
            .iter()
            .filter(|(_, e)| !e.entry_points.is_empty())
            .map(|(f, _)| f.clone())
            .collect();
        match std::fs::read_to_string(repo_root.join(".codegraph/routes.tsv")) {
            Ok(text) => route = Some(route_row(&run, &ours, &route_files(&text, &source_set))),
            Err(_) => eprintln!("{}: no .codegraph/routes.tsv", repo_root.display()),
        }
    }
    Some(RunReport {
        line,
        route,
        details,
    })
}

fn main() {
    let mut detail = false;
    let mut codegraph = false;
    let mut dirs: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--detail" {
            detail = true;
        } else if arg == "--codegraph" {
            codegraph = true;
        } else {
            dirs.push(PathBuf::from(arg));
        }
    }
    if dirs.is_empty() {
        eprintln!("usage: parsed_slices [--detail] [--codegraph] <run dir>...");
        std::process::exit(2);
    }
    println!("Mean slice size per entry-point file; common / lexical only / other only, summed over the files.\n");
    let (extra, extra_rule) = if codegraph {
        (
            " Codegraph | Lexical vs codegraph | Mix | Lexical vs mix |",
            "---|---|---|---|",
        )
    } else {
        ("", "")
    };
    println!("| Run | Entry-point files | Parsable | Lexical | Mixed | Parsed | Lexical vs mixed | Mixed = lexical | Lexical vs parsed |{extra}");
    println!("|---|---|---|---|---|---|---|---|---|{extra_rule}");
    let mut routes = Vec::new();
    let mut details = Vec::new();
    for dir in dirs {
        match row(&dir, detail, codegraph) {
            Some(report) => {
                println!("{}", report.line);
                routes.extend(report.route);
                details.push(report.details);
            }
            None => eprintln!("{}: no entry-points.yaml, skipped", dir.display()),
        }
    }
    if !routes.is_empty() {
        println!("\nEntry-point files against the files codegraph's routes lead to (their target, else the file declaring them).\n");
        println!("| Run | Entry-point files | Codegraph routes | Their files | Both / ours only / codegraph only | Ours only | Codegraph only |");
        println!("|---|---|---|---|---|---|---|");
        for line in routes {
            println!("{line}");
        }
    }
    for text in details {
        print!("{text}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_diff_splits_the_files_in_common_and_the_files_of_one_side() {
        let d = diff(&paths(&["a", "b", "c"]), &paths(&["c", "d", "a"]));
        assert_eq!(d.common, paths(&["a", "c"]));
        assert_eq!(d.only_before, paths(&["b"]));
        assert_eq!(d.only_after, paths(&["d"]));
    }

    #[test]
    fn totals_sum_the_files_and_count_the_unchanged_slices() {
        let mut totals = Totals::default();
        totals.add(
            true,
            &paths(&["a", "b"]),
            &paths(&["a", "b"]),
            &paths(&["c"]),
        );
        totals.add(false, &paths(&["a"]), &paths(&["a", "d"]), &[]);
        assert_eq!(totals.files, 2);
        assert_eq!(totals.parsed_files, 1);
        assert_eq!(totals.mixed_same, 1);
        assert_eq!(totals.mixed_diff, (3, 0, 1));
        assert_eq!(totals.parsed_diff, (0, 3, 1));
        assert_eq!(
            totals.row("r"),
            "| r | 2 | 1 | 1.5 | 2.0 | 0.5 | 3 / 0 / 1 | 1 | 0 / 3 / 1 |"
        );
    }

    #[test]
    fn the_edges_of_codegraph_sum_their_kinds_per_pair_of_files() {
        let graph = Graph::parse(
            "a.py\tb.py\tcalls\t3\na.py\t./b.py\timports\t1\na.py\tc.py\tcalls\t2\nbroken line\n",
        );
        assert_eq!(graph.next(Path::new("a.py")), paths(&["b.py", "c.py"]));
        assert_eq!(graph.next(Path::new("b.py")), Vec::<PathBuf>::new());
    }

    #[test]
    fn the_codegraph_slice_goes_by_hops_most_linked_first_within_the_sources() {
        let graph = Graph::parse(
            "a\tb\tcalls\t1\na\tc\tcalls\t5\nc\td\tcalls\t1\nb\ttest\tcalls\t9\nd\ta\tcalls\t1\nd\te\tcalls\t1\n",
        );
        let sources: BTreeSet<PathBuf> = paths(&["a", "b", "c", "d", "e"]).into_iter().collect();
        let starts = paths(&["a"]);
        assert_eq!(
            graph.slice(&starts, &sources, 2, 8),
            paths(&["c", "b", "d"])
        );
        assert_eq!(
            graph.slice(&starts, &sources, 3, 8),
            paths(&["c", "b", "d", "e"])
        );
        assert_eq!(graph.slice(&starts, &sources, 2, 1), paths(&["c"]));
    }

    #[test]
    fn the_mix_follows_codegraph_from_a_linked_file_and_the_lexical_follow_from_the_others() {
        // `a` links to `b` (and to a test, out of the sources); `b` links nowhere: the lexical follow goes on.
        let graph = Graph::parse("a\tb\tcalls\t1\na\ttest\tcalls\t4\n");
        let sources: BTreeSet<PathBuf> = paths(&["a", "b", "c", "d", "e"]).into_iter().collect();
        let lexical = |file: &Path| match file.to_str() {
            Some("a") => paths(&["e", "c"]),
            Some("b") => paths(&["a", "d", "test"]),
            _ => Vec::new(),
        };
        let next = |file: &Path| graph.mixed_next(file, &sources, lexical);
        assert_eq!(
            slice_by(&paths(&["a"]), &sources, 2, 8, next),
            paths(&["b", "d"])
        );
        // Started from a file codegraph does not link, the lexical follow.
        assert_eq!(
            slice_by(&paths(&["b"]), &sources, 1, 8, next),
            paths(&["a", "d"])
        );
    }

    #[test]
    fn the_routes_of_codegraph_are_compared_to_the_entry_point_files_by_target() {
        let sources: BTreeSet<PathBuf> = paths(&[
            "urls.py",
            "views/users.py",
            "views/admin.py",
            "api/routes.py",
        ])
        .into_iter()
        .collect();
        // A route with a target among the sources leads there only, whatever its other references.
        let reached = route_files(
            "1\tusers/\turls.py\tviews/users.py\n1\tusers/\turls.py\tserializers.py\n",
            &sources,
        );
        assert_eq!(
            reached,
            (1, paths(&["views/users.py"]).into_iter().collect())
        );
        // One route per id; a route with no target among the sources leads to the file declaring it.
        let routes = route_files(
            "1\tusers/\turls.py\tviews/users.py\n1\tusers/\turls.py\tserializers.py\n2\thealth\turls.py\t\n\
             3\tadmin/\t./urls.py\tviews/admin.py\n4\tadmin/\turls.py\ttemplates/admin.html\n",
            &sources,
        );
        assert_eq!(routes.0, 4);
        assert_eq!(
            routes.1,
            paths(&["urls.py", "views/admin.py", "views/users.py"])
                .into_iter()
                .collect()
        );
        let ours: BTreeSet<PathBuf> = paths(&["views/users.py", "api/routes.py"])
            .into_iter()
            .collect();
        assert_eq!(
            route_row("r", &ours, &routes),
            "| r | 2 | 4 | 3 | 1 / 1 / 2 | api/routes.py | urls.py, views/admin.py |"
        );
    }
}
