//! The loop shared by the passes that read many small files per LLM call
//! (glossary, entry points).
//!
//! The changed files, already cut in chunks, are grouped in batches of about
//! [`BATCH_CHARS`] of code. A batch whose answer is unusable is retried file
//! by file. A file is handed back only once all its chunks are answered, so
//! an unusable answer leaves it to be retried whole next run, and the state
//! is checkpointed after every batch: a failure later in a long run (a call
//! timing out) must not lose the batches already read.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_llm::LlmProvider;
use serde::de::DeserializeOwned;

use crate::error::PipelineError;
use crate::progress::Progress;
use crate::response::complete_json;

/// Characters of code per LLM call.
const BATCH_CHARS: usize = 12_000;

/// One chunk of a changed file: (path, hash of the whole file, chunk text).
pub(crate) type PendingChunk = (PathBuf, String, String);

/// What distinguishes one batched pass from another. `R` is the answer the
/// LLM gives, `T` what is found in a file, `S` the state the pass builds.
pub(crate) struct BatchedRead<'a, R, T, S> {
    /// Name of the pass, for the progress lines and the log.
    pub pass: &'static str,
    /// What a batch holds, for the progress lines ("model file(s)").
    pub unit: &'static str,
    pub system_prompt: &'static str,
    /// First line of the user prompt, before the files.
    pub header: &'a str,
    /// What the answer found, by the file of the batch it belongs to.
    pub attribute: fn(R, &[PendingChunk]) -> BTreeMap<String, Vec<T>>,
    /// All the chunks of a file are read: record what was found in it.
    pub finish: fn(&mut S, &Path, &str, Vec<T>),
    /// Saves the state after a batch.
    pub checkpoint: &'a dyn Fn(&S) -> Result<(), PipelineError>,
}

impl<R: DeserializeOwned, T, S> BatchedRead<'_, R, T, S> {
    /// Reads `pending` batch by batch, feeding `state`.
    ///
    /// # Errors
    ///
    /// Returns an error if an LLM call fails or `checkpoint` does; the
    /// batches already read were checkpointed.
    pub async fn run(
        &self,
        llm: &dyn LlmProvider,
        pending: &[PendingChunk],
        state: &mut S,
    ) -> Result<(), PipelineError> {
        // Chunks of a file not yet answered, and what was found in the
        // answered ones.
        let mut remaining: BTreeMap<&Path, usize> = BTreeMap::new();
        for (path, _, _) in pending {
            *remaining.entry(path.as_path()).or_insert(0) += 1;
        }
        let mut partial: BTreeMap<&Path, Vec<T>> = BTreeMap::new();

        let batches = batches(pending);
        let mut progress = Progress::new(self.pass, batches.len());
        for batch in batches {
            progress.begin(&format!(
                "{} {}, from {}",
                batch.len(),
                self.unit,
                batch[0].0.display()
            ));
            let mut queue: VecDeque<&[PendingChunk]> = VecDeque::from([batch]);
            while let Some(group) = queue.pop_front() {
                let mut prompt = String::from(self.header);
                for (path, _, content) in group {
                    let _ = write!(prompt, "\n--- {} ---\n{content}\n", path.display());
                }
                let Some(response) =
                    complete_json::<R>(llm, self.system_prompt, &prompt, self.pass).await?
                else {
                    if group.len() > 1 {
                        tracing::warn!(
                            items = group.len(),
                            "unusable answer for a batch, retrying its files one by one"
                        );
                        queue.extend(group.chunks(1));
                    }
                    continue;
                };

                let mut found = (self.attribute)(response, group);
                for (path, hash, _) in group {
                    let key = path.to_string_lossy();
                    if let Some(items) = found.remove(key.as_ref()) {
                        partial.entry(path).or_default().extend(items);
                    }
                    let left = remaining.entry(path).or_insert(1);
                    *left -= 1;
                    if *left == 0 {
                        let items = partial.remove(path.as_path()).unwrap_or_default();
                        (self.finish)(state, path, hash, items);
                    }
                }
            }
            (self.checkpoint)(state)?;
        }
        Ok(())
    }
}

/// The files whose attribution key (`path` as a string) appears in `group`.
pub(crate) fn group_paths(group: &[PendingChunk]) -> BTreeSet<String> {
    group
        .iter()
        .map(|(path, _, _)| path.to_string_lossy().into_owned())
        .collect()
}

/// Groups files so that each batch holds about [`BATCH_CHARS`] of code
/// (at least one file).
fn batches(files: &[PendingChunk]) -> Vec<&[PendingChunk]> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut size = 0;
    for (i, (_, _, content)) in files.iter().enumerate() {
        let len = content.chars().count();
        if i > start && size + len > BATCH_CHARS {
            out.push(&files[start..i]);
            start = i;
            size = 0;
        }
        size += len;
    }
    if start < files.len() {
        out.push(&files[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::Deserialize;

    use crate::testing::FakeLlm;

    #[test]
    fn batches_respect_the_char_budget() {
        let file = |n: &str, len: usize| (PathBuf::from(n), String::new(), "x".repeat(len));
        let files = vec![
            file("a", 7_000),
            file("b", 7_000),
            file("c", 100),
            file("d", 20_000),
        ];
        let sizes: Vec<usize> = batches(&files).iter().map(|b| b.len()).collect();
        assert_eq!(sizes, vec![1, 2, 1]);
    }

    #[derive(Deserialize)]
    struct Answer {
        files: Vec<String>,
    }

    /// Answers `{"files":[...]}` with the files of the prompt, but refuses
    /// (not JSON) a prompt holding several files when `picky`.
    fn echo(picky: bool) -> FakeLlm {
        FakeLlm::replying(move |_, request| {
            let files: Vec<&str> = request.messages[1]
                .content
                .lines()
                .filter_map(|l| l.strip_prefix("--- ")?.strip_suffix(" ---"))
                .collect();
            Ok(if picky && files.len() > 1 {
                "too long, cut".to_string()
            } else {
                format!("{{\"files\":{}}}", serde_json::to_string(&files).unwrap())
            })
        })
    }

    #[derive(Default)]
    struct State {
        done: Vec<(String, usize)>,
    }

    fn spec() -> BatchedRead<'static, Answer, String, State> {
        BatchedRead {
            pass: "test",
            unit: "file(s)",
            system_prompt: "system",
            header: "Files:\n",
            attribute: |answer, _| {
                let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
                for file in answer.files {
                    found.entry(file.clone()).or_default().push(file);
                }
                found
            },
            finish: |state, path, _, items| {
                state
                    .done
                    .push((path.to_string_lossy().into_owned(), items.len()));
            },
            checkpoint: &|_| Ok(()),
        }
    }

    fn chunk(path: &str, text: &str) -> PendingChunk {
        (PathBuf::from(path), "hash".to_string(), text.to_string())
    }

    #[tokio::test]
    async fn a_file_is_finished_once_all_its_chunks_are_read() {
        let llm = echo(false);
        let pending = vec![chunk("a", "one"), chunk("a", "two"), chunk("b", "three")];
        let mut state = State::default();

        spec().run(&llm, &pending, &mut state).await.unwrap();

        // One batch, one call; each chunk of `a` yields one item.
        assert_eq!(llm.calls(), 1);
        assert_eq!(state.done.len(), 2);
        assert!(state.done.contains(&("b".to_string(), 1)));
        assert!(state.done.contains(&("a".to_string(), 2)));
    }

    #[tokio::test]
    async fn an_unusable_batch_is_retried_file_by_file() {
        let llm = echo(true);
        let pending = vec![chunk("a", "one"), chunk("b", "two")];
        let mut state = State::default();

        spec().run(&llm, &pending, &mut state).await.unwrap();

        // The batch is tried twice (`complete_json` retries once), then each file once.
        assert_eq!(llm.calls(), 4);
        assert_eq!(state.done.len(), 2);
    }

    #[tokio::test]
    async fn the_state_is_checkpointed_after_every_batch() {
        let llm = echo(false);
        let big = "x".repeat(BATCH_CHARS);
        let pending = vec![chunk("a", &big), chunk("b", &big)];
        let mut state = State::default();
        let mut read = spec();
        read.checkpoint = &|_| {
            Err(PipelineError::Read {
                path: PathBuf::from("stop"),
                source: std::io::Error::other("stop"),
            })
        };

        let result = read.run(&llm, &pending, &mut state).await;

        // The first checkpoint fails: the second batch is never read.
        assert!(result.is_err());
        assert_eq!(llm.calls(), 1);
    }
}
