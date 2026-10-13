//! The map of the bounded contexts (`issues/business_files_cap_on_large_application.md`): the
//! vertical slices a large application is documented in, one run at a time.
//!
//! It is built before any code is read, in two steps. The mechanical groups ([`groups`]) put the
//! source files around the names of the entry point files, with no LLM call. One call then reads
//! those groups (name, size, a sample of files) and the folders no group reached, framed by the
//! product brief, and merges the groups that serve one business concept into a context, sets the
//! technical ones aside as support, and gives each orphan folder to a context. A group the answer
//! forgot becomes a context of its own, so no file is lost.
//!
//! `contexts.yaml` keeps the rules (group names and folders per context), not the file lists: the
//! files of a context are found again from the current tree by [`ContextMap::resolve`], so a new
//! file joins a context by the same rules and a hand edit (moving a group or a folder) takes effect
//! at once. The map is reused as it is until `retrodoc contexts --force`: the code changing never
//! changes the map, which is what lets a context done stay done.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::brief::ProductBrief;
use crate::error::PipelineError;
use crate::features::{slugify, unique_slug};
use crate::naming::normalize;
use crate::response::complete_json;

mod domains;
mod groups;
mod route_owner;
mod run;

pub use domains::{domain_vocabulary, domains_from_contexts};
pub use groups::{contexts_of, mechanical_groups, name_words, Groups};
use groups::{group_of, group_words};
pub use route_owner::EntryOwners;
pub use run::{mark_done, plan_run, RunSlice};

/// Groups shown to the LLM, the largest first; the others go to the support.
const MAX_GROUPS: usize = 250;
/// Folders of orphan files shown to the LLM, the largest first.
const MAX_FOLDERS: usize = 100;
/// Leading directories that name a folder of orphan files.
const FOLDER_DEPTH: usize = 2;
/// A folder holding more orphan files than this is listed by its sub-folders.
const SPLIT_FOLDER_FILES: usize = 40;
/// Groups and folders a context needs to be worth a split call.
const MIN_ENTRIES_TO_SPLIT: usize = 3;
/// Times a context over the limit is split, a part of a split included.
const MAX_SPLIT_LEVELS: usize = 2;
/// Deepest folder a listing names.
const MAX_FOLDER_DEPTH: usize = 5;
const SAMPLE_FILES: usize = 6;
/// A context with more files than this is too big for one run: one more call splits it.
const MAX_CONTEXT_FILES: usize = 150;

const SYSTEM_PROMPT: &str = "You are mapping a large software application into bounded contexts: \
vertical slices that each hold one area of the business with everything that serves it (entry \
points, models, services, rules, policies), so that each can be documented on its own. You are \
given what the product is for, GROUPS of files (each named after a business object, from the \
names of the entry point files, with its size and a sample of its files) and FOLDERS holding \
files that belong to no group. Merge the groups that serve the same business concept (an object \
and its join with another one, an object and its variants) into one context named after a \
business concept or activity, never after a technical layer or a kind of folder (services, \
models, controllers, api, base, admin, jobs). Groups that are purely technical (a base class, a \
verb, framework plumbing, a technical namespace) go to `support`, which is not documented; a job, \
a mailer or an API endpoint that does a business operation is not technical, it belongs with the \
context it serves. Give each folder to the context its files serve, even when it is a layer \
(models, services, controllers, components): `support` is only for what no context uses (build \
and framework configuration, migrations, seeds, assets, generic helpers). Cite groups by their id (g1, g2…) and \
folders as listed, never invent either. Each group and each folder goes in exactly one context or \
in `support`. \
Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching this shape: \
{\"contexts\":[{\"slug\":\"kebab-case\",\"name\":\"...\",\"description\":\"one or two sentences\",\
\"groups\":[\"g1\",\"g4\"],\"folders\":[\"app/workflows/\"]}],\
\"support\":{\"groups\":[\"g9\"],\"folders\":[\"db/migrate/\"]}}.";

const SPLIT_SYSTEM_PROMPT: &str = "You are splitting one bounded context of a large software \
application into sub-contexts, because it is too big to be documented in one go. It holds several \
business sub-domains (an activity, a stage of the business process, a kind of object). You are \
given what the product is for, the context, its GROUPS of files (each named after a business \
object, with its size and a sample of its files) and its FOLDERS of files that belong to no group. \
Split it into 2 to 8 sub-contexts, each named after a business concept or activity, never after a \
technical layer or a kind of folder. Cite groups by their id (g1, g2…) and folders as listed, \
never invent either. Each group and each folder goes in exactly one sub-context; nothing is set \
aside. \
Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching this shape: \
{\"contexts\":[{\"slug\":\"kebab-case\",\"name\":\"...\",\"description\":\"one or two sentences\",\
\"groups\":[\"g1\",\"g4\"],\"folders\":[\"app/workflows/rules/\"]}]}.";

/// One bounded context: the groups (names as written by the mechanical step) and the folders that
/// make it, with a name and a description for the docs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedContext {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub groups: Vec<String>,
    /// Folders (with a trailing `/`) whose files belong to this context when no group claims them.
    #[serde(default)]
    pub folders: Vec<String>,
}

/// What is documented in no context.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Support {
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub folders: Vec<String>,
}

/// The saved artifact, `contexts.yaml`, editable by hand.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextMap {
    pub contexts: Vec<BoundedContext>,
    #[serde(default)]
    pub support: Support,
}

/// A context with the files the current tree gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContext {
    pub slug: String,
    pub name: String,
    pub description: String,
    /// Source files, in path order, entry point files included.
    pub files: Vec<PathBuf>,
    pub entry_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolved {
    pub contexts: Vec<ResolvedContext>,
    /// Source files in no context: the support, and what no rule reaches (a file added since).
    pub support: Vec<PathBuf>,
}

impl ContextMap {
    /// Loads `.retrodoc/cache/contexts.yaml`. Missing or unreadable: `None`.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::Contexts.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::Contexts.path(repo_root), self)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }

    /// Keeps the `features` of the contexts of the map, in the order of the map (the order within a
    /// context is kept): whatever the order the contexts were documented in, the docs come out the
    /// same, and a context the map no longer has leaves no feature behind.
    pub fn arrange(&self, features: &mut Vec<retrodoc_core::model::Feature>) {
        let position = |slug: &str| self.contexts.iter().position(|c| c.slug == slug);
        features.retain(|f| position(&f.domain_slug).is_some());
        features.sort_by_key(|f| position(&f.domain_slug));
    }

    /// The `sources` of the current tree by context. A file joins the context of the group its
    /// name or its directories give (the longest name first), else the context of the longest
    /// folder that holds it, else the support. `seeds` are the entry point files.
    #[must_use]
    pub fn resolve(&self, sources: &[PathBuf], seeds: &[PathBuf]) -> Resolved {
        // `None`: the support.
        let mut owner_of_group: BTreeMap<Vec<String>, Option<usize>> = BTreeMap::new();
        for (i, context) in self.contexts.iter().enumerate() {
            for group in &context.groups {
                owner_of_group.entry(group_words(group)).or_insert(Some(i));
            }
        }
        for group in &self.support.groups {
            owner_of_group.entry(group_words(group)).or_insert(None);
        }
        let names: BTreeSet<Vec<String>> = owner_of_group.keys().cloned().collect();
        let mut folders: Vec<(PathBuf, Option<usize>)> = Vec::new();
        // A folder written by hand may start with `./`.
        let folder = |f: &String| PathBuf::from(f.trim().trim_start_matches("./"));
        for (i, context) in self.contexts.iter().enumerate() {
            folders.extend(context.folders.iter().map(|f| (folder(f), Some(i))));
        }
        folders.extend(self.support.folders.iter().map(|f| (folder(f), None)));
        folders.sort_by_key(|(dir, _)| std::cmp::Reverse(dir.components().count()));
        let seed_set: BTreeSet<&PathBuf> = seeds.iter().collect();

        let mut out = Resolved {
            contexts: self
                .contexts
                .iter()
                .map(|c| ResolvedContext {
                    slug: c.slug.clone(),
                    name: c.name.clone(),
                    description: c.description.clone(),
                    files: Vec::new(),
                    entry_files: Vec::new(),
                })
                .collect(),
            support: Vec::new(),
        };
        for file in sources {
            // An entry point file is in the group of its own name, as when the groups were made.
            let own = seed_set.contains(file).then(|| name_words(file));
            let group = own
                .filter(|name| names.contains(name))
                .or_else(|| group_of(file, &names));
            let owner = group.map(|group| owner_of_group[&group]).or_else(|| {
                folders
                    .iter()
                    .find(|(dir, _)| file.starts_with(dir))
                    .map(|(_, owner)| *owner)
            });
            match owner {
                Some(Some(i)) => {
                    out.contexts[i].files.push(file.clone());
                    if seed_set.contains(file) {
                        out.contexts[i].entry_files.push(file.clone());
                    }
                }
                _ => out.support.push(file.clone()),
            }
        }
        for context in &mut out.contexts {
            context.files.sort();
            context.entry_files.sort();
        }
        out.support.sort();
        out
    }
}

struct ListedGroup {
    id: String,
    name: String,
    files: usize,
    sample: Vec<String>,
}

struct ListedFolder {
    path: String,
    files: usize,
    sample: Vec<String>,
}

/// What the LLM is shown, and what its citations resolve against.
struct Listing {
    groups: Vec<ListedGroup>,
    folders: Vec<ListedFolder>,
}

impl Listing {
    fn new(grouped: &Groups) -> Self {
        let mut by_size: Vec<(&String, &BTreeSet<PathBuf>)> = grouped.groups.iter().collect();
        by_size.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
        let groups = by_size
            .into_iter()
            .take(MAX_GROUPS)
            .enumerate()
            .map(|(i, (name, files))| ListedGroup {
                id: format!("g{}", i + 1),
                name: name.clone(),
                files: files.len(),
                sample: files
                    .iter()
                    .take(SAMPLE_FILES)
                    .map(|f| f.display().to_string())
                    .collect(),
            })
            .collect();

        let folder_of = |file: &Path, depth: usize| -> PathBuf {
            file.parent()
                .map(|p| p.components().take(depth).collect())
                .unwrap_or_default()
        };
        // Orphan files under each folder, at every depth that can name one.
        let mut sizes: BTreeMap<PathBuf, usize> = BTreeMap::new();
        let parent_depth = |file: &Path| file.parent().map_or(0, |p| p.components().count());
        for file in &grouped.orphans {
            // Once per folder that holds it, from the top one down to its own.
            let deepest = parent_depth(file).clamp(FOLDER_DEPTH, MAX_FOLDER_DEPTH);
            for depth in FOLDER_DEPTH..=deepest {
                *sizes.entry(folder_of(file, depth)).or_insert(0) += 1;
            }
        }
        let mut by_folder: BTreeMap<PathBuf, Vec<&PathBuf>> = BTreeMap::new();
        for file in &grouped.orphans {
            // A layer folder holding many orphans is listed by sub-folder, as deep as it stays
            // big: its files serve many contexts, and a family often is a folder of its own.
            let mut depth = FOLDER_DEPTH;
            while depth < MAX_FOLDER_DEPTH
                && depth < parent_depth(file)
                && sizes[&folder_of(file, depth)] > SPLIT_FOLDER_FILES
            {
                depth += 1;
            }
            let folder = folder_of(file, depth);
            if !folder.as_os_str().is_empty() {
                by_folder.entry(folder).or_default().push(file);
            }
        }
        let mut by_folder: Vec<(PathBuf, Vec<&PathBuf>)> = by_folder.into_iter().collect();
        by_folder.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
        let folders = by_folder
            .into_iter()
            .take(MAX_FOLDERS)
            .map(|(folder, files)| ListedFolder {
                path: format!("{}/", folder.display()),
                files: files.len(),
                sample: files
                    .iter()
                    .take(SAMPLE_FILES)
                    .map(|f| f.strip_prefix(&folder).unwrap_or(f).display().to_string())
                    .collect(),
            })
            .collect();
        Self { groups, folders }
    }

    fn prompt(&self, brief: &ProductBrief) -> String {
        let mut prompt = brief.prompt_head();
        prompt.push_str("Groups (id | files | name | sample):\n");
        for group in &self.groups {
            let _ = writeln!(
                prompt,
                "{} | {} | {} | {}",
                group.id,
                group.files,
                group.name,
                group.sample.join(", ")
            );
        }
        if !self.folders.is_empty() {
            prompt.push_str("\nFolders of files in no group (folder | files | sample):\n");
            for folder in &self.folders {
                let _ = writeln!(
                    prompt,
                    "{} | {} | {}",
                    folder.path,
                    folder.files,
                    folder.sample.join(", ")
                );
            }
        }
        prompt
    }
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct Response {
    #[serde(default)]
    contexts: Vec<RawContext>,
    #[serde(default)]
    support: RawSupport,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct RawContext {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    folders: Vec<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct RawSupport {
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    folders: Vec<String>,
}

/// The map of the contexts of the repo: the saved `contexts.yaml` as it is (edits included), or
/// the one the LLM answers over the mechanical groups of `sources` around the `seeds` (the entry
/// point files), saved for next time. `force` asks again whatever the file holds. An answer that
/// can't be read gives one context per group and is not saved, so the next run asks again. A saved
/// file that can't be read is left alone.
///
/// # Errors
///
/// Returns an error if an LLM call fails or the map can't be saved.
pub async fn build_context_map(
    repo_root: &Path,
    sources: &[PathBuf],
    seeds: &[PathBuf],
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
    force: bool,
) -> Result<ContextMap, PipelineError> {
    if !force {
        if Artifact::Contexts.path(repo_root).exists() && ContextMap::load(repo_root).is_none() {
            // Never overwrite what a person may be in the middle of editing.
            tracing::warn!(
                "{} can't be read; fix it or run `retrodoc contexts --force`",
                Artifact::Contexts.relative_path()
            );
            return Ok(ContextMap::default());
        }
        if let Some(saved) = ContextMap::load(repo_root).filter(|m| !m.is_empty()) {
            tracing::info!("contexts unchanged, reused");
            return Ok(saved);
        }
    }
    let grouped = mechanical_groups(sources, seeds);
    if grouped.groups.is_empty() {
        if sources.is_empty() {
            tracing::warn!("no source file: no context to map");
            return Ok(ContextMap::default());
        }
        // Nothing to cut the application by (a library has no controller): it is one context.
        tracing::warn!("no entry point file: the whole application is one context");
        let map = whole_application();
        map.save(repo_root)?;
        return Ok(map);
    }
    let listing = Listing::new(&grouped);
    let answer = complete_json::<Response>(
        llm,
        SYSTEM_PROMPT,
        &listing.prompt(brief),
        "map of the contexts",
    )
    .await?;
    let map = answer
        .map(|raw| place(raw, &listing, &grouped, sources))
        .filter(|map| !map.is_empty());
    let Some(map) = map else {
        tracing::warn!("the LLM answer for the contexts was unusable: one context per group");
        return Ok(one_context_per_group(&grouped));
    };
    let map = without_empty_contexts(map, sources, seeds);
    // Saved before the split: a failure there keeps the paid map.
    map.save(repo_root)?;
    let split = match split_large_contexts(map, &grouped, sources, seeds, brief, llm).await {
        Ok(split) => split,
        Err(error) => {
            tracing::warn!(
                "{} is saved without the split of its large contexts; `retrodoc contexts --force` asks again",
                Artifact::Contexts.relative_path()
            );
            return Err(error);
        }
    };
    let split = without_empty_contexts(split, sources, seeds);
    split.save(repo_root)?;
    Ok(split)
}

/// The map without the contexts that hold no file of the tree (every group of theirs was claimed by
/// a longer name): nothing to document there.
fn without_empty_contexts(map: ContextMap, sources: &[PathBuf], seeds: &[PathBuf]) -> ContextMap {
    let resolved = map.resolve(sources, seeds);
    let ContextMap { contexts, support } = map;
    let contexts = contexts
        .into_iter()
        .zip(resolved.contexts)
        .filter(|(context, own)| {
            let kept = !own.files.is_empty();
            if !kept {
                tracing::info!(context = %context.slug, "context without any file, dropped");
            }
            kept
        })
        .map(|(context, _)| context)
        .collect();
    ContextMap { contexts, support }
}

/// The map with each context over [`MAX_CONTEXT_FILES`] replaced by the sub-contexts one more call
/// makes of it. An unusable split leaves the context whole.
async fn split_large_contexts(
    map: ContextMap,
    grouped: &Groups,
    sources: &[PathBuf],
    seeds: &[PathBuf],
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
) -> Result<ContextMap, PipelineError> {
    let mut map = map;
    // A part of a split can still be big: it is split again, a bounded number of times.
    let mut whole: BTreeSet<String> = BTreeSet::new();
    for _ in 0..MAX_SPLIT_LEVELS {
        let resolved = map.resolve(sources, seeds);
        let big: Vec<bool> = map
            .contexts
            .iter()
            .zip(&resolved.contexts)
            .map(|(c, own)| own.files.len() > MAX_CONTEXT_FILES && !whole.contains(&c.slug))
            .collect();
        if !big.contains(&true) {
            break;
        }
        map = split_once(map, resolved, &big, &mut whole, grouped, brief, llm).await?;
    }
    let left = map.resolve(sources, seeds);
    let still_big = left
        .contexts
        .iter()
        .filter(|c| c.files.len() > MAX_CONTEXT_FILES)
        .count();
    if still_big > 0 {
        tracing::warn!(
            contexts = still_big,
            "contexts still over {MAX_CONTEXT_FILES} files after splitting: a run takes each whole"
        );
    }
    Ok(map)
}

/// One level: the contexts flagged in `big` replaced by their parts. The ones that could not be
/// split are added to `whole`, so they are not asked about again.
async fn split_once(
    map: ContextMap,
    resolved: Resolved,
    big: &[bool],
    whole: &mut BTreeSet<String>,
    grouped: &Groups,
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
) -> Result<ContextMap, PipelineError> {
    let ContextMap { contexts, support } = map;
    // The slugs of the contexts left whole are kept; a part that takes one gets a suffix.
    let mut slugs: Vec<String> = contexts
        .iter()
        .zip(big)
        .filter(|(_, big)| !**big)
        .map(|(c, _)| c.slug.clone())
        .collect();
    let mut out: Vec<BoundedContext> = Vec::new();
    for ((context, own), big) in contexts.into_iter().zip(resolved.contexts).zip(big) {
        if !big {
            out.push(context);
            continue;
        }
        if let Some(parts) = split_context(&context, &own.files, grouped, brief, llm).await? {
            for mut part in parts {
                part.slug = unique_slug(&part.slug, slugs.iter().map(String::as_str));
                slugs.push(part.slug.clone());
                out.push(part);
            }
        } else {
            whole.insert(context.slug.clone());
            out.push(context);
        }
    }
    Ok(ContextMap {
        contexts: out,
        support,
    })
}

/// The sub-contexts of `context` (which holds `files`), or `None` when the answer is unusable or
/// gives fewer than two. What the answer forgets stays with the first one, and so do the folders of
/// the context as a fallback (the sub-folders it cites are longer, so they win): no file is lost.
///
/// # Errors
///
/// Returns an error if the LLM call fails.
async fn split_context(
    context: &BoundedContext,
    files: &[PathBuf],
    grouped: &Groups,
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
) -> Result<Option<Vec<BoundedContext>>, PipelineError> {
    let in_context: BTreeSet<&PathBuf> = files.iter().collect();
    let groups: BTreeMap<String, BTreeSet<PathBuf>> = grouped
        .groups
        .iter()
        .filter(|(name, _)| context.groups.contains(name))
        .map(|(name, group_files)| {
            let own = group_files
                .iter()
                .filter(|f| in_context.contains(f))
                .cloned()
                .collect();
            (name.clone(), own)
        })
        .collect();
    let in_a_group: BTreeSet<&PathBuf> = groups.values().flatten().collect();
    let orphans: Vec<PathBuf> = files
        .iter()
        .filter(|f| !in_a_group.contains(f))
        .cloned()
        .collect();
    let sub = Groups { groups, orphans };
    let listing = Listing::new(&sub);
    if listing.groups.len() + listing.folders.len() < MIN_ENTRIES_TO_SPLIT {
        // With fewer than three groups and folders there is hardly anything to cut: the call
        // would answer the context itself, for the price of a call.
        return Ok(None);
    }
    let prompt = format!(
        "Context to split: {} - {}\n\n{}",
        context.name,
        context.description,
        listing.prompt(brief)
    );
    let what = format!("split of the context {}", context.slug);
    let Some(raw) = complete_json::<Response>(llm, SPLIT_SYSTEM_PROMPT, &prompt, &what).await?
    else {
        tracing::warn!(context = %context.slug, "the split of a large context was unusable, kept whole");
        return Ok(None);
    };
    let (mut parts, free) = place_cited(raw, &listing, files);
    if parts.contexts.len() < 2 {
        tracing::warn!(context = %context.slug, "the split gave fewer than two parts, kept whole");
        return Ok(None);
    }
    let placed: BTreeSet<String> = parts
        .contexts
        .iter()
        .flat_map(|c| &c.folders)
        .chain(&parts.support.folders)
        .cloned()
        .collect();
    let forgotten: Vec<String> = listing
        .folders
        .iter()
        .map(|f| f.path.clone())
        .filter(|path| !placed.contains(path))
        .collect();
    let first = &mut parts.contexts[0];
    first.groups.extend(free);
    first.groups.extend(unshown_groups(&listing, &sub));
    first.groups.extend(parts.support.groups);
    first.folders.extend(parts.support.folders);
    first.folders.extend(forgotten);
    // The context's own folders stay as a fallback on the first part, unless a part holds them
    // (the sub-folders the split lists are often the context's own): a folder in two contexts goes
    // to the first of them, whatever the split meant.
    first.folders.extend(
        context
            .folders
            .iter()
            .filter(|f| !placed.contains(*f))
            .cloned(),
    );
    first.folders.sort();
    first.folders.dedup();
    tracing::info!(
        context = %context.slug,
        parts = parts.contexts.len(),
        "large context split"
    );
    Ok(Some(parts.contexts))
}

/// One context for every source file: the empty folder is a prefix of all of them.
fn whole_application() -> ContextMap {
    ContextMap {
        contexts: vec![BoundedContext {
            slug: "application".to_string(),
            name: "Application".to_string(),
            description: "The whole application: no entry point file was found to cut it by."
                .to_string(),
            groups: Vec::new(),
            folders: vec![String::new()],
        }],
        support: Support::default(),
    }
}

fn one_context_per_group(grouped: &Groups) -> ContextMap {
    let mut map = ContextMap::default();
    for name in grouped.groups.keys() {
        add_group(&mut map.contexts, name);
    }
    map
}

/// A context of its own for a group, or the group added to the context of that slug.
fn add_group(contexts: &mut Vec<BoundedContext>, group: &str) {
    let slug = slugify(group);
    if let Some(same) = contexts.iter_mut().find(|c| c.slug == slug) {
        same.groups.push(group.to_string());
        return;
    }
    let name = group
        .split('_')
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ");
    contexts.push(BoundedContext {
        slug,
        name,
        description: String::new(),
        groups: vec![group.to_string()],
        folders: Vec::new(),
    });
}

/// The group a citation designates (its id, else its name), once: the first placement is kept.
/// A citation that is dropped (unknown, or already placed) is counted in `dropped`.
fn take_group(
    cited: &str,
    listing: &Listing,
    free: &mut BTreeSet<String>,
    dropped: &mut usize,
) -> Option<String> {
    let key = cited.trim();
    let found = listing
        .groups
        .iter()
        .find(|g| g.id.eq_ignore_ascii_case(key) || normalize(&g.name) == normalize(key));
    let taken = found.filter(|group| free.remove(&group.name));
    if taken.is_none() {
        *dropped += 1;
    }
    taken.map(|group| group.name.clone())
}

/// A cited folder (written with a trailing `/`) that holds a source file, once. A citation that is
/// dropped is counted in `dropped`.
fn take_folder(
    cited: &str,
    sources: &[PathBuf],
    taken: &mut BTreeSet<String>,
    dropped: &mut usize,
) -> Option<String> {
    let dir = cited.trim().trim_start_matches("./").trim_end_matches('/');
    let folder = (!dir.is_empty() && sources.iter().any(|s| s.starts_with(dir)))
        .then(|| format!("{dir}/"))
        .filter(|folder| taken.insert(folder.clone()));
    if folder.is_none() {
        *dropped += 1;
    }
    folder
}

/// The contexts of the answer with their citations resolved, what it forgot kept as contexts of
/// their own, and the groups too small to be shown set aside as support.
fn place(raw: Response, listing: &Listing, grouped: &Groups, sources: &[PathBuf]) -> ContextMap {
    let (mut map, free) = place_cited(raw, listing, sources);
    if !free.is_empty() {
        tracing::info!(
            groups = free.len(),
            "the answer left groups out, each is a context of its own"
        );
    }
    for group in free {
        add_group(&mut map.contexts, &group);
    }
    let unshown = unshown_groups(listing, grouped);
    if !unshown.is_empty() {
        tracing::warn!(
            groups = unshown.len(),
            "more groups than the {MAX_GROUPS} shown to the LLM: the smallest are set aside as support"
        );
    }
    map.support.groups.extend(unshown);
    map
}

/// The groups too small to have been shown to the LLM.
fn unshown_groups(listing: &Listing, grouped: &Groups) -> Vec<String> {
    let shown: BTreeSet<&String> = listing.groups.iter().map(|g| &g.name).collect();
    grouped
        .groups
        .keys()
        .filter(|name| !shown.contains(name))
        .cloned()
        .collect()
}

/// What the answer cites, resolved, and the shown groups it did not cite. A context the answer
/// calls `support` is the support, not a context.
fn place_cited(
    raw: Response,
    listing: &Listing,
    sources: &[PathBuf],
) -> (ContextMap, BTreeSet<String>) {
    let mut free: BTreeSet<String> = listing.groups.iter().map(|g| g.name.clone()).collect();
    let mut taken_folders: BTreeSet<String> = BTreeSet::new();
    let mut dropped = 0;
    let mut map = ContextMap::default();
    let mut support_groups: Vec<String> = Vec::new();
    let mut support_folders: Vec<String> = Vec::new();

    for context in raw.contexts {
        let groups: Vec<String> = context
            .groups
            .iter()
            .filter_map(|g| take_group(g, listing, &mut free, &mut dropped))
            .collect();
        let folders: Vec<String> = context
            .folders
            .iter()
            .filter_map(|f| take_folder(f, sources, &mut taken_folders, &mut dropped))
            .collect();
        if groups.is_empty() && folders.is_empty() {
            tracing::warn!(context = %context.slug, "context with nothing placed in it, dropped");
            continue;
        }
        let label = [&context.slug, &context.name]
            .into_iter()
            .chain(groups.first())
            .chain(folders.first())
            .find(|l| !l.trim().is_empty())
            .cloned()
            .unwrap_or_default();
        let slug = slugify(&label);
        if slug == "support" {
            tracing::info!("a context called support is the support");
            support_groups.extend(groups);
            support_folders.extend(folders);
            continue;
        }
        if let Some(same) = map.contexts.iter_mut().find(|c| c.slug == slug) {
            same.groups.extend(groups);
            same.folders.extend(folders);
        } else {
            map.contexts.push(BoundedContext {
                name: if context.name.is_empty() {
                    slug.clone()
                } else {
                    context.name
                },
                slug,
                description: context.description,
                groups,
                folders,
            });
        }
    }

    support_groups.extend(
        raw.support
            .groups
            .iter()
            .filter_map(|g| take_group(g, listing, &mut free, &mut dropped)),
    );
    support_folders.extend(
        raw.support
            .folders
            .iter()
            .filter_map(|f| take_folder(f, sources, &mut taken_folders, &mut dropped)),
    );
    map.support.groups = support_groups;
    map.support.folders = support_folders;

    if dropped > 0 {
        tracing::warn!(
            citations = dropped,
            "the answer cites groups or folders that do not exist or are placed twice: dropped, the first placement is kept"
        );
    }
    (map, free)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod two_runs_tests;
