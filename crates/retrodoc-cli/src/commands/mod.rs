pub mod generate;
pub mod init;
pub mod scan;

use std::path::Path;

/// Commands planned by the architecture (PLAN.md §4) but not yet
/// implemented at this stage of the roadmap (PLAN.md §5): the coverage
/// report needs confidence scores, which arrive with the "confidence score"
/// phase.
pub fn not_implemented(command: &str, _path: &Path) {
    println!(
        "`retrodoc {command}` is not implemented yet — v1 is currently at the \"repo map\" phase (see PLAN.md §5)."
    );
}
