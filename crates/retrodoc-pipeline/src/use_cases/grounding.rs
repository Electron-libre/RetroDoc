use super::*;

use crate::citations::{cited_path, codes_of, RawCode};
use crate::entry_points::EntryKind;
use crate::features::{entry_point_id, tag_position};

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
    #[serde(default, deserialize_with = "lenient_steps")]
    #[schemars(with = "Vec<StepShape>")]
    pub(super) steps: Vec<RawStep>,
    #[serde(default)]
    pub(super) entry_points: Vec<String>,
    #[serde(default, deserialize_with = "lenient_text")]
    pub(super) primary_actor: Option<String>,
    #[serde(default, deserialize_with = "lenient_text")]
    pub(super) narrative: Option<String>,
}

/// A step as the prompt asks for it: what the schema sent with the request
/// describes (`RawStep` is read from more shapes than that).
#[derive(schemars::JsonSchema)]
#[allow(dead_code)] // only its schema is used
struct StepShape {
    actor: ActorShape,
    text: String,
    /// `"F2:10-24"`: the number of a file of the prompt and a line range.
    code: Vec<String>,
}

/// An actor of the list by its name, any other with its kind.
#[derive(schemars::JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)] // only its schema is used
enum ActorShape {
    Name(String),
    Object { name: String, kind: String },
}

/// Steps are read leniently: small models emit empty objects, steps
/// without an actor, or "references" that are just notes, and a model may mix
/// the shape asked for (`actor` name, `text`, `code`) with the one before
/// (`actor` object, `description`, `action`, `source_refs`), even in one step.
/// Incomplete steps are dropped and incomplete references ignored, rather than
/// rejecting the whole answer.
#[derive(Debug)]
pub(super) struct RawStep {
    /// The one text of the step (`text`), else its `description`.
    pub(super) description: String,
    pub(super) actor: Option<RawActor>,
    pub(super) action: Option<String>,
    /// From `code`, then from `source_refs`.
    pub(super) code: Vec<RawCode>,
}

impl RawStep {
    fn of(step: &serde_json::Map<String, serde_json::Value>) -> Self {
        let text = |key: &str| step.get(key).and_then(text_of);
        RawStep {
            description: text("text")
                .filter(|t| !t.trim().is_empty())
                .or_else(|| text("description"))
                .unwrap_or_default(),
            actor: step.get("actor").and_then(actor_of),
            action: text("action"),
            code: ["code", "source_refs"]
                .iter()
                .flat_map(|key| codes_of(step.get(*key)))
                .collect(),
        }
    }
}

fn lenient_steps<'de, D>(deserializer: D) -> Result<Vec<RawStep>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let steps = match value {
        Some(serde_json::Value::Array(items)) => items,
        _ => Vec::new(),
    };
    Ok(steps
        .iter()
        .filter_map(serde_json::Value::as_object)
        .map(RawStep::of)
        .collect())
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
    Ok(T::from_text(text_of(&value)))
}

/// The text of a value, read as [`lenient_text`] says.
fn text_of(value: &serde_json::Value) -> Option<String> {
    match value {
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
    }
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

#[derive(Debug, Deserialize)]
pub(super) struct RawActor {
    pub(super) name: String,
    /// `human` or `system`; empty when the actor is given by its name alone.
    #[serde(default)]
    pub(super) kind: String,
}

/// A step's actor: a name (its kind then comes from the actors list) or a
/// `{"name", "kind"}` object; anything else, or a blank name, is no actor.
fn actor_of(value: &serde_json::Value) -> Option<RawActor> {
    let actor = match value {
        serde_json::Value::String(name) => Some(RawActor {
            name: name.clone(),
            kind: String::new(),
        }),
        object @ serde_json::Value::Object(_) => serde_json::from_value(object.clone()).ok(),
        _ => None,
    };
    actor.filter(|a| !a.name.trim().is_empty())
}

/// The LLM's use cases of `feature` turned into [`UseCase`]s: those without
/// steps are dropped, slugs made unique among the feature's `existing` ones
/// and each other, steps grounded on the `cited_files` (in the order of their
/// numbers in the prompt), and entry points and
/// primary actor kept only when known.
pub(super) fn grounded_use_cases(
    raw: RawUseCases,
    feature: &Feature,
    existing: &[UseCase],
    input: &FeatureInput,
    cited_files: &[String],
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
        let mut use_case = UseCase {
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
            narrative_check: None,
        };
        without_invented_humans(&mut use_case, &input.entries, actors);
        produced.push(use_case);
    }
    produced
}

/// The actors that replace an invented human, by what triggers the behaviour.
pub(super) const CALLING_PROGRAM: &str = "Calling program";
pub(super) const EXTERNAL_PROVIDER: &str = "External provider";
pub(super) const SYSTEM: &str = "System";

/// Whether a person can start the entry point: a route or a command. A kind the inventory could
/// not name (`other`) is given the benefit of the doubt.
fn started_by_a_person(kind: EntryKind) -> bool {
    matches!(
        kind,
        EntryKind::HttpRoute | EntryKind::CliCommand | EntryKind::Other
    )
}

/// A use case reached only through entry points no person starts (a job, a consumer, a webhook,
/// a public method of a class) has no human actor: its human steps and primary actor are put
/// back to what triggers it, a calling program, the external provider or the system. The
/// entry points are those the use case cites, else all those of its feature; a feature with none
/// is not judged, nothing says who triggers it.
fn without_invented_humans(
    use_case: &mut UseCase,
    entries: &[(PathBuf, EntryPoint)],
    actors: &Actors,
) {
    let kinds: Vec<EntryKind> = if use_case.entry_points.is_empty() {
        entries.iter().map(|(_, e)| e.kind).collect()
    } else {
        entries
            .iter()
            .filter(|(file, e)| use_case.entry_points.contains(&entry_point_id(file, e)))
            .map(|(_, e)| e.kind)
            .collect()
    };
    if kinds.is_empty() || kinds.iter().any(|k| started_by_a_person(*k)) {
        return;
    }
    let name = if kinds.iter().all(|k| *k == EntryKind::PublicApi) {
        CALLING_PROGRAM
    } else if kinds.iter().all(|k| *k == EntryKind::Webhook) {
        EXTERNAL_PROVIDER
    } else {
        SYSTEM
    };
    let mut changed = false;
    for step in use_case.steps.iter_mut().filter(|s| is_human(&s.actor)) {
        step.actor = Actor {
            name: name.to_string(),
            kind: ActorKind::System,
        };
        changed = true;
    }
    // A known software actor (an external provider of the actors list) is a fine primary actor.
    let human_primary = use_case
        .primary_actor
        .as_deref()
        .and_then(|n| actors.canonical(n))
        .is_none_or(|a| a.kind == ActorKind::Human);
    if human_primary {
        use_case.primary_actor = None;
    }
    if changed || human_primary {
        tracing::info!(
            use_case = %use_case.slug,
            "human actor dropped: no route nor command starts this behaviour"
        );
    }
}

/// Numbers steps from 1 (skipping incomplete ones) and keeps only the source
/// references that point to a file actually shown to the LLM for this
/// feature, by its number in the prompt (`numbered`, from 1) or its path.
pub(super) fn ground_steps(
    raw_steps: Vec<RawStep>,
    numbered: &[String],
    actors: &Actors,
) -> Vec<Step> {
    let allowed: BTreeSet<String> = numbered.iter().cloned().collect();
    let complete = raw_steps.into_iter().filter_map(|raw| {
        let action = raw.action.unwrap_or_default();
        let Some(actor) = raw.actor else {
            tracing::warn!("step dropped: no actor");
            return None;
        };
        if action.trim().is_empty() && raw.description.trim().is_empty() {
            tracing::warn!("step dropped: no text");
            return None;
        }
        Some((raw.description, actor, action, raw.code))
    });
    complete
        .zip(1..)
        .map(|((description, actor, action, refs), order)| {
            let source_refs = refs
                .into_iter()
                .filter_map(|r| {
                    let Some(path) = cited_path(&r, numbered, &allowed) else {
                        tracing::warn!(path = ?r.path, "step reference dropped: file not in the feature");
                        return None;
                    };
                    let (start_line, end_line) = match (r.start_line, r.end_line) {
                        (Some(s), Some(e)) => (Some(s.min(e)), Some(s.max(e))),
                        lines => lines,
                    };
                    Some(SourceRef {
                        path,
                        start_line,
                        end_line,
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

/// The ids (`<file>#<name>`) of the known entry points an answer cites, each by its tag
/// (`E1`, as the prompt shows it), else by its id, else by its name alone when that is
/// unambiguous (exact, else case-insensitive). The unknown and the ambiguous are dropped.
pub(super) fn known_entry_points(
    cited: &[String],
    entries: &[(PathBuf, EntryPoint)],
) -> Vec<String> {
    let ids: Vec<String> = entries
        .iter()
        .map(|(file, entry)| entry_point_id(file, entry))
        .collect();
    let resolve = |name: &str| -> Option<usize> {
        let name = name.trim();
        if let Some(position) = tag_position(name, entries.len()) {
            return Some(position);
        }
        if let Some(position) = ids.iter().position(|id| id == name) {
            return Some(position);
        }
        let by_name = |same: fn(&str, &str) -> bool| {
            let mut found = (0..entries.len()).filter(|&i| same(&entries[i].1.name, name));
            let first = found.next()?;
            found.next().is_none().then_some(first)
        };
        by_name(|a, b| a == b).or_else(|| by_name(str::eq_ignore_ascii_case))
    };
    let mut known: Vec<String> = Vec::new();
    for name in cited {
        match resolve(name) {
            Some(position) if !known.contains(&ids[position]) => known.push(ids[position].clone()),
            Some(_) => {}
            None => {
                tracing::info!(entry_point = %name, "use case cites an unknown entry point, dropped");
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
