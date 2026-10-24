# Conventions specific to this codebase

- Repositories used for smoke tests may be confidential: never name them, nor quote their paths or code, in
  committed files (docs, `PLAN.md`, `issues/`, commit messages). Describe them generically.
- Everything is in English: doc comments, user-facing CLI/error strings, commit messages, and `PLAN.md`/
  `PRODUCT.md`. Don't reintroduce French — an earlier revision of this repo was French throughout and its
  history was rewritten to English (see `backup-fr-history` if you ever need the old wording for reference).
- `clippy::pedantic` is enabled workspace-wide (`[workspace.lints.clippy]` in the root `Cargo.toml`; each
  crate opts in via `[lints] workspace = true`). Any new `pub fn` returning `Result` needs a `# Errors` doc
  section; getters with no side effects need `#[must_use]`. Keep clippy warning-free before committing.
- Pipeline code that calls an `LlmProvider` is tested with a fake in-`#[cfg(test)]` implementation of the
  trait (`FakeLlm` in `testing.rs`: canned or scripted answers, call count, recorded prompts) rather than hitting the network — follow that pattern for
  new pipeline passes instead of adding integration tests that need `OPENROUTER_API_KEY`.
- Filesystem-touching tests use `tempfile::tempdir()` and, where git history matters, actually run `git
  init`/`git commit` in the tempdir (see `git_history.rs` and `walker.rs` tests) rather than mocking `git2`.
- A test that proves a requirement of a spec in `specs/` carries `// spec: ABC-n` right above it; the rules are in
  `specs/README.md` and `just check-specs` enforces them.
