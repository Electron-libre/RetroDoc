//! The context a route belongs to when its file declares it for another file. A routes file
//! declares the routes of many handlers, so the context that owns the file is rarely the one the
//! routes serve: a route is given to the context that owns the entry point file it names, found
//! by name (its resource, then the static segments of its path) against the names of the entry point
//! files (`name_words`: no framework convention), else it stays with its file. Only routes move: a
//! job or an action named after a resource is declared where it runs. A route that names a word of its
//! own file's name is that file's (a word as generic as `api` in both stays with the file: known limit).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::entry_points::{EntryKind, EntryPoint};
use crate::naming::normalize;

use super::{name_words, ResolvedContext};

/// The contexts that own the entry point files of each name.
#[derive(Debug, Default)]
pub struct EntryOwners {
    /// Normalized name of an entry point file (`name_words` joined) to the slugs of the contexts
    /// owning a file of that name.
    by_name: BTreeMap<String, BTreeSet<String>>,
}

/// The name an entry point file answers to.
fn file_key(file: &Path) -> String {
    normalize(&name_words(file).concat())
}

/// A path segment that names a resource: not a placeholder (`:id`, `*rest`, `{id}`, `<int:id>`,
/// `[id]`, `(.:format)`), not a version (`v1`). An extension (`signatures.json`) is dropped.
fn static_segment(segment: &str) -> Option<&str> {
    let name = segment.split('.').next().unwrap_or_default();
    let version = name
        .strip_prefix('v')
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    let plain = name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    (!name.is_empty() && !version && plain).then_some(name)
}

/// What a route names: the words of its resource and the static segments of its path (normalized).
struct Names {
    resource: Vec<String>,
    segments: Vec<String>,
}

impl Names {
    fn of(entry: &EntryPoint) -> Self {
        let resource = entry
            .resource
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(normalize)
            .collect();
        // `GET /path`: the verb is the upper-case word before the first space.
        let path = match entry.name.split_once(' ') {
            Some((verb, path)) if verb.chars().all(|c| c.is_ascii_uppercase()) => path,
            _ => &entry.name,
        };
        let segments = path
            .split('/')
            .filter_map(static_segment)
            .map(normalize)
            .collect();
        Self { resource, segments }
    }

    /// The words of the resource, whole then last word, most specific first.
    fn resource_names(&self) -> Vec<String> {
        let mut names = vec![self.resource.concat()];
        names.extend(self.resource.last().cloned());
        names.retain(|n| !n.is_empty());
        names.dedup();
        names
    }

    /// The segments of the path from the last, which names the object, to the first, which is often a
    /// prefix (`admin`, `api`).
    fn segment_names(&self) -> impl Iterator<Item = &String> {
        self.segments.iter().rev()
    }
}

impl EntryOwners {
    #[must_use]
    pub fn new(contexts: &[ResolvedContext]) -> Self {
        let mut by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for context in contexts {
            for file in &context.entry_files {
                by_name
                    .entry(file_key(file))
                    .or_default()
                    .insert(context.slug.clone());
            }
        }
        Self { by_name }
    }

    /// The context owning the entry point file `entry` (declared in `file`) is about, when exactly one
    /// context owns a file of the name it gives and the name is not `file`'s own. `None`: the entry
    /// point stays with its file.
    #[must_use]
    pub fn owner(&self, file: &Path, entry: &EntryPoint) -> Option<&str> {
        if entry.kind != EntryKind::HttpRoute {
            return None;
        }
        let names = Names::of(entry);
        let own: BTreeSet<String> = name_words(file).iter().map(|w| normalize(w)).collect();
        if names
            .resource
            .iter()
            .chain(&names.segments)
            .any(|w| own.contains(w))
        {
            return None;
        }
        // A name decides when exactly one context owns a file of that name. A resource name two
        // contexts share says nothing and the next one is tried; a path segment that some file bears
        // decides alone, so that a prefix (`admin`) never takes a route whose object is ambiguous.
        let unique = |name: &str| {
            let mut slugs = self.by_name.get(name)?.iter();
            match (slugs.next(), slugs.next()) {
                (Some(slug), None) => Some(slug.as_str()),
                _ => None,
            }
        };
        names
            .resource_names()
            .iter()
            .find_map(|name| unique(name))
            .or_else(|| {
                let known = names
                    .segment_names()
                    .find(|name| self.by_name.contains_key(*name))?;
                unique(known)
            })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::entry_points::EntryKind;

    fn context(slug: &str, entry_files: &[&str]) -> ResolvedContext {
        ResolvedContext {
            slug: slug.into(),
            name: slug.into(),
            description: String::new(),
            files: entry_files.iter().map(PathBuf::from).collect(),
            entry_files: entry_files.iter().map(PathBuf::from).collect(),
        }
    }

    fn entry(name: &str, resource: &str) -> EntryPoint {
        EntryPoint {
            kind: EntryKind::HttpRoute,
            name: name.into(),
            verb: String::new(),
            resource: resource.into(),
            description: String::new(),
            outputs: Vec::new(),
        }
    }

    const ROUTES: &str = "config/routes.rb";

    /// Three layouts of the same idea: a suffix, a verb folder, a bare name.
    fn owners() -> EntryOwners {
        EntryOwners::new(&[
            context("signing", &["app/handlers/signatures_handler.py"]),
            context(
                "contracts",
                &["src/contracts/index.ts", "app/admin/shares.rb"],
            ),
            context("sharing", &["internal/api/shares.go"]),
        ])
    }

    #[test]
    fn an_entry_point_goes_to_the_owner_of_the_file_it_names() {
        let owners = owners();
        let by_resource = entry("GET /anything", "signature");
        assert_eq!(
            owners.owner(Path::new(ROUTES), &by_resource),
            Some("signing")
        );
        let by_path = entry("POST /signatures/:id/send", "");
        assert_eq!(owners.owner(Path::new(ROUTES), &by_path), Some("signing"));
        let by_folder = entry("GET /contracts/:id", "");
        assert_eq!(
            owners.owner(Path::new(ROUTES), &by_folder),
            Some("contracts")
        );
    }

    #[test]
    fn a_resource_wins_over_the_path_and_the_last_word_is_tried() {
        let owners = owners();
        let routes = Path::new(ROUTES);
        let named = entry("GET /contracts/:id/sign", "contract signature");
        assert_eq!(owners.owner(routes, &named), Some("signing"));
        let last_word = entry("GET /x", "contract signature");
        assert_eq!(owners.owner(routes, &last_word), Some("signing"));
    }

    #[test]
    fn an_unknown_name_or_a_name_two_contexts_own_stays() {
        let owners = owners();
        let routes = Path::new(ROUTES);
        assert_eq!(owners.owner(routes, &entry("GET /health", "health")), None);
        assert_eq!(owners.owner(routes, &entry("GET /shares", "share")), None);
    }

    #[test]
    fn an_ambiguous_name_falls_back_to_the_next_one() {
        let owners = owners();
        let routes = Path::new(ROUTES);
        let entry = entry("GET /contracts/:id", "contract share");
        assert_eq!(owners.owner(routes, &entry), Some("contracts"));
    }

    #[test]
    fn only_routes_move() {
        let owners = owners();
        let mut job = entry("signatures", "signature");
        job.kind = EntryKind::Job;
        assert_eq!(owners.owner(Path::new("app/jobs/cleanup.rb"), &job), None);
    }

    #[test]
    fn an_entry_point_naming_its_own_file_stays() {
        let owners = owners();
        let own = Path::new("app/handlers/signatures_handler.py");
        assert_eq!(
            owners.owner(own, &entry("GET /signatures", "signature")),
            None
        );
        // Naming another file, it moves.
        let other = entry("GET /contracts", "contract");
        assert_eq!(owners.owner(own, &other), Some("contracts"));
    }

    #[test]
    fn a_prefix_never_takes_a_route_whose_object_is_ambiguous() {
        let owners = EntryOwners::new(&[
            context("admin", &["app/admin.rb"]),
            context("a", &["app/shares.rb"]),
            context("b", &["lib/shares.go"]),
        ]);
        let routes = Path::new(ROUTES);
        assert_eq!(owners.owner(routes, &entry("GET /admin/shares", "")), None);
        assert_eq!(
            owners.owner(routes, &entry("GET /admin/unknowns", "")),
            Some("admin")
        );
    }

    #[test]
    fn an_extension_is_not_part_of_the_name() {
        let owners = owners();
        let routes = Path::new(ROUTES);
        let entry = entry("GET /signatures.json", "");
        assert_eq!(owners.owner(routes, &entry), Some("signing"));
    }

    #[test]
    fn prefixes_and_placeholders_name_nothing() {
        let owners = owners();
        let routes = Path::new(ROUTES);
        let versioned = entry("GET /api/v1/signatures/{id}", "");
        assert_eq!(owners.owner(routes, &versioned), Some("signing"));
        assert_eq!(owners.owner(routes, &entry("GET /{tenant}/x", "")), None);
    }
}
