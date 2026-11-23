//! The `read_source` tool.

use super::answer::NO_SOURCE_ACCESS;
use super::Docs;

impl Docs {
    /// A window of a file the docs cite, to check a claim against the code
    /// (`start_line` and `end_line` are 1-based and inclusive).
    #[must_use]
    pub fn read_source(
        &self,
        path: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> String {
        let Some(source) = &self.source else {
            return NO_SOURCE_ACCESS.into();
        };
        match source.read_source(path, start_line, end_line) {
            Ok(excerpt) => {
                let notice = self.stale_notice([path.strip_prefix("./").unwrap_or(path)]);
                format!("{}\n{notice}\n{}", excerpt.header, excerpt.body)
            }
            Err(reason) => format!("{reason}\n"),
        }
    }
}
