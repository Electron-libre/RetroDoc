//! What the audit page shows (`specs/audit-dashboard.md`): the figures of
//! `retrodoc_mcp::audit` as text, and the heatmap of the debt, each cell
//! linked to the debt page filtered on its domain and kind.

use std::fmt::Write as _;

use retrodoc_core::model::ConfidenceScore;
use retrodoc_mcp::audit::{
    audit, format_duration, thousands, ModelRecord, PassRecord, RecordedProgress, Share, MAX_LEVEL,
};
use retrodoc_mcp::debt::DebtKind;
use retrodoc_mcp::GeneratedDocs;

use crate::debt::kind_value;
use crate::texts::WebTexts;
use crate::view::Confidence;

pub struct AuditPage {
    /// `None` when the run has no confidence score.
    pub confidence: Option<Confidence>,
    pub business_language: Option<Confidence>,
    pub coverage: Option<CoverageView>,
    pub stale: ShareView,
    pub cost: Option<CostView>,
    pub contexts: Option<ContextsView>,
    /// The names of the columns of the heatmap.
    pub kinds: Vec<&'static str>,
    pub rows: Vec<HeatRowView>,
    /// The levels above 0, for the legend.
    pub levels: Vec<u8>,
}

/// `count` of `total`, and its percentage.
pub struct ShareView {
    pub count: usize,
    pub total: usize,
    pub percent: usize,
}

impl From<Share> for ShareView {
    fn from(share: Share) -> Self {
        Self {
            count: share.count,
            total: share.total,
            percent: share.percent(),
        }
    }
}

pub struct CoverageView {
    pub placed: Option<ShareView>,
    pub cited: Option<ShareView>,
}

pub struct CostView {
    /// The day the run ended, `YYYY-MM-DD`.
    pub finished: String,
    pub passes: Vec<PassRow>,
    pub total: PassRow,
}

/// A pass, or the total, its counts grouped by thousands.
pub struct PassRow {
    pub name: String,
    pub duration: String,
    pub calls: String,
    pub prompt: String,
    pub cached: String,
    pub completion: String,
    /// Some calls reported no tokens: the sums are a lower bound.
    pub lower_bound: bool,
}

pub struct ContextsView {
    pub done: usize,
    pub total: usize,
    /// The names of the pending contexts, in the order of the map.
    pub pending: Vec<String>,
}

pub struct HeatRowView {
    pub domain: String,
    pub cells: Vec<HeatCellView>,
}

pub struct HeatCellView {
    pub count: usize,
    pub level: u8,
    pub kind: &'static str,
    /// The debt page on its domain and kind, `None` for no item.
    pub href: Option<String>,
}

/// The audit page of `docs`. Reads the cited files and the saved figures.
#[must_use]
pub fn audit_page(docs: &GeneratedDocs, texts: &WebTexts) -> AuditPage {
    let audit = audit(docs);
    let confidence = |value: Option<f32>| {
        value.map(|value| Confidence::of(Some(&ConfidenceScore::new(value, None))))
    };
    let domain_name = |slug: &str| {
        docs.domains()
            .iter()
            .find(|d| d.slug == slug)
            .map_or_else(|| slug.to_string(), |d| d.name.clone())
    };
    AuditPage {
        confidence: confidence(audit.confidence),
        business_language: confidence(audit.business_language),
        coverage: audit.coverage.map(|c| CoverageView {
            placed: c.placed.map(ShareView::from),
            cited: c.cited.map(ShareView::from),
        }),
        stale: audit.stale.into(),
        cost: audit.cost.map(|run| CostView {
            finished: run.finished_at.chars().take(10).collect(),
            total: pass_row(
                texts.audit_page.total,
                run.wall_ms,
                run.passes.iter().flat_map(|p| &p.models),
            ),
            passes: run
                .passes
                .iter()
                .filter(|p| p.models.iter().any(|m| m.calls > 0))
                .map(|p: &PassRecord| pass_row(&p.name, p.wall_ms, &p.models))
                .collect(),
        }),
        contexts: audit
            .contexts
            .map(|RecordedProgress { done, pending }| ContextsView {
                done: done.len(),
                total: done.len() + pending.len(),
                pending: pending.iter().map(|slug| domain_name(slug)).collect(),
            }),
        levels: (1..=MAX_LEVEL).collect(),
        kinds: DebtKind::ALL.iter().map(|k| texts.debt_kind(*k)).collect(),
        rows: audit
            .heatmap
            .into_iter()
            .map(|row| HeatRowView {
                domain: domain_name(&row.domain),
                cells: row
                    .cells
                    .iter()
                    .map(|cell| HeatCellView {
                        count: cell.count,
                        level: cell.level,
                        kind: texts.debt_kind(cell.kind),
                        href: (cell.count > 0).then(|| {
                            format!(
                                "/debt?kind={}&domain={}",
                                kind_value(cell.kind),
                                query_encoded(&row.domain)
                            )
                        }),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn pass_row<'a>(
    name: &str,
    wall_ms: u64,
    models: impl IntoIterator<Item = &'a ModelRecord>,
) -> PassRow {
    let (mut calls, mut prompt, mut cached, mut completion, mut without) = (0, 0, 0, 0, 0);
    for model in models {
        calls += model.calls;
        prompt += model.prompt_tokens;
        cached += model.cached_tokens;
        completion += model.completion_tokens;
        without += model.calls_without_usage;
    }
    PassRow {
        name: name.to_string(),
        duration: format_duration(std::time::Duration::from_millis(wall_ms)),
        calls: thousands(calls),
        prompt: thousands(prompt),
        cached: thousands(cached),
        completion: thousands(completion),
        lower_bound: without > 0,
    }
}

/// `value` as a query value: the unreserved characters kept, every other byte percent-encoded.
fn query_encoded(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: AUD-6 (internal: a slug the run writes is kebab-case; a hand-edited one may not be)
    #[test]
    fn a_domain_slug_is_encoded_in_the_link_of_its_cells() {
        assert_eq!(query_encoded("billing-v2"), "billing-v2");
        assert_eq!(query_encoded("a&b c#é"), "a%26b%20c%23%C3%A9");
    }
}
