use std::path::Path;

use anyhow::Context;
use retrodoc_pipeline::{EntryPoints, Glossary, Surface};

use super::workspace::repo_root;

/// Prints the application surface (entities and entry points by resource)
/// from the artifacts of `retrodoc glossary` and `retrodoc entry-points`, as
/// saved: before the entry points `generate` leaves out (the public API no
/// route reaches, the routes a routing file declares twice); makes no LLM call.
pub fn run(path: &Path) -> anyhow::Result<()> {
    let repo_root = repo_root(path)?;
    let glossary =
        Glossary::load(&repo_root).context("no glossary found — run `retrodoc glossary` first")?;
    let entry_points = EntryPoints::load(&repo_root)
        .context("no entry points found — run `retrodoc entry-points` first")?;

    let surface = Surface::new(&glossary, &entry_points);
    let section = surface.prompt_section();
    println!(
        "{} entit(ies), {} resource(s); prompt section: {} chars (~{} tokens).\n",
        surface.entities.len(),
        surface.resources.len(),
        section.chars().count(),
        section.chars().count() / 4
    );
    print!("{section}");
    Ok(())
}
