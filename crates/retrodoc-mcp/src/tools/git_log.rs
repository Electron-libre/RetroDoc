//! The `git_log` tool.

use super::answer::NO_SOURCE_ACCESS;
use super::Docs;

impl Docs {
    /// The recent commits that changed a file the docs cite.
    #[must_use]
    pub fn git_log(&self, path: &str, limit: usize) -> String {
        let Some(source) = &self.source else {
            return NO_SOURCE_ACCESS.into();
        };
        source
            .git_log(path, limit)
            .unwrap_or_else(|reason| format!("{reason}\n"))
    }
}
