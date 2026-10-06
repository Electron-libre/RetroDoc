//! A small BM25 ranking over a few hundred documents, rebuilt at every start:
//! no persistence and no dependency. Lexical only (no embeddings): identifiers
//! and domain words match, a paraphrase does not.

use std::collections::HashMap;

/// Lowercase words of `text`, accents folded, split on anything that is not
/// a letter or a digit and inside `camelCase` words, with a trailing plural
/// `s` dropped (`contracts` and `contract` meet).
#[must_use]
pub fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut after_lower = false;
    for c in text.chars() {
        if !c.is_alphanumeric() {
            push_word(&mut tokens, &mut word);
            after_lower = false;
            continue;
        }
        if c.is_uppercase() && after_lower {
            push_word(&mut tokens, &mut word);
        }
        after_lower = c.is_lowercase() || c.is_numeric();
        word.extend(c.to_lowercase().map(fold_accent));
    }
    push_word(&mut tokens, &mut word);
    tokens
}

fn fold_accent(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'î' | 'ï' => 'i',
        'ô' | 'ö' => 'o',
        'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    }
}

fn push_word(tokens: &mut Vec<String>, word: &mut String) {
    if word.is_empty() {
        return;
    }
    let mut token = std::mem::take(word);
    if token.chars().count() > 3 && token.ends_with('s') && !token.ends_with("ss") {
        token.pop();
    }
    tokens.push(token);
}

const K1: f32 = 1.2;
const B: f32 = 0.75;

/// BM25 index; document `i` is the `i`-th text given to [`Bm25::new`].
pub struct Bm25 {
    /// Word -> (document, occurrences in it).
    postings: HashMap<String, Vec<(usize, u32)>>,
    lengths: Vec<usize>,
    average_length: f32,
}

impl Bm25 {
    #[must_use]
    pub fn new(texts: &[String]) -> Self {
        let mut postings: HashMap<String, Vec<(usize, u32)>> = HashMap::new();
        let mut lengths = Vec::with_capacity(texts.len());
        for (doc, text) in texts.iter().enumerate() {
            let tokens = tokenize(text);
            lengths.push(tokens.len());
            let mut counts: HashMap<String, u32> = HashMap::new();
            for token in tokens {
                *counts.entry(token).or_default() += 1;
            }
            for (token, count) in counts {
                postings.entry(token).or_default().push((doc, count));
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let average_length = if lengths.is_empty() {
            0.0
        } else {
            lengths.iter().sum::<usize>() as f32 / lengths.len() as f32
        };
        Self {
            postings,
            lengths,
            average_length,
        }
    }

    /// The documents sharing at least one word with `query`, best first,
    /// with their score; ties keep document order.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn search(&self, query: &str) -> Vec<(usize, f32)> {
        let total = self.lengths.len() as f32;
        let mut terms = tokenize(query);
        terms.sort();
        terms.dedup();
        let mut scores: HashMap<usize, f32> = HashMap::new();
        for term in terms {
            let Some(postings) = self.postings.get(&term) else {
                continue;
            };
            let containing = postings.len() as f32;
            let idf = (1.0 + (total - containing + 0.5) / (containing + 0.5)).ln();
            for &(doc, count) in postings {
                let tf = count as f32;
                let length = self.lengths[doc] as f32 / self.average_length;
                *scores.entry(doc).or_default() +=
                    idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * length));
            }
        }
        let mut ranked: Vec<(usize, f32)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_lowercased_unaccented_and_split_on_punctuation() {
        assert_eq!(
            tokenize("Résilier le contrat, vite!"),
            ["resilier", "le", "contrat", "vite"]
        );
    }

    #[test]
    fn identifiers_are_split_in_snake_and_camel_case() {
        assert_eq!(
            tokenize("sign_contract ContractSigner"),
            ["sign", "contract", "contract", "signer"]
        );
    }

    #[test]
    fn a_trailing_plural_s_is_dropped_from_long_words_only() {
        assert_eq!(tokenize("contracts as"), ["contract", "as"]);
    }

    fn index(texts: &[&str]) -> Bm25 {
        Bm25::new(&texts.iter().map(ToString::to_string).collect::<Vec<_>>())
    }

    #[test]
    fn the_document_with_the_rarer_word_comes_first() {
        let bm25 = index(&[
            "the invoice is paid by the customer",
            "the customer cancels the subscription",
            "the customer logs in",
        ]);
        let hits = bm25.search("cancel subscription");
        assert_eq!(hits.iter().map(|h| h.0).collect::<Vec<_>>(), [1]);
        let hits = bm25.search("customer subscription");
        assert_eq!(hits[0].0, 1);
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn a_shorter_document_beats_a_longer_one_with_the_same_word_count() {
        let bm25 = index(&[
            "refund filler filler filler filler filler filler filler",
            "refund policy",
        ]);
        assert_eq!(bm25.search("refund")[0].0, 1);
    }

    #[test]
    fn a_query_without_known_word_finds_nothing() {
        let bm25 = index(&["alpha beta"]);
        assert_eq!(bm25.search("gamma"), Vec::<(usize, f32)>::new());
        assert_eq!(bm25.search(""), Vec::<(usize, f32)>::new());
    }

    #[test]
    fn an_empty_corpus_finds_nothing() {
        assert_eq!(index(&[]).search("anything"), Vec::<(usize, f32)>::new());
    }
}
