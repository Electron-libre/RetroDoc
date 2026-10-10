//! Domains from the application surface (`[pipeline] domains_from = "surface"`, ADR 0020): one call
//! groups the resources (entry points by resource) and the entities into business domains, framed by
//! the product brief; the files follow mechanically. No repo map is read. The unit assigned is the
//! resource, not the file: a file that serves two domains belongs to both.

use std::collections::BTreeSet;

use super::*;
use crate::naming::normalize;
use crate::response::complete_json;
use crate::surface::Resource;

/// Entities and resources listed in the prompt, the best connected first.
const MAX_ENTITIES: usize = 80;
const MAX_RESOURCES: usize = 150;
const MAX_DESCRIPTION_CHARS: usize = 120;

const SURFACE_DOMAINS_SYSTEM_PROMPT: &str = "You are analyzing a software application to identify \
its functional (business) domains. You are given its business entities and its resources: what \
users and systems can do, grouped by the business object they act on. Group the resources and the \
entities into a small set of business domains, each named after a business concept or activity \
(e.g. contract signing, billing, onboarding), never after a technical layer or a folder kind \
(services, models, controllers, views, helpers, api, jobs). A resource or an entity goes in the \
domain it serves most, and in exactly one. A resource is one of the quoted names of the second list, \
never an action, a method or an endpoint: copy resource and entity names verbatim from the lists. \
Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching this shape: \
{\"domains\":[{\"slug\":\"kebab-case\",\"name\":\"...\",\"description\":\"one or two sentences\",\
\"resources\":[\"...\"],\"entities\":[\"...\"]}]}.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawSurfaceDomains {
    #[serde(default)]
    domains: Vec<RawSurfaceDomain>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawSurfaceDomain {
    slug: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    resources: Vec<String>,
    #[serde(default)]
    entities: Vec<String>,
}

/// The names the model can cite, with no file path: a shown path made it answer with files.
fn surface_prompt(surface: &Surface, brief: &ProductBrief) -> String {
    let mut prompt = brief.prompt_head();
    prompt.push_str("Business entities:\n");
    for entity in surface.entities.iter().take(MAX_ENTITIES) {
        let description: String = entity
            .description
            .chars()
            .take(MAX_DESCRIPTION_CHARS)
            .collect();
        let _ = writeln!(prompt, "- \"{}\": {description}", entity.name);
    }
    prompt.push_str("\nResources (entry points by business object):\n");
    for resource in surface.resources.iter().take(MAX_RESOURCES) {
        let _ = writeln!(
            prompt,
            "- \"{}\" ({} entry point(s) with the actions: {})",
            resource.name,
            resource.entry_count,
            resource.verbs.join(", ")
        );
    }
    prompt
}

fn surface_fingerprint(surface: &Surface, brief: &ProductBrief, model: String) -> String {
    fingerprint(
        std::iter::once("domains from the surface".to_string())
            .chain(surface.fingerprint_parts())
            .chain((!brief.fingerprint().is_empty()).then(|| brief.fingerprint()))
            .chain(std::iter::once(model)),
    )
}

/// Builds the domains from the surface and persists them to `.retrodoc/cache/domains.yaml`.
/// A resource or an entity the model leaves out, or places twice, is not lost: the first placement
/// wins and what is left goes to the "uncategorized" domain. A name the surface does not know is
/// dropped. The saved domains are reused while the surface, the brief and the model are unchanged.
///
/// # Errors
///
/// Returns an error if the LLM call fails or the artifact can't be saved.
pub async fn build_domains_from_surface(
    repo_root: &Path,
    surface: &Surface,
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
) -> Result<DomainMap, PipelineError> {
    if surface.is_empty() {
        let map = DomainMap::default();
        map.save(repo_root)?;
        return Ok(map);
    }

    let mut prints = Fingerprints::load(repo_root);
    let input_print = surface_fingerprint(surface, brief, model_part(llm));
    if prints.domains.as_ref() == Some(&input_print) {
        if let Some(saved) = DomainMap::load(repo_root).filter(|m| !m.domains.is_empty()) {
            tracing::info!("domains unchanged, reused");
            return Ok(saved);
        }
    }

    let prompt = surface_prompt(surface, brief);
    let raw = complete_json::<RawSurfaceDomains>(
        llm,
        SURFACE_DOMAINS_SYSTEM_PROMPT,
        &prompt,
        "domains from the surface",
    )
    .await?
    .unwrap_or(RawSurfaceDomains {
        domains: Vec::new(),
    });

    let map = place(raw, surface);
    let layered = layer_named_domains(&map);
    if !layered.is_empty() {
        tracing::warn!(
            domains = ?layered,
            "domains named after a technical layer, not a business concept"
        );
    }
    map.save(repo_root)?;
    prints.domains = Some(input_print);
    prints.save(repo_root)?;
    Ok(map)
}

/// The model's domains with names resolved against the surface, the first placement kept, the files
/// attached, and what no domain claimed gathered in the uncategorized domain.
fn place(raw: RawSurfaceDomains, surface: &Surface) -> DomainMap {
    let mut free_resources: BTreeSet<String> = surface
        .resources
        .iter()
        .map(|r| normalize(&r.name))
        .collect();
    let mut free_entities: BTreeSet<String> = surface
        .entities
        .iter()
        .map(|e| normalize(&e.name))
        .collect();
    let mut domains: Vec<DomainCluster> = Vec::new();

    for domain in raw.domains {
        let resources: Vec<&Resource> = domain
            .resources
            .iter()
            .filter_map(|name| {
                let key = normalize(name);
                let found = surface.resources.iter().find(|r| normalize(&r.name) == key);
                if found.is_none() {
                    tracing::warn!(resource = %name, "domain cites a resource the surface does not know, dropped");
                }
                found.filter(|_| free_resources.remove(&key))
            })
            .collect();
        let entities: Vec<&crate::glossary::MergedEntity> = domain
            .entities
            .iter()
            .filter_map(|name| {
                let key = normalize(name);
                let found = surface.entities.iter().find(|e| normalize(&e.name) == key);
                if found.is_none() {
                    tracing::warn!(entity = %name, "domain cites an entity the surface does not know, dropped");
                }
                found.filter(|_| free_entities.remove(&key))
            })
            .collect();
        if resources.is_empty() && entities.is_empty() {
            tracing::warn!(domain = %domain.slug, "domain with nothing placed in it, dropped");
            continue;
        }
        if let Some(same) = domains.iter_mut().find(|d| d.slug == domain.slug) {
            extend(same, &resources, &entities);
        } else {
            let mut cluster = DomainCluster {
                slug: domain.slug,
                name: domain.name,
                description: domain.description,
                ..DomainCluster::default()
            };
            extend(&mut cluster, &resources, &entities);
            domains.push(cluster);
        }
    }

    let left_resources: Vec<&Resource> = surface
        .resources
        .iter()
        .filter(|r| free_resources.contains(&normalize(&r.name)))
        .collect();
    let left_entities: Vec<&crate::glossary::MergedEntity> = surface
        .entities
        .iter()
        .filter(|e| free_entities.contains(&normalize(&e.name)))
        .collect();
    if !left_resources.is_empty() || !left_entities.is_empty() {
        // Not a loss: the features pass places the entry points of these resources.
        tracing::info!(
            resources = left_resources.len(),
            entities = left_entities.len(),
            "the domains left resources or entities out, bucketed into \"{UNCATEGORIZED_SLUG}\""
        );
        // The model may have a domain of that name itself: one bucket, not two with the same slug.
        if let Some(rest) = domains.iter_mut().find(|d| d.slug == UNCATEGORIZED_SLUG) {
            extend(rest, &left_resources, &left_entities);
        } else {
            let mut rest = DomainCluster {
                slug: UNCATEGORIZED_SLUG.to_string(),
                name: "Uncategorized".to_string(),
                description: "Resources and entities no domain claimed".to_string(),
                ..DomainCluster::default()
            };
            extend(&mut rest, &left_resources, &left_entities);
            domains.push(rest);
        }
    }
    DomainMap { domains }
}

/// Adds resources and entities to a domain, with their files (the files of a resource, the home file of
/// an entity), keeping `paths` sorted and without repeats.
fn extend(
    cluster: &mut DomainCluster,
    resources: &[&Resource],
    entities: &[&crate::glossary::MergedEntity],
) {
    for resource in resources {
        cluster.resources.push(resource.name.clone());
        cluster.paths.extend(resource.files.iter().cloned());
    }
    for entity in entities {
        cluster.entities.push(entity.name.clone());
        cluster.paths.extend(entity.files.first().cloned());
    }
    cluster.paths.sort();
    cluster.paths.dedup();
}
