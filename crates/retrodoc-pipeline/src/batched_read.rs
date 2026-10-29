//! The loop shared by the passes that read many small files per LLM call
//! (glossary, entry points, and the joint reading of both), and what they
//! share around it: the files to read again, the attribution of an answer
//! to the cited file, the saved files a partial run keeps.
//!
//! The changed files, already cut in chunks, are grouped in batches of about
//! [`BATCH_CHARS`] of code, never two chunks of one file in the same batch: a
//! model asked several parts of a file at once can answer for the first one
//! only (`specs/batched-reading.md`). A batch whose answer is unusable is
//! retried file by file. A file is handed back only once all its chunks are
//! answered, so an unusable answer leaves it to be retried whole next run
//! (its file named in the log), and the state is checkpointed after every
//! batch: a failure later in a long run (a call timing out) must not lose the
//! batches already read. A part that gives nothing while another part of its
//! file gives something is logged, as a warning for the files the pass says
//! are `loud`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_llm::LlmProvider;
use serde::de::DeserializeOwned;

use crate::brief::ProductBrief;
use crate::chunks::strip_part_marker;
use crate::citations::resolve_cited_path;
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, hash_for_model, prompt_part};
use crate::progress::Progress;
use crate::response::complete_json;
use crate::text::read_file_lossy;

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
    /// Whether a part of the file that found nothing beside parts that found something is a
    /// warning (else an info line: a part of a model often holds no entity and no public method).
    pub loud: &'a dyn Fn(&Path) -> bool,
    /// Saves the state after a batch.
    pub checkpoint: &'a dyn Fn(&S) -> Result<(), PipelineError>,
}

impl<R: DeserializeOwned + schemars::JsonSchema, T, S> BatchedRead<'_, R, T, S> {
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
        // answered ones with how much each found, in the order of its parts.
        let mut remaining: BTreeMap<&Path, usize> = BTreeMap::new();
        for (path, _, _) in pending {
            *remaining.entry(path.as_path()).or_insert(0) += 1;
        }
        let mut partial: BTreeMap<&Path, (Vec<T>, Vec<usize>)> = BTreeMap::new();

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
                    let _ = writeln!(
                        prompt,
                        "{FILE_OPENS}{}{FILE_CLOSES}{content}",
                        path.display()
                    );
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
                    } else {
                        // `complete_json` warned of the answer: this names its file.
                        tracing::info!(
                            pass = self.pass,
                            file = %group[0].0.display(),
                            "unusable answer for a file, left unread by this pass"
                        );
                    }
                    continue;
                };

                let mut found = (self.attribute)(response, group);
                for (path, hash, _) in group {
                    let key = path.to_string_lossy();
                    let items = found.remove(key.as_ref()).unwrap_or_default();
                    let (all, per_part) = partial.entry(path).or_default();
                    per_part.push(items.len());
                    all.extend(items);
                    let left = remaining.entry(path).or_insert(1);
                    *left -= 1;
                    if *left == 0 {
                        let (items, per_part) = partial.remove(path.as_path()).unwrap_or_default();
                        self.log_empty_parts(path, &per_part);
                        (self.finish)(state, path, hash, items);
                    }
                }
            }
            (self.checkpoint)(state)?;
        }
        Ok(())
    }

    /// Logs the parts of `path` that found nothing while another part found something
    /// (`parts`: what each part found, in order).
    fn log_empty_parts(&self, path: &Path, parts: &[usize]) {
        let empty: Vec<String> = parts
            .iter()
            .enumerate()
            .filter(|&(_, &n)| n == 0)
            .map(|(i, _)| (i + 1).to_string())
            .collect();
        if empty.is_empty() || empty.len() == parts.len() {
            return;
        }
        let (pass, file, parts, of) = (self.pass, path.display(), empty.join(", "), parts.len());
        let message = "parts of a file found nothing while the others did";
        if (self.loud)(path) {
            tracing::warn!(pass, %file, parts, of, "{message}");
        } else {
            tracing::info!(pass, %file, parts, of, "{message}");
        }
    }
}

/// The fixed instructions of the three reading passes (glossary, entry points and their joint reading),
/// for the hash of a file: one for all three, since the joint reading saves what the two others reuse
/// (ADR 0028), so a change of any of them reads the files again (ADR 0037).
pub(crate) fn reading_instructions() -> String {
    prompt_part(&[
        crate::glossary::GLOSSARY_SYSTEM_PROMPT,
        crate::glossary::GLOSSARY_HEADER,
        crate::glossary::BUSINESS_SYSTEM_PROMPT,
        crate::glossary::BUSINESS_HEADER,
        crate::entry_points::ENTRY_POINTS_SYSTEM_PROMPT,
        crate::entry_points::ENTRY_POINTS_HEADER,
        crate::business_read::BUSINESS_READ_SYSTEM_PROMPT,
        crate::business_read::BUSINESS_READ_HEADER,
        FILE_OPENS,
        FILE_CLOSES,
    ])
}

/// What frames the path of a file in the prompt of a batch.
const FILE_OPENS: &str = "\n--- ";
const FILE_CLOSES: &str = " ---\n";

/// The chunks of the `paths` to read again. The hash of a file is that of its content with the
/// `instructions` of the pass and the brief, for the model of `llm`; `current(path, hash)` says whether the saved reading is up to date (and
/// keeps it), else the file is cut by `cut(path, content)`.
///
/// # Errors
///
/// Returns an error if a file can't be read.
pub(crate) fn pending_chunks<'p>(
    repo_root: &Path,
    paths: impl IntoIterator<Item = &'p Path>,
    instructions: &str,
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
    cut: impl Fn(&Path, &str) -> Vec<String>,
    mut current: impl FnMut(&Path, &str) -> bool,
) -> Result<Vec<PendingChunk>, PipelineError> {
    let mut pending = Vec::new();
    for path in paths {
        let content = read_file_lossy(repo_root, path)?;
        let hash = hash_for_model(
            &fingerprint([instructions, &brief.hash_with(&content)]),
            llm,
        );
        if current(path, &hash) {
            continue;
        }
        for chunk in cut(path, &content) {
            pending.push((path.to_path_buf(), hash.clone(), chunk));
        }
    }
    Ok(pending)
}

/// What an answer found, by the file of `batch` it cites. A batch of a single file needs no
/// citation; otherwise the cited path must resolve to a file of the batch (else the item is dropped,
/// with a warning naming `what` it is). `split` gives the cited file of an item and what it found,
/// `name` the name of that, and a nameless one is ignored.
pub(crate) fn attribute<I, T>(
    items: Vec<I>,
    batch: &[PendingChunk],
    what: &str,
    split: impl Fn(I) -> (String, T),
    name: impl Fn(&T) -> &str,
) -> BTreeMap<String, Vec<T>> {
    let allowed = group_paths(batch);
    let mut found: BTreeMap<String, Vec<T>> = BTreeMap::new();
    for item in items {
        let (file, value) = split(item);
        let target = if allowed.len() == 1 {
            allowed.iter().next().cloned()
        } else {
            resolve_cited_path(strip_part_marker(&file), &allowed)
        };
        match target {
            Some(target) if !name(&value).trim().is_empty() => {
                found.entry(target).or_default().push(value);
            }
            Some(_) => {}
            None => tracing::warn!(
                file = %file,
                name = %name(&value),
                "{what} attributed to an unknown file, dropped"
            ),
        }
    }
    found
}

/// The `saved` readings of the files not `read` this time that still exist, which a run reading a
/// part of the application keeps (`Retention::Others`).
pub(crate) fn unread_kept<V: Clone>(
    repo_root: &Path,
    saved: &BTreeMap<PathBuf, V>,
    read: &BTreeSet<&Path>,
) -> Vec<(PathBuf, V)> {
    saved
        .iter()
        .filter(|(path, _)| !read.contains(path.as_path()) && repo_root.join(path).is_file())
        .map(|(path, value)| (path.clone(), value.clone()))
        .collect()
}

/// The files whose attribution key (`path` as a string) appears in `group`.
pub(crate) fn group_paths(group: &[PendingChunk]) -> BTreeSet<String> {
    group
        .iter()
        .map(|(path, _, _)| path.to_string_lossy().into_owned())
        .collect()
}

/// Groups files so that each batch holds about [`BATCH_CHARS`] of code
/// (at least one file) and at most one chunk of a file.
fn batches(files: &[PendingChunk]) -> Vec<&[PendingChunk]> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut size = 0;
    let mut paths: BTreeSet<&Path> = BTreeSet::new();
    for (i, (path, _, content)) in files.iter().enumerate() {
        let len = content.chars().count();
        if i > start && (size + len > BATCH_CHARS || paths.contains(path.as_path())) {
            out.push(&files[start..i]);
            start = i;
            size = 0;
            paths.clear();
        }
        size += len;
        paths.insert(path);
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

    // spec: BRD-1 (internal: a long file needs more than 12k characters of code, the batches are built inside the pass)
    #[test]
    fn two_parts_of_a_file_never_share_a_batch() {
        let part = |n: &str| (PathBuf::from(n), String::new(), "x".repeat(4_000));
        let paths = |files: &[PendingChunk]| -> Vec<Vec<String>> {
            batches(files)
                .iter()
                .map(|b| b.iter().map(|(p, _, _)| p.display().to_string()).collect())
                .collect()
        };
        let files = vec![part("a"), part("b"), part("b"), part("b"), part("c")];
        assert_eq!(
            paths(&files),
            vec![vec!["a", "b"], vec!["b"], vec!["b", "c"]]
        );
        // Parts that are not next to each other do not share a batch either.
        let files = vec![part("a"), part("b"), part("a")];
        assert_eq!(paths(&files), vec![vec!["a", "b"], vec!["a"]]);
    }

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

    #[derive(Deserialize, schemars::JsonSchema)]
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
            loud: &|_| true,
            checkpoint: &|_| Ok(()),
        }
    }

    fn chunk(path: &str, text: &str) -> PendingChunk {
        (PathBuf::from(path), "hash".to_string(), text.to_string())
    }

    // spec: RER-10 (internal: the instructions are constants of the binary, a test can't change them)
    #[test]
    fn a_file_saved_under_other_instructions_is_read_again() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rb"), "class A; end").unwrap();
        let llm = FakeLlm::answering("");
        let saved = std::cell::RefCell::new(String::new());
        let pending = |instructions: &str| {
            pending_chunks(
                dir.path(),
                [Path::new("a.rb")],
                instructions,
                &ProductBrief::default(),
                &llm,
                |_, content| vec![content.to_string()],
                |_, hash| {
                    let same = *saved.borrow() == hash;
                    *saved.borrow_mut() = hash.to_string();
                    same
                },
            )
            .unwrap()
            .len()
        };

        assert_eq!(
            [pending("v1"), pending("v1"), pending("v2"), pending("v2")],
            [1, 0, 1, 0]
        );
    }

    // spec: BRD-1, BRD-2 (internal: the fake LLM answers per prompt, the binary needs a real provider)
    #[tokio::test]
    async fn a_file_is_finished_once_all_its_chunks_are_read() {
        let llm = echo(false);
        let pending = vec![chunk("a", "one"), chunk("a", "two"), chunk("b", "three")];
        let mut state = State::default();

        spec().run(&llm, &pending, &mut state).await.unwrap();

        // The two parts of `a` are asked in two calls, `b` with the second; each yields one item.
        assert_eq!(llm.calls(), 2);
        assert_eq!(state.done.len(), 2);
        assert!(state.done.contains(&("b".to_string(), 1)));
        assert!(state.done.contains(&("a".to_string(), 2)));
    }

    /// Answers one item per file of the prompt, except for a file whose text says `empty`.
    fn per_part() -> FakeLlm {
        FakeLlm::replying(|_, request| {
            let mut files = Vec::new();
            let mut lines = request.messages[1].content.lines().peekable();
            while let Some(line) = lines.next() {
                if let Some(path) = line
                    .strip_prefix("--- ")
                    .and_then(|l| l.strip_suffix(" ---"))
                {
                    if lines.peek().is_some_and(|text| !text.contains("empty")) {
                        files.push(path.to_string());
                    }
                }
            }
            Ok(format!(
                "{{\"files\":{}}}",
                serde_json::to_string(&files).unwrap()
            ))
        })
    }

    // spec: BRD-3 (internal: the fake LLM answers per part, the binary needs a real provider)
    #[tokio::test]
    async fn a_part_with_nothing_beside_parts_with_something_is_warned_of() {
        let (logs, _guard) = crate::testing::capture_logs();
        let pending = vec![
            chunk("routes.rb", "(part 1/3) resources"),
            chunk("routes.rb", "(part 2/3) empty"),
            chunk("routes.rb", "(part 3/3) empty"),
            chunk("empty.rb", "(part 1/2) empty"),
            chunk("empty.rb", "(part 2/2) empty"),
        ];
        let mut state = State::default();

        spec().run(&per_part(), &pending, &mut state).await.unwrap();

        let logs = logs.text();
        let warnings: Vec<&str> = logs.lines().filter(|l| l.contains("WARN")).collect();
        assert_eq!(warnings.len(), 1, "{logs}");
        assert!(warnings[0].contains("pass=\"test\""), "{logs}");
        assert!(warnings[0].contains("file=routes.rb"), "{logs}");
        assert!(warnings[0].contains("parts=\"2, 3\""), "{logs}");
        assert!(state.done.contains(&("routes.rb".to_string(), 1)));
    }

    // spec: BRD-5 (internal: the fake LLM answers per part, the binary needs a real provider)
    #[tokio::test]
    async fn the_empty_parts_of_a_quiet_file_are_logged_at_info() {
        let (logs, _guard) = crate::testing::capture_logs();
        let pending = vec![
            chunk("comment.rb", "(part 1/2) empty"),
            chunk("comment.rb", "(part 2/2) def public"),
        ];
        let mut state = State::default();
        let mut read = spec();
        read.loud = &|_| false;

        read.run(&per_part(), &pending, &mut state).await.unwrap();

        let logs = logs.text();
        assert!(!logs.contains("WARN"), "{logs}");
        let empty: Vec<&str> = logs
            .lines()
            .filter(|l| l.contains("found nothing"))
            .collect();
        assert_eq!(empty.len(), 1, "{logs}");
        assert!(
            empty[0].contains("INFO") && empty[0].contains("parts=\"1\""),
            "{logs}"
        );
    }

    // spec: BRD-4 (internal: the fake LLM refuses per prompt, the binary needs a real provider)
    #[tokio::test]
    async fn an_unusable_answer_for_a_single_file_names_it() {
        let (logs, _guard) = crate::testing::capture_logs();
        let llm = FakeLlm::replying(|_, request| {
            Ok(if request.messages[1].content.contains("--- b ---") {
                "no".to_string()
            } else {
                r#"{"files":["a"]}"#.to_string()
            })
        });
        let pending = vec![chunk("a", "one"), chunk("b", "two")];
        let mut state = State::default();

        spec().run(&llm, &pending, &mut state).await.unwrap();

        let logs = logs.text();
        let skipped: Vec<&str> = logs.lines().filter(|l| l.contains("left unread")).collect();
        assert_eq!(skipped.len(), 1, "{logs}");
        assert!(
            skipped[0].contains("pass=\"test\"") && skipped[0].contains("file=b"),
            "{logs}"
        );
        // `complete_json` warned of the answer: naming the file adds no warning.
        assert!(skipped[0].contains("INFO"), "{logs}");
        assert_eq!(state.done, vec![("a".to_string(), 1)]);
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
