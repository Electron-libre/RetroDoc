pub mod init;
pub mod scan;

use std::path::Path;

/// Commands planned by the architecture (PLAN.md §4) but not yet implemented
/// at this stage of the roadmap (PLAN.md §5).
pub fn not_implemented(command: &str, _path: &Path) -> anyhow::Result<()> {
    println!(
        "`retrodoc {command}` is not implemented yet — v1 is currently at the \"foundation\" phase (see PLAN.md §5)."
    );
    Ok(())
}
