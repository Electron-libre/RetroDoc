//! What `generate --force` does to the cache before the run: an older one moved aside (CFV-13), the
//! results of the LLM passes removed.

use std::path::Path;

use anyhow::Context;
use retrodoc_core::cache::format as cache_format;
use retrodoc_core::cache::Artifact;

/// Moves a cache of an older format out of `.retrodoc/cache/`, so `--force` keeps none of its files
/// under the current format (CFV-13), and says where the hand edits went.
pub(super) fn set_aside_older(repo_root: &Path) -> anyhow::Result<()> {
    let Some(moved) = cache_format::set_aside_older(repo_root)
        .context("could not move the cache of an older format aside")?
    else {
        return Ok(());
    };
    println!(
        "The cache was of an older format: moved to {}, written again from scratch.",
        moved.dir
    );
    if !moved.hand_editable.is_empty() {
        println!(
            "Files you may have edited by hand, to copy back if they still apply: {}.",
            moved.hand_editable.join(", ")
        );
    }
    Ok(())
}

/// Removes the cached results of the LLM passes (see
/// [`Artifact::cleared_by_force`] for what is kept and why).
pub(super) fn clear_caches(repo_root: &Path) -> anyhow::Result<()> {
    for artifact in Artifact::ALL {
        if !artifact.cleared_by_force() {
            continue;
        }
        let file = artifact.path(repo_root);
        match std::fs::remove_file(&file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| format!("could not remove {}", file.display()))
            }
        }
    }
    Ok(())
}
