//! Descriptions of the test blocks of a file: what the application is
//! expected to do, in the words of the people who wrote the tests.

use std::collections::BTreeSet;

/// Test phrases kept per test file.
const MAX_PHRASES_PER_TEST_FILE: usize = 30;
/// Characters kept of each test phrase.
const MAX_PHRASE_CHARS: usize = 160;

/// Block keywords whose first string argument describes a behaviour, across
/// the `RSpec` / Jest / Mocha / Cucumber-like families.
const TEST_KEYWORDS: &[&str] = &[
    "describe", "context", "it", "scenario", "feature", "specify", "test", "example",
];

/// Descriptions of the test blocks of a file (`it "signs a contract" do`,
/// `test('rejects an expired token', …)`) and the words of `def test_foo_bar`
/// style names, deduplicated, in file order.
#[must_use]
pub fn test_phrases(content: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut phrases = Vec::new();
    for line in content.lines() {
        if phrases.len() >= MAX_PHRASES_PER_TEST_FILE {
            break;
        }
        let line = line.trim();
        let phrase = if let Some(name) = line.strip_prefix("def test_") {
            let name: String = name
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            Some(name.replace('_', " "))
        } else {
            TEST_KEYWORDS
                .iter()
                .find_map(|keyword| quoted_argument(line, keyword))
        };
        if let Some(phrase) = phrase {
            let phrase: String = phrase.trim().chars().take(MAX_PHRASE_CHARS).collect();
            if !phrase.is_empty() && seen.insert(phrase.clone()) {
                phrases.push(phrase);
            }
        }
    }
    phrases
}

/// The quoted string right after `keyword` (`it "x"`, `it("x"`, `it 'x'`).
fn quoted_argument(line: &str, keyword: &str) -> Option<String> {
    let rest = line.strip_prefix(keyword)?;
    let rest = rest.strip_prefix('(').unwrap_or(rest).trim_start();
    // A keyword followed directly by a letter is another word (`items`).
    if rest.len() == line.len() - keyword.len() && !line[keyword.len()..].starts_with(' ') {
        return None;
    }
    let quote = rest
        .chars()
        .next()
        .filter(|c| matches!(c, '"' | '\'' | '`'))?;
    let body = &rest[1..];
    body.find(quote).map(|end| body[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_test_descriptions() {
        let content = r##"
    RSpec.describe Contract do
      describe "#sign" do
        context 'when the signatory is a partner' do
          it "marks the contract as signed" do
      items.each { }
      it("rejects an expired token", () => {})
      it "marks the contract as signed" do
      def test_cancel_subscription_twice
    "##;
        assert_eq!(
            test_phrases(content),
            vec![
                "#sign",
                "when the signatory is a partner",
                "marks the contract as signed",
                "rejects an expired token",
                "cancel subscription twice",
            ]
        );
    }
}
