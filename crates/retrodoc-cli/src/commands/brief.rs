use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use retrodoc_ingest::signals::{self, Signal};
use retrodoc_ingest::IngestResult;
use retrodoc_llm::LlmProvider;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{Artifact, Claim, Evidence, ProductBrief};

use super::workspace::Workspace;

/// Origins shown after a claim.
const SHOWN_SOURCES: usize = 3;

/// Writes the product brief (one LLM call, or the saved
/// `.retrodoc/cache/product.yaml`) and prints it. `force` writes it again
/// and infers the sources of the schema and translations again. With
/// `signals_only` nothing is sent to the LLM: it prints how much evidence the
/// repository gives, per kind, with the saved sources or else a guess.
pub async fn run(
    path: &Path,
    force: bool,
    signals_only: bool,
    tracker: &UsageTracker,
) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;
    let config = &workspace.config;
    let mut ingest = workspace.ingest()?;
    ingest.existing_docs = super::docs::without_generated(ingest.existing_docs, config);

    if signals_only {
        let sources = retrodoc_pipeline::saved_or_sniffed(repo_root, &ingest.files);
        let all = signals::collect(repo_root, &ingest, &sources)
            .context("could not collect the signals")?;
        print!(
            "{}",
            render_volume(&all, retrodoc_pipeline::sample_chars(&all))
        );
        return Ok(());
    }

    let llm = super::usage::provider(&config.llm, tracker)?;
    tracker.set_pass("sources");
    let sources = retrodoc_pipeline::infer_sources(repo_root, &ingest, &llm, force)
        .await
        .context("failed to locate the schema and translations")?;
    let all =
        signals::collect(repo_root, &ingest, &sources).context("could not collect the signals")?;
    tracker.set_pass("brief");
    let brief = retrodoc_pipeline::build_brief(repo_root, &all, &llm, force)
        .await
        .context("failed to write the product brief")?;
    let Some(brief) = brief else {
        anyhow::bail!(
            "no product brief (no signal, or the LLM answer was unusable, see the warnings above)"
        );
    };
    print!("{}", render_brief(&brief));
    Ok(())
}

/// The saved product brief, or an empty one (the passes then run as they did
/// without it): what the standalone commands read. A file that can't be read
/// is reported and not used.
pub fn saved(repo_root: &Path) -> ProductBrief {
    let loaded = ProductBrief::load(repo_root);
    if loaded.is_none() && Artifact::Product.path(repo_root).exists() {
        tracing::warn!(
            "{} can't be read, running without the product brief",
            Artifact::Product.relative_path()
        );
    }
    loaded.unwrap_or_default()
}

/// The brief `generate` frames its passes with: the saved one as it is
/// (edited or not, it is refreshed by `retrodoc brief`, not behind the back
/// of every other pass), otherwise one written now from the signals of the
/// repository. Empty when there is no evidence or no usable answer. With
/// `with_evidence` (`brief.evidence`) come the signals, searched for the
/// extracts closest to a unit; otherwise the evidence is empty.
pub async fn for_generate(
    repo_root: &Path,
    ingest: &IngestResult,
    llm: &dyn LlmProvider,
    with_evidence: bool,
    tracker: &UsageTracker,
) -> anyhow::Result<(ProductBrief, Evidence)> {
    let saved_brief = Artifact::Product.path(repo_root).exists();
    if !saved_brief {
        println!("Writing the product brief…");
    }
    let sources = if saved_brief {
        retrodoc_pipeline::saved_or_sniffed(repo_root, &ingest.files)
    } else {
        tracker
            .in_pass(
                "sources",
                retrodoc_pipeline::infer_sources(repo_root, ingest, llm, false),
            )
            .await
            .context("failed to locate the schema and translations")?
    };
    let collected = signals::collect(repo_root, ingest, &sources);
    if saved_brief {
        // The signals only feed the extracts of the units: the saved brief
        // doesn't need them.
        let evidence = collected.map_or_else(
            |err| {
                tracing::warn!("signals not collected, no extracts for the units: {err}");
                Evidence::default()
            },
            |all| {
                if with_evidence {
                    Evidence::new(&all)
                } else {
                    Evidence::default()
                }
            },
        );
        return Ok((saved(repo_root), evidence));
    }
    let all = collected.context("could not collect the signals")?;
    let evidence = if with_evidence {
        Evidence::new(&all)
    } else {
        Evidence::default()
    };
    let brief = tracker
        .in_pass(
            "brief",
            retrodoc_pipeline::build_brief(repo_root, &all, llm, false),
        )
        .await
        .context("failed to write the product brief")?;
    Ok((brief.unwrap_or_default(), evidence))
}

/// The brief as printed: each claim with where it comes from, `(unsupported)`
/// when it cites nothing.
fn render_brief(brief: &ProductBrief) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Product brief ({}, editable):\n",
        Artifact::Product.relative_path()
    );
    let _ = writeln!(out, "Purpose: {}", claim_line(&brief.purpose));
    for (title, claims) in [
        ("Users and actors", &brief.users),
        ("Main business objects", &brief.objects),
        ("Capabilities", &brief.capabilities),
        ("External systems", &brief.external_systems),
    ] {
        if claims.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n{title}:");
        for claim in claims {
            let _ = writeln!(out, "  - {}", claim_line(claim));
        }
    }
    if !brief.open_questions.is_empty() {
        let _ = writeln!(out, "\nOpen questions:");
        for question in &brief.open_questions {
            let _ = writeln!(out, "  - {question}");
        }
    }
    out
}

fn claim_line(claim: &Claim) -> String {
    if !claim.is_supported() {
        return format!("{} (unsupported)", claim.text);
    }
    let mut shown: Vec<&str> = claim
        .sources
        .iter()
        .take(SHOWN_SOURCES)
        .map(String::as_str)
        .collect();
    if claim.sources.len() > SHOWN_SOURCES {
        shown.push("…");
    }
    format!("{} [{}]", claim.text, shown.join(", "))
}

/// Signals and characters per kind, and the size of the sample the LLM
/// would get.
fn render_volume(signals: &[Signal], sample_chars: usize) -> String {
    let mut per_kind: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for signal in signals {
        let entry = per_kind.entry(format!("{:?}", signal.kind)).or_default();
        entry.0 += 1;
        entry.1 += signal.text.chars().count();
    }
    let mut out = format!("{:<18} {:>8} {:>12}\n", "Signals", "count", "characters");
    for (kind, (count, chars)) in &per_kind {
        let _ = writeln!(out, "{kind:<18} {count:>8} {chars:>12}");
    }
    let total_chars: usize = per_kind.values().map(|(_, chars)| chars).sum();
    let _ = writeln!(
        out,
        "{:<18} {:>8} {total_chars:>12}\n\nSample sent to the LLM: {sample_chars} characters",
        "total",
        signals.len()
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrodoc_ingest::signals::SignalKind;

    fn claim(text: &str, sources: &[&str]) -> Claim {
        Claim {
            text: text.to_string(),
            sources: sources.iter().map(ToString::to_string).collect(),
        }
    }

    #[test]
    fn the_brief_shows_where_each_claim_comes_from() {
        let brief = ProductBrief {
            purpose: claim("Lets buyers pay.", &["README.md#Shop"]),
            users: vec![claim("Buyer", &["a", "b", "c", "d"]), claim("Agent", &[])],
            open_questions: vec!["Who refunds?".to_string()],
            ..ProductBrief::default()
        };
        let text = render_brief(&brief);
        assert!(
            text.contains("Purpose: Lets buyers pay. [README.md#Shop]"),
            "{text}"
        );
        assert!(text.contains("  - Buyer [a, b, c, …]"), "{text}");
        assert!(text.contains("  - Agent (unsupported)"), "{text}");
        assert!(text.contains("Open questions:\n  - Who refunds?"), "{text}");
        assert!(!text.contains("External systems"), "{text}");
    }

    #[test]
    fn the_volume_counts_signals_and_characters_per_kind() {
        let signal = |kind, text: &str| Signal {
            kind,
            origin: "x".to_string(),
            text: text.to_string(),
        };
        let text = render_volume(
            &[
                signal(SignalKind::DocSection, "abcd"),
                signal(SignalKind::DocSection, "ef"),
                signal(SignalKind::Schema, "t: a"),
            ],
            9,
        );
        let rows: Vec<Vec<&str>> = text
            .lines()
            .map(|l| l.split_whitespace().collect())
            .collect();
        assert!(rows.contains(&vec!["DocSection", "2", "6"]), "{text}");
        assert!(rows.contains(&vec!["Schema", "1", "4"]), "{text}");
        assert!(rows.contains(&vec!["total", "3", "10"]), "{text}");
        assert!(
            text.contains("Sample sent to the LLM: 9 characters"),
            "{text}"
        );
    }

    #[tokio::test]
    async fn signals_only_needs_no_llm_and_no_api_key() {
        let dir = tempfile::tempdir().unwrap();
        retrodoc_core::config::Config::write_default(dir.path(), false).unwrap();
        std::fs::write(dir.path().join("README.md"), "# Shop\nSells things.\n").unwrap();
        for args in [["init", "-q"], ["add", "."]] {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .status()
                .unwrap();
            assert!(status.success());
        }
        let status = std::process::Command::new("git")
            .args(["-c", "user.name=A", "-c", "user.email=a@example.com"])
            .args(["commit", "-q", "-m", "init"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(status.success());
        let tracker = UsageTracker::new();
        run(dir.path(), false, true, &tracker).await.unwrap();
        assert_eq!(tracker.report().total().calls, 0);
    }

    struct NoCall;

    #[async_trait::async_trait]
    impl LlmProvider for NoCall {
        async fn complete(
            &self,
            _: retrodoc_llm::CompletionRequest,
        ) -> Result<retrodoc_llm::CompletionResponse, retrodoc_llm::LlmError> {
            panic!("the saved brief must be used without asking the LLM");
        }
    }

    #[tokio::test]
    async fn generate_uses_the_saved_brief_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let mut brief = ProductBrief {
            purpose: claim("Sells things.", &["README.md"]),
            ..ProductBrief::default()
        };
        // An edit makes it differ from what was written: still used.
        brief.content_hash = "written-before-the-edit".to_string();
        brief.save(dir.path()).unwrap();
        let ingest = retrodoc_ingest::IngestResult {
            files: Vec::new(),
            history_by_path: std::collections::HashMap::new(),
            existing_docs: Vec::new(),
            commits: Vec::new(),
        };

        let (used, _) = for_generate(dir.path(), &ingest, &NoCall, false, &UsageTracker::new())
            .await
            .unwrap();

        assert_eq!(used.purpose.text, "Sells things.");
        assert_eq!(saved(dir.path()), used);
    }

    #[tokio::test]
    async fn the_extracts_of_each_unit_are_kept_only_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        ProductBrief {
            purpose: claim("Sells things.", &["README.md"]),
            ..ProductBrief::default()
        }
        .save(dir.path())
        .unwrap();
        let ingest = retrodoc_ingest::IngestResult {
            files: Vec::new(),
            history_by_path: std::collections::HashMap::new(),
            existing_docs: vec![retrodoc_ingest::existing_docs::ExistingDoc {
                path: "README.md".into(),
                content: "# Refunds\nA buyer asks a refund of a paid order.\n".to_string(),
            }],
            commits: Vec::new(),
        };
        let (root, ingest) = (dir.path(), &ingest);
        let section = |with| async move {
            let (_, evidence) = for_generate(root, ingest, &NoCall, with, &UsageTracker::new())
                .await
                .unwrap();
            evidence.section("refund order")
        };
        assert_eq!(section(false).await, "");
        assert!(section(true).await.contains("A buyer asks a refund"));
    }

    #[test]
    fn a_missing_or_unreadable_brief_is_an_empty_one() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(saved(dir.path()), ProductBrief::default());
        let path = Artifact::Product.path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "purpose: [oops").unwrap();
        assert_eq!(saved(dir.path()), ProductBrief::default());
    }
}
