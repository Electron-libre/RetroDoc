//! The bounded sample of signals a brief is written from. Each signal gets
//! a short id (`S1`, `S2`...) the LLM cites; the ids are resolved back to the
//! origins of the signals. No LLM, fully deterministic.

use std::collections::HashMap;
use std::fmt::Write as _;

use retrodoc_ingest::signals::{Signal, SignalKind};

/// Characters of signals sent to the LLM (a few thousand tokens).
pub(crate) const SAMPLE_BUDGET: usize = 20_000;
/// Characters kept of one signal.
const MAX_PER_SIGNAL: usize = 1_500;
/// A signal is not squeezed below this (when its kind has budget for it).
const MIN_PER_SIGNAL: usize = 150;
/// Characters of the titles of the sections, kept out of the budget.
const TITLES_RESERVE: usize = 400;

/// The kinds in the order of the prompt, with their title and their share of
/// the budget (in percent; what a kind doesn't need goes to the others).
const KINDS: &[(SignalKind, &str, usize)] = &[
    (SignalKind::Manifest, "Project manifests", 6),
    (SignalKind::Tree, "Folder layout", 6),
    (SignalKind::DocSection, "Documentation", 26),
    (SignalKind::FeatureScenarios, "Behaviour scenarios", 12),
    (SignalKind::TestDescriptions, "Test descriptions", 12),
    (SignalKind::Schema, "Database schema", 12),
    (SignalKind::Migrations, "Migration history", 4),
    (SignalKind::I18n, "Texts shown to users", 12),
    (SignalKind::CommitSubject, "Commit history", 10),
];

#[derive(Debug, Clone, Default)]
pub(crate) struct Sample {
    /// The prompt section.
    pub text: String,
    /// Id (`S3`) to the origin of the signal (`README.md#Features`).
    pub origins: HashMap<String, String>,
}

impl Sample {
    /// Picks signals within `budget` characters. Each kind gets its share,
    /// the unused part of a share going to the kinds that need more; inside a
    /// kind the signals come in their own order (root docs first), each
    /// squeezed to an equal slice so one big file doesn't eat the others.
    pub fn build(signals: &[Signal], budget: usize) -> Self {
        let mut groups: Vec<Vec<&Signal>> = KINDS
            .iter()
            .map(|(kind, ..)| signals.iter().filter(|s| s.kind == *kind).collect())
            .collect();
        for (group, (kind, ..)) in groups.iter_mut().zip(KINDS) {
            if *kind == SignalKind::DocSection {
                // Stable: root documents first, the rest in path order.
                group.sort_by_key(|s| s.origin.split('#').next().is_some_and(|p| p.contains('/')));
            }
        }
        let demands: Vec<usize> = groups
            .iter()
            .map(|g| {
                g.iter()
                    .map(|s| s.text.chars().count().min(MAX_PER_SIGNAL) + header_len(s))
                    .sum()
            })
            .collect();
        // The commits have a fixed share that nothing else can use or lend,
        // so that new commits never move what the other kinds get.
        let commits = KINDS
            .iter()
            .position(|(kind, ..)| *kind == SignalKind::CommitSubject);
        let budget = budget.saturating_sub(TITLES_RESERVE);
        let commit_share = commits.map_or(0, |i| budget * KINDS[i].2 / 100);
        let mut others = demands.clone();
        if let Some(i) = commits {
            others[i] = 0;
        }
        let mut budgets = allocate(&others, budget - commit_share);
        if let Some(i) = commits {
            budgets[i] = demands[i].min(commit_share);
        }

        let mut sample = Sample::default();
        let mut next_id = 1;
        for (((_, title, _), group), kind_budget) in KINDS.iter().zip(&groups).zip(budgets) {
            let picked = pick(group, kind_budget);
            if picked.is_empty() {
                continue;
            }
            let _ = writeln!(sample.text, "== {title} ==");
            for (signal, text) in picked {
                let id = format!("S{next_id}");
                next_id += 1;
                let _ = writeln!(sample.text, "[{id}] {}\n{text}", signal.origin);
                sample.origins.insert(id, signal.origin.clone());
            }
            sample.text.push('\n');
        }
        sample
    }

    /// The sample without its commit section: what the brief is reused on.
    /// New commits arrive all the time and must not redo the brief, nor, by
    /// its fingerprint, every pass that reads it.
    pub fn text_without_commits(&self) -> String {
        let title = "== Commit history ==";
        match self.text.find(title) {
            Some(start) => self.text[..start].to_string(),
            None => self.text.clone(),
        }
    }
}

/// Water-filling: every kind gets its percentage, a kind that needs less
/// keeps only what it needs and the rest is shared again among the others.
fn allocate(demands: &[usize], total: usize) -> Vec<usize> {
    let mut budgets = vec![0; demands.len()];
    let mut open: Vec<usize> = (0..demands.len()).filter(|&i| demands[i] > 0).collect();
    let mut left = total;
    loop {
        let weight: usize = open.iter().map(|&i| KINDS[i].2).sum();
        if weight == 0 {
            break;
        }
        let satisfied: Vec<usize> = open
            .iter()
            .copied()
            .filter(|&i| demands[i] <= left * KINDS[i].2 / weight)
            .collect();
        if satisfied.is_empty() {
            for &i in &open {
                budgets[i] = left * KINDS[i].2 / weight;
            }
            break;
        }
        for i in satisfied {
            budgets[i] = demands[i];
            left -= demands[i];
            open.retain(|&j| j != i);
        }
    }
    budgets
}

/// What a signal costs besides its text: its id and origin line.
fn header_len(signal: &Signal) -> usize {
    signal.origin.chars().count() + 8
}

/// The signals that fit in `budget` (header lines included), each with its
/// text squeezed to a slice.
fn pick<'a>(group: &[&'a Signal], budget: usize) -> Vec<(&'a Signal, String)> {
    if group.is_empty() || budget == 0 {
        return Vec::new();
    }
    let slice = (budget / group.len()).clamp(MIN_PER_SIGNAL, MAX_PER_SIGNAL);
    let mut left = budget;
    let mut picked = Vec::new();
    for signal in group {
        let Some(room) = left.checked_sub(header_len(signal)) else {
            break;
        };
        let text = squeeze(&signal.text, slice.min(room));
        let used = text.chars().count() + header_len(signal);
        if text.is_empty() || used > left {
            break;
        }
        left -= used;
        picked.push((*signal, text));
    }
    picked
}

/// `text` cut at a line end within `max` characters, marked when cut.
fn squeeze(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max.saturating_sub(2)).collect();
    let cut = cut.rsplit_once('\n').map_or(cut.as_str(), |(head, _)| head);
    if cut.is_empty() {
        return String::new();
    }
    format!("{cut}\n…")
}
