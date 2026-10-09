use super::*;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawUseCases {
    #[serde(default)]
    pub(super) use_cases: Vec<RawUseCase>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawUseCase {
    pub(super) slug: String,
    pub(super) name: String,
    #[serde(default, deserialize_with = "lenient_text")]
    pub(super) description: String,
    #[serde(default)]
    pub(super) steps: Vec<RawStep>,
    #[serde(default)]
    pub(super) entry_points: Vec<String>,
    #[serde(default, deserialize_with = "lenient_text")]
    pub(super) primary_actor: Option<String>,
    #[serde(default, deserialize_with = "lenient_text")]
    pub(super) narrative: Option<String>,
}

/// Steps are parsed leniently: small models emit empty objects, steps
/// without an actor, or "references" that are just notes. Incomplete steps
/// are dropped and incomplete references ignored, rather than rejecting the
/// whole answer.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawStep {
    #[serde(default, deserialize_with = "lenient_text")]
    pub(super) description: String,
    pub(super) actor: Option<RawActor>,
    #[serde(default, deserialize_with = "lenient_text")]
    pub(super) action: Option<String>,
    #[serde(default)]
    pub(super) source_refs: Vec<RawSourceRef>,
}

/// A text field read as a string, or as an object turned into text: the
/// `name` it carries, else its string values joined (not `kind`/`type`). Models copy the
/// `{"name", "kind"}` shape of a step's actor into `primary_actor`; that costs
/// the field's shape, not the whole feature. Anything else (a number, an
/// object without text) is no text.
fn lenient_text<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: FromText,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let text = match &value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Object(map) => {
            let text = if let Some(serde_json::Value::String(name)) = map.get("name") {
                Some(name.clone())
            } else {
                // `kind` and `type` label an actor, they are not its text.
                let joined: Vec<&str> = map
                    .iter()
                    .filter(|(key, _)| !matches!(key.as_str(), "kind" | "type"))
                    .filter_map(|(_, value)| value.as_str())
                    .collect();
                (!joined.is_empty()).then_some(joined.join(" "))
            };
            tracing::debug!(?text, "text field given as an object, converted to text");
            text
        }
        serde_json::Value::Null => None,
        other => {
            tracing::debug!(%other, "text field of an unexpected type, ignored");
            None
        }
    };
    Ok(T::from_text(text))
}

/// What a lenient text field is built from: the text, if any.
trait FromText {
    fn from_text(text: Option<String>) -> Self;
}

impl FromText for String {
    fn from_text(text: Option<String>) -> Self {
        text.unwrap_or_default()
    }
}

impl FromText for Option<String> {
    fn from_text(text: Option<String>) -> Self {
        text
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawActor {
    pub(super) name: String,
    #[serde(default)]
    pub(super) kind: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawSourceRef {
    pub(super) path: Option<String>,
    pub(super) start_line: Option<u32>,
    pub(super) end_line: Option<u32>,
}

/// The LLM's use cases of `feature` turned into [`UseCase`]s: those without
/// steps are dropped, slugs made unique among the feature's `existing` ones
/// and each other, steps grounded on the `cited_files`, and entry points and
/// primary actor kept only when known.
pub(super) fn grounded_use_cases(
    raw: RawUseCases,
    feature: &Feature,
    existing: &[UseCase],
    input: &FeatureInput,
    cited_files: &BTreeSet<String>,
    actors: &Actors,
) -> Vec<UseCase> {
    let mut produced: Vec<UseCase> = Vec::new();
    for raw_use_case in raw.use_cases {
        if raw_use_case.steps.is_empty() {
            tracing::warn!(use_case = %raw_use_case.slug, "use case dropped: no steps");
            continue;
        }
        let taken = existing
            .iter()
            .chain(&produced)
            .filter(|u| u.feature_slug == feature.slug)
            .map(|u| u.slug.as_str());
        let slug = unique_slug(&raw_use_case.slug, taken);
        produced.push(UseCase {
            slug,
            feature_slug: feature.slug.clone(),
            name: raw_use_case.name,
            description: raw_use_case.description,
            steps: ground_steps(raw_use_case.steps, cited_files, actors),
            business_language: None,
            entry_points: known_entry_points(&raw_use_case.entry_points, &input.entries),
            narrative: raw_use_case
                .narrative
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty()),
            primary_actor: raw_use_case
                .primary_actor
                .as_deref()
                .and_then(|name| actors.canonical(name))
                .map(|actor| actor.name.clone()),
            diagram_mermaid: None,
            confidence: None,
        });
    }
    produced
}

/// Numbers steps from 1 (skipping incomplete ones) and keeps only the source
/// references that point to a file actually shown to the LLM for this
/// feature.
pub(super) fn ground_steps(
    raw_steps: Vec<RawStep>,
    allowed: &BTreeSet<String>,
    actors: &Actors,
) -> Vec<Step> {
    let complete = raw_steps.into_iter().filter_map(|raw| {
        let (Some(actor), Some(action)) = (raw.actor, raw.action) else {
            tracing::warn!("step dropped: no actor or no action");
            return None;
        };
        Some((raw.description, actor, action, raw.source_refs))
    });
    complete
        .zip(1..)
        .map(|((description, actor, action, refs), order)| {
            let source_refs = refs
                .into_iter()
                .filter_map(|r| {
                    let cited = r.path?;
                    let Some(path) = resolve_cited_path(&cited, allowed) else {
                        tracing::warn!(path = %cited, "step reference dropped: file not in the feature");
                        return None;
                    };
                    Some(SourceRef {
                        path,
                        start_line: r.start_line,
                        end_line: r.end_line,
                    })
                })
                .collect();
            Step {
                order,
                description,
                actor: resolve_actor(&actor, actors),

                action,
                source_refs,
            }
        })
        .collect()
}

/// The step's actor as the known list spells it (name and kind), when the
/// LLM named a known one. A human actor outside a non-empty list is kept but
/// logged at `info`: the list is the vocabulary the use cases should use, but
/// the step is not lost.
pub(super) fn resolve_actor(raw: &RawActor, actors: &Actors) -> Actor {
    if let Some(known) = actors.canonical(&raw.name) {
        return Actor {
            name: known.name.clone(),
            kind: known.kind,
        };
    }
    let kind = if raw.kind.eq_ignore_ascii_case("human") {
        ActorKind::Human
    } else {
        ActorKind::System
    };
    if kind == ActorKind::Human && !actors.is_empty() {
        tracing::info!(actor = %raw.name, "human actor outside the known actors");
    }
    Actor {
        name: raw.name.clone(),
        kind,
    }
}

/// Maps a path cited by the LLM to the feature file it designates: an exact
/// match, or a unique file the citation is a path suffix of (or the other way
/// round). Models often shorten `crate/src/a.rs` to `src/a.rs`.
pub(crate) fn resolve_cited_path(cited: &str, allowed: &BTreeSet<String>) -> Option<String> {
    let cited = cited.trim().trim_start_matches("./");
    if allowed.contains(cited) {
        return Some(cited.to_string());
    }
    let mut matches = allowed.iter().filter(|file| {
        file.ends_with(&format!("/{cited}")) || cited.ends_with(&format!("/{file}"))
    });
    match (matches.next(), matches.next()) {
        (Some(only), None) => Some(only.clone()),
        _ => None,
    }
}

/// The entry point names of an answer that designate a known entry point
/// (exact, else case-insensitive), as the inventory spells them.
pub(super) fn known_entry_points(
    cited: &[String],
    entries: &[(PathBuf, EntryPoint)],
) -> Vec<String> {
    let mut known: Vec<String> = Vec::new();
    for name in cited {
        let found = entries
            .iter()
            .map(|(_, entry)| entry.name.as_str())
            .find(|n| *n == name || n.eq_ignore_ascii_case(name.trim()));
        match found {
            Some(found) if !known.iter().any(|k| k == found) => known.push(found.to_string()),
            Some(_) => {}
            None => {
                tracing::warn!(entry_point = %name, "use case cites an unknown entry point, dropped");
            }
        }
    }
    known
}

/// Whether the actor is a person rather than a software component.
#[must_use]
pub(crate) fn is_human(actor: &Actor) -> bool {
    actor.kind == ActorKind::Human
}
