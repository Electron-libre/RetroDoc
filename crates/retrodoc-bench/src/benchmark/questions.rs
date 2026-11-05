//! The question set of `retrodoc ask` on a benchmark repository (spec `ask-question-set`,
//! `issues/ask_question_set.md`): questions whose answer is known, documented ones naming the reference
//! feature that answers them, undocumented ones naming the words of what the application does not do.
//! [`check_questions`] checks them against the reference and a checkout of the repository. Example
//! `question_set`.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::evidence::git_head;
use super::reference::Reference;
use crate::error::PipelineError;

/// `benchmark/<repo>/questions.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionSet {
    pub questions: Vec<Question>,
}

/// One question; exactly one of `feature` and `absent_terms` is set ([`QuestionSet::load`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub id: String,
    /// The question, in the words of a user of the application.
    pub question: String,
    /// What a right answer says.
    pub expected: String,
    /// A documented question: the reference feature that answers it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    /// An undocumented question: the distinctive words of what the application does not do, none of
    /// which may appear in the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absent_terms: Option<Vec<String>>,
}

/// What a question asks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionKind<'a> {
    /// The reference feature that answers it.
    Documented(&'a str),
    /// The words of what the application does not do.
    Undocumented(&'a [String]),
}

impl Question {
    /// What the question asks about; a loaded set always has one.
    #[must_use]
    pub fn kind(&self) -> Option<QuestionKind<'_>> {
        match (&self.feature, &self.absent_terms) {
            (Some(feature), None) => Some(QuestionKind::Documented(feature)),
            (None, Some(terms)) => Some(QuestionKind::Undocumented(terms)),
            _ => None,
        }
    }

    /// Why the question can't be used, if it can't.
    fn problem(&self) -> Option<String> {
        let blank = [
            ("id", self.id.as_str()),
            ("question", &self.question),
            ("expected", &self.expected),
        ]
        .into_iter()
        .chain(self.feature.as_deref().map(|f| ("feature", f)))
        .find(|(_, value)| value.trim().is_empty());
        if let Some((field, _)) = blank {
            return Some(format!("`{field}` is empty"));
        }
        match (&self.feature, &self.absent_terms) {
            (None, None) => Some("neither `feature` nor `absent_terms`".into()),
            (Some(_), Some(_)) => Some("both `feature` and `absent_terms`".into()),
            (None, Some(terms))
                if terms.is_empty() || terms.iter().any(|t| t.trim().is_empty()) =>
            {
                Some("`absent_terms` is empty or holds an empty term".into())
            }
            _ => None,
        }
    }
}

impl QuestionSet {
    /// Reads a question set and checks the form of each question.
    ///
    /// # Errors
    ///
    /// Returns an error when the file can't be read or parsed, or names the first question that has
    /// neither or both of `feature` and `absent_terms`, an empty field, or an id already used.
    pub fn load(path: &Path) -> Result<Self, PipelineError> {
        let raw = std::fs::read_to_string(path).map_err(|source| PipelineError::ArtifactIo {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&raw).map_err(|reason| PipelineError::InvalidReference {
            path: path.to_path_buf(),
            reason,
        })
    }

    fn parse(raw: &str) -> Result<Self, String> {
        let set: Self = serde_yaml::from_str(raw).map_err(|error| error.to_string())?;
        let mut ids = BTreeSet::new();
        for (index, question) in set.questions.iter().enumerate() {
            let name = if question.id.trim().is_empty() {
                format!("question {}", index + 1)
            } else {
                format!("question `{}`", question.id)
            };
            if let Some(problem) = question.problem() {
                return Err(format!("{name}: {problem}"));
            }
            if !ids.insert(question.id.as_str()) {
                return Err(format!("{name}: the id is used twice"));
            }
        }
        Ok(set)
    }
}

/// One line per question the reference or the tree contradicts: a documented question whose feature
/// the reference lacks or disputes, an undocumented one whose term the tree holds; and a first line
/// when the tree is a checkout at another commit than the reference's. Empty when the set is sound.
#[must_use]
pub fn check_questions(set: &QuestionSet, reference: &Reference, tree: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    if let Some(head) = git_head(tree) {
        if !head.starts_with(reference.commit.trim()) {
            problems.push(format!(
                "the tree is at `{head}`, the reference at `{}`",
                reference.commit.trim()
            ));
        }
    }
    let files = text_files(tree);
    for question in &set.questions {
        match question.kind() {
            Some(QuestionKind::Documented(name)) => {
                let feature = reference
                    .domains
                    .iter()
                    .flat_map(|d| &d.features)
                    .find(|f| f.name == name);
                match feature {
                    None => problems.push(format!(
                        "`{}`: the reference has no feature `{name}`",
                        question.id
                    )),
                    Some(f) if f.disputed.is_some() => problems.push(format!(
                        "`{}`: the feature `{name}` is disputed",
                        question.id
                    )),
                    Some(_) => {}
                }
            }
            Some(QuestionKind::Undocumented(terms)) => {
                for term in terms {
                    let needle = term.trim().to_lowercase();
                    for (path, text) in &files {
                        if holds_word(text, &needle) {
                            problems
                                .push(format!("`{}`: `{term}` appears in `{path}`", question.id));
                        }
                    }
                }
            }
            None => {}
        }
    }
    problems
}

/// The text files under `tree` (`.git` left out, files that are not UTF-8 skipped), by path relative
/// to it, lowercased.
fn text_files(tree: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut dirs = vec![tree.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if entry.file_name() != ".git" {
                    dirs.push(path);
                }
            } else if kind.is_file() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    let relative = path.strip_prefix(tree).unwrap_or(&path);
                    files.push((relative.display().to_string(), text.to_lowercase()));
                }
            }
        }
    }
    files.sort();
    files
}

/// Whether `text` holds `word` (lowercase both) with no letter, digit or `_` right before or after it.
fn holds_word(text: &str, word: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + word.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::reference::{ReferenceDomain, ReferenceFeature};

    fn reference() -> Reference {
        let feature = |name: &str, disputed: bool| ReferenceFeature {
            name: name.into(),
            description: format!("{name}."),
            use_cases: Vec::new(),
            evidence: Vec::new(),
            disputed: disputed.then(|| "one draft only".into()),
        };
        Reference {
            repository: "https://example.com/app".into(),
            commit: "0123456".into(),
            purpose: "An app.".into(),
            actors: vec!["User".into()],
            domains: vec![ReferenceDomain {
                name: "Bookmarks".into(),
                description: "Bookmarks.".into(),
                features: vec![feature("Share bookmarks", false), feature("Notes", true)],
            }],
            rules: Vec::new(),
            checked: None,
        }
    }

    fn set(raw: &str) -> QuestionSet {
        QuestionSet::parse(raw).unwrap()
    }

    const VALID: &str = "questions:
  - id: share
    question: Can other users see my links?
    expected: Yes, when the bookmark is shared.
    feature: Share bookmarks
  - id: two-factor
    question: Can I protect my account with a one-time code?
    expected: No, the application does not do it.
    absent_terms: [two-factor, 2fa]
";

    // spec: AQS-1 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn loads_documented_and_undocumented_questions() {
        let set = set(VALID);
        assert_eq!(
            set.questions[0].kind(),
            Some(QuestionKind::Documented("Share bookmarks"))
        );
        assert_eq!(
            set.questions[1].kind(),
            Some(QuestionKind::Undocumented(&[
                "two-factor".into(),
                "2fa".into()
            ]))
        );
    }

    // spec: AQS-12 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn refuses_a_question_of_the_wrong_form_naming_it() {
        let cases = [
            (
                "questions:\n  - {id: a, question: Q?, expected: E}\n",
                "question `a`: neither `feature` nor `absent_terms`",
            ),
            (
                "questions:\n  - {id: a, question: Q?, expected: E, feature: F, absent_terms: [x]}\n",
                "question `a`: both `feature` and `absent_terms`",
            ),
            (
                "questions:\n  - {id: a, question: ' ', expected: E, feature: F}\n",
                "question `a`: `question` is empty",
            ),
            (
                "questions:\n  - {id: a, question: Q?, expected: E, absent_terms: []}\n",
                "question `a`: `absent_terms` is empty or holds an empty term",
            ),
            (
                "questions:\n  - {id: a, question: Q?, expected: E, feature: F}\n  - {id: a, question: Q?, expected: E, feature: F}\n",
                "question `a`: the id is used twice",
            ),
        ];
        for (raw, expected) in cases {
            assert_eq!(QuestionSet::parse(raw).unwrap_err(), expected, "{raw}");
        }
        let unknown = QuestionSet::parse(
            "questions:\n  - {id: a, question: Q?, expected: E, feature: F, answer: x}\n",
        )
        .unwrap_err();
        assert!(unknown.contains("unknown field `answer`"), "{unknown}");
    }

    // spec: AQS-2 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn reports_a_feature_the_reference_lacks_or_disputes() {
        let tree = tempfile::tempdir().unwrap();
        let set = set("questions:
  - {id: pdf, question: Q?, expected: E, feature: Export to PDF}
  - {id: notes, question: Q?, expected: E, feature: Notes}
  - {id: share, question: Q?, expected: E, feature: Share bookmarks}
");
        assert_eq!(
            check_questions(&set, &reference(), tree.path()),
            [
                "`pdf`: the reference has no feature `Export to PDF`",
                "`notes`: the feature `Notes` is disputed",
            ]
        );
    }

    // spec: AQS-3 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn reports_an_absent_term_the_tree_holds_as_a_whole_word() {
        let tree = tempfile::tempdir().unwrap();
        std::fs::write(
            tree.path().join("settings.py"),
            "# TOTP codes\nUSE_TOTP = True\n",
        )
        .unwrap();
        std::fs::create_dir(tree.path().join("docs")).unwrap();
        std::fs::write(
            tree.path().join("docs/a.md"),
            "Totpx and 2fab are other words.\n",
        )
        .unwrap();
        std::fs::create_dir(tree.path().join(".git")).unwrap();
        std::fs::write(tree.path().join(".git/config"), "totp\n").unwrap();
        let set = set("questions:
  - {id: otp, question: Q?, expected: E, absent_terms: [TOTP, 2fa]}
");
        assert_eq!(
            check_questions(&set, &reference(), tree.path()),
            ["`otp`: `TOTP` appears in `settings.py`"]
        );
    }

    // spec: AQS-4 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn reports_a_tree_at_another_commit() {
        let tree = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tree.path()).unwrap();
        let index_tree = repo.index().unwrap().write_tree().unwrap();
        let signature = git2::Signature::now("t", "t@example.com").unwrap();
        let head = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "first",
                &repo.find_tree(index_tree).unwrap(),
                &[],
            )
            .unwrap()
            .to_string();
        let set = set(VALID);
        assert_eq!(
            check_questions(&set, &reference(), tree.path()),
            [format!(
                "the tree is at `{head}`, the reference at `0123456`"
            )]
        );
        let mut at_head = reference();
        at_head.commit = head;
        assert_eq!(
            check_questions(&set, &at_head, tree.path()),
            Vec::<String>::new()
        );
    }
}
