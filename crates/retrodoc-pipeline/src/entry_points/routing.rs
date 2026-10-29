//! The routes a routing file declares for other files (`specs/routing-file-duplicates.md`). A
//! routing file (`config/routes.rb`) declares the routes its controllers also give, so the later
//! passes would turn each route into two features or use cases. The copy of the routing file is
//! left out when a controller covers each of its paths. Only the names the LLM gave are read, with
//! no framework convention: the paths are the words of a name that start with `/`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::{EntryKind, EntryPoints};
use crate::features::entry_point_id;

/// Other files that must share a path with a file for it to be a routing file.
const MIN_ROUTED_FILES: usize = 3;

/// A segment of a route path: a name, or a placeholder (`:id`, `{id}`...), all alike.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Name(String),
    Placeholder,
}

/// A route path as its segments (the root `/` has none), and whether a verb names it (a single
/// route) or not (a resource or a namespace).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RoutePath {
    segments: Vec<Segment>,
    verb: bool,
}

impl RoutePath {
    fn parse(word: &str, verb: bool) -> Self {
        let parts: Vec<&str> = word.split('/').filter(|s| !s.is_empty()).collect();
        let last = parts.len().saturating_sub(1);
        let segments = parts
            .iter()
            .enumerate()
            .map(|(i, part)| {
                let part = if i == last {
                    part.split('.').next().unwrap_or_default()
                } else {
                    part
                };
                if is_placeholder(part) {
                    Segment::Placeholder
                } else {
                    Segment::Name(part.to_string())
                }
            })
            .collect();
        RoutePath { segments, verb }
    }

    /// Whether `other` names the same path, or, for a path named with no verb, a path it starts;
    /// the root is covered by the root only. The verbs are not compared: the readings of a route
    /// and of its action do not always give it the same one.
    fn covered_by(&self, other: &RoutePath) -> bool {
        if self.segments.is_empty() || self.verb {
            return self.segments == other.segments;
        }
        other.segments.starts_with(&self.segments)
    }

    /// The path with its last segment replaced by `short`'s (`/stories/:id/upvote` and
    /// `/unvote` give `/stories/:id/unvote`), named with a verb as `short` is.
    fn sibling(&self, short: &RoutePath) -> RoutePath {
        let mut segments = self.segments[..self.segments.len() - 1].to_vec();
        segments.extend(short.segments.iter().cloned());
        RoutePath {
            segments,
            verb: short.verb,
        }
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

/// The HTTP verbs a route can be named with.
const VERBS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

/// A word naming HTTP verbs (`GET`, `GET/POST`); an acronym such as `REST` or `API` is not one.
fn is_verb(word: &str) -> bool {
    !word.is_empty() && word.split('/').all(|v| VERBS.contains(&v))
}

/// The paths an entry names: its words starting with `/`, parenthesised parts dropped, each named
/// with a verb when a verb word comes before it in the name.
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
    let mut verb = false;
    let mut paths = Vec::new();
    for word in plain.split(|c: char| c.is_whitespace() || c == ',') {
        if word.starts_with('/') {
            paths.push(RoutePath::parse(word, verb));
        } else if is_verb(word) {
            verb = true;
        }
    }
    paths
}

/// The ways each path of a name can be read: itself, and for a one-segment path after a longer
/// one, the sibling of the last longer one.
fn readings(paths: &[RoutePath]) -> Vec<Vec<RoutePath>> {
    let mut longer: Option<&RoutePath> = None;
    paths
        .iter()
        .map(|path| {
            let mut ways = vec![path.clone()];
            match longer {
                Some(long) if path.segments.len() == 1 => ways.push(long.sibling(path)),
                _ if path.segments.len() > 1 => longer = Some(path),
                _ => {}
            }
            ways
        })
        .collect()
}

/// An `http_route` entry: its index in its file and the paths it names.
type Route = (usize, Vec<RoutePath>);

/// The paths of the `http_route` entries of each file.
struct Routes<'a> {
    files: Vec<(&'a Path, Vec<Route>)>,
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
                    .enumerate()
                    .filter(|(_, e)| e.kind == EntryKind::HttpRoute)
                    .map(|(i, e)| (i, paths_of(&e.name)))
                    .collect();
                (path.as_path(), routes)
            })
            .collect();
        Routes { files }
    }

    /// The files other than `file` that `keep` accepts and with a path `matches` accepts.
    fn others_with<'s>(
        &'s self,
        file: &'s Path,
        keep: impl Fn(&Path) -> bool + 's,
        matches: impl Fn(&RoutePath) -> bool + 's,
    ) -> impl Iterator<Item = &'a Path> + 's {
        self.files
            .iter()
            .filter(move |(other, routes)| {
                *other != file
                    && keep(other)
                    && routes.iter().any(|(_, paths)| paths.iter().any(&matches))
            })
            .map(|(other, _)| *other)
    }

    /// At least half of its routes have a path another file covers.
    fn mostly_covered(&self, file: &Path, routes: &[Route]) -> bool {
        let covered = routes
            .iter()
            .filter(|(_, paths)| {
                paths.iter().any(|p| {
                    self.others_with(file, |_| true, |q| p.covered_by(q))
                        .next()
                        .is_some()
                })
            })
            .count();
        !routes.is_empty() && 2 * covered >= routes.len()
    }

    /// The routing files: mostly covered, and sharing a path with at least [`MIN_ROUTED_FILES`]
    /// other files. Known limit: a controller whose paths three routing files declare is one too.
    fn routing_files(&self) -> BTreeSet<&'a Path> {
        self.files
            .iter()
            .filter(|(file, routes)| {
                let paths: Vec<&RoutePath> = routes.iter().flat_map(|(_, p)| p).collect();
                let shared = self
                    .others_with(
                        file,
                        |_| true,
                        |q| paths.iter().any(|p| p.segments == q.segments),
                    )
                    .nth(MIN_ROUTED_FILES - 1);
                shared.is_some() && self.mostly_covered(file, routes)
            })
            .map(|(file, _)| *file)
            .collect()
    }
}

impl EntryPoints {
    /// The inventory without the routes a routing file declares for other files: an
    /// `http_route` entry of a routing file whose every path a file that is not one covers (see
    /// `specs/routing-file-duplicates.md`). Returns it with the ids (`<file>#<name>`) of the
    /// entries left out; the saved inventory is not touched, so the choice is made again at each
    /// run. Limits: only the names the LLM gave are read, a route declared under another path
    /// than its action's (an alias) stays, and a `generate` by contexts decides on the entry
    /// points read so far (a context done before the controllers were read keeps the copies).
    #[must_use]
    pub fn without_routes_declared_elsewhere(mut self) -> (Self, Vec<String>) {
        let left_out: BTreeMap<PathBuf, BTreeSet<usize>> = {
            let routes = Routes::of(&self);
            let routing = routes.routing_files();
            let covered = |file: &Path, path: &RoutePath| {
                routes
                    .others_with(
                        file,
                        |other| !routing.contains(other),
                        |q| path.covered_by(q),
                    )
                    .next()
                    .is_some()
            };
            routes
                .files
                .iter()
                .filter(|(file, _)| routing.contains(file))
                .map(|(file, entries)| {
                    let gone = entries
                        .iter()
                        .filter(|(_, paths)| {
                            !paths.is_empty()
                                && readings(paths)
                                    .iter()
                                    .all(|ways| ways.iter().any(|p| covered(file, p)))
                        })
                        .map(|(i, _)| *i)
                        .collect();
                    (file.to_path_buf(), gone)
                })
                .collect()
        };
        let mut ids = Vec::new();
        for (path, gone) in left_out {
            let Some(file) = self.files.get_mut(&path) else {
                continue;
            };
            let mut index = 0;
            file.entry_points.retain(|e| {
                let keep = !gone.contains(&index);
                index += 1;
                if !keep {
                    ids.push(entry_point_id(&path, e));
                }
                keep
            });
        }
        (self, ids)
    }
}

#[cfg(test)]
mod tests;
