//! The routes a routing file declares for other files (`specs/routing-file-duplicates.md`). A
//! routing file (`config/routes.rb`) declares the routes its controllers also give, so the later
//! passes would turn each route into two features or use cases. The copy of the routing file is
//! left out when a controller covers each of its paths. Only the names the LLM gave are read, with
//! no framework convention: the paths are the words of a name that start with `/`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{EntryKind, EntryPoint, EntryPoints};

/// Other files that must give a route equal to one of a file's for it to be a routing file.
const MIN_ROUTED_FILES: usize = 3;

/// A segment of a route path: a name, or a placeholder (`:id`, `{id}`...), all alike.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Name(String),
    Placeholder,
}

/// A route path as its segments; the root `/` has none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RoutePath(Vec<Segment>);

impl RoutePath {
    fn parse(word: &str) -> Self {
        let parts: Vec<&str> = word.split('/').filter(|s| !s.is_empty()).collect();
        let last = parts.len().saturating_sub(1);
        RoutePath(
            parts
                .iter()
                .enumerate()
                .map(|(i, part)| {
                    if is_placeholder(part) {
                        Segment::Placeholder
                    } else if i == last {
                        Segment::Name(part.split('.').next().unwrap_or_default().to_string())
                    } else {
                        Segment::Name((*part).to_string())
                    }
                })
                .collect(),
        )
    }

    /// Whether its segments are the first ones of `other`'s, or all of them; the root is
    /// covered by the root only.
    fn covered_by(&self, other: &RoutePath) -> bool {
        if self.0.is_empty() {
            return other.0.is_empty();
        }
        other.0.starts_with(&self.0)
    }

    /// The path with its last segment replaced by `segments` (`/stories/:id/upvote` and
    /// `/unvote` give `/stories/:id/unvote`).
    fn sibling(&self, segments: &RoutePath) -> RoutePath {
        let mut parent = self.0[..self.0.len() - 1].to_vec();
        parent.extend(segments.0.iter().cloned());
        RoutePath(parent)
    }
}

fn is_placeholder(segment: &str) -> bool {
    let wrapped = |open, close| segment.starts_with(open) && segment.ends_with(close);
    segment.starts_with(':')
        || segment.starts_with('*')
        || wrapped('{', '}')
        || wrapped('<', '>')
        || wrapped('[', ']')
}

/// The paths an entry names: its words starting with `/`, parenthesised parts dropped.
fn paths_of(name: &str) -> Vec<RoutePath> {
    let mut plain = String::new();
    let mut depth = 0usize;
    for c in name.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    plain
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| w.starts_with('/'))
        .map(RoutePath::parse)
        .collect()
}

/// The ways each path of a name can be read: itself, and for a one-segment path after a longer
/// one, its sibling.
fn readings(paths: &[RoutePath]) -> Vec<Vec<RoutePath>> {
    paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let mut ways = vec![path.clone()];
            if i > 0 && path.0.len() == 1 && paths[i - 1].0.len() > 1 {
                ways.push(paths[i - 1].sibling(path));
            }
            ways
        })
        .collect()
}

/// An `http_route` entry with the paths it names.
type Route<'a> = (&'a EntryPoint, Vec<RoutePath>);

/// The paths of the `http_route` entries of each file.
struct Routes<'a> {
    files: Vec<(&'a Path, Vec<Route<'a>>)>,
}

impl<'a> Routes<'a> {
    fn of(entries: &'a EntryPoints) -> Self {
        let files = entries
            .files
            .iter()
            .map(|(path, file)| {
                let routes = file
                    .entry_points
                    .iter()
                    .filter(|e| e.kind == EntryKind::HttpRoute)
                    .map(|e| (e, paths_of(&e.name)))
                    .collect();
                (path.as_path(), routes)
            })
            .collect();
        Routes { files }
    }

    /// The files other than `file` with a path `matches` accepts.
    fn files_with(&self, file: &Path, matches: impl Fn(&RoutePath) -> bool) -> BTreeSet<&'a Path> {
        self.files
            .iter()
            .filter(|(other, routes)| {
                *other != file && routes.iter().any(|(_, paths)| paths.iter().any(&matches))
            })
            .map(|(other, _)| *other)
            .collect()
    }

    /// At least [`MIN_ROUTED_FILES`] other files give a path equal to one of its paths, and at
    /// least half of its routes have a path another file covers.
    fn is_routing_file(&self, file: &Path, routes: &[Route]) -> bool {
        let paths: Vec<&RoutePath> = routes.iter().flat_map(|(_, p)| p).collect();
        let equal = self.files_with(file, |q| paths.contains(&q));
        let covered = routes
            .iter()
            .filter(|(_, paths)| {
                paths
                    .iter()
                    .any(|p| !self.files_with(file, |q| p.covered_by(q)).is_empty())
            })
            .count();
        equal.len() >= MIN_ROUTED_FILES && 2 * covered >= routes.len()
    }
}

impl EntryPoints {
    /// The inventory without the routes a routing file declares for other files: an
    /// `http_route` entry of a routing file whose every path a file that is not one covers (see
    /// `specs/routing-file-duplicates.md`). Returns it with the ids (`<file>#<name>`) of the
    /// entries left out; the saved inventory is not touched, so the choice is made again at each
    /// run. Limits: only the names the LLM gave are read, and a route declared under another path
    /// than its action's (an alias) stays.
    #[must_use]
    pub fn without_routes_declared_elsewhere(mut self) -> (Self, Vec<String>) {
        let left_out: BTreeSet<(PathBuf, String)> = {
            let routes = Routes::of(&self);
            let routing: BTreeSet<&Path> = routes
                .files
                .iter()
                .filter(|(file, entries)| routes.is_routing_file(file, entries))
                .map(|(file, _)| *file)
                .collect();
            let covered = |file: &Path, path: &RoutePath| {
                routes
                    .files_with(file, |q| path.covered_by(q))
                    .iter()
                    .any(|other| !routing.contains(other))
            };
            routes
                .files
                .iter()
                .filter(|(file, _)| routing.contains(file))
                .flat_map(|(file, entries)| {
                    entries
                        .iter()
                        .filter(|(_, paths)| {
                            !paths.is_empty()
                                && readings(paths)
                                    .iter()
                                    .all(|ways| ways.iter().any(|p| covered(file, p)))
                        })
                        .map(|(entry, _)| (file.to_path_buf(), entry.name.clone()))
                })
                .collect()
        };
        let mut ids = Vec::new();
        for (path, file) in &mut self.files {
            file.entry_points.retain(|e| {
                let keep = e.kind != EntryKind::HttpRoute
                    || !left_out.contains(&(path.clone(), e.name.clone()));
                if !keep {
                    ids.push(format!("{}#{}", path.display(), e.name));
                }
                keep
            });
        }
        (self, ids)
    }
}

#[cfg(test)]
mod tests;
