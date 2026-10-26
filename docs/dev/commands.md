# Commands and smoke tests

```sh
cargo build --workspace                 # build everything
cargo test --workspace                  # run all tests
cargo test -p retrodoc-pipeline         # run one crate's tests
cargo test -p retrodoc-pipeline repo_map::tests::builds_bottom_up  # run a single test
cargo clippy --workspace --all-targets  # lint (see Lints below — must be warning-free)
cargo fmt --all                         # format
just check                              # fmt + clippy -D warnings + tests + check-specs + check-harness in one go
just check-harness                      # fail when a file synced from the shared harness was edited locally (needs the `harness` binary: `cargo install --git <source in .claude/harness.toml> harness --locked`)
just check-specs                        # validate specs/*.md and trace their requirements to the tests (`// spec: ID`; needs rust-script)
just test-harness                       # test the agent hooks and skills (needs rust-script)
just probe-schema <ollama|hybrid|deepseek|gemini|openrouter>   # one HTTP call: does the server reject, ignore or obey a strict JSON schema; also prints finish_reason and usage (`--max-tokens 8` forces a cut)
cargo run -q -p retrodoc-pipeline --example entry_points_replay -- benchmark/<repo>/entry-points.yaml <checkout> <runs dir>   # saved runs vs a hand-made entry points/actors reference (no LLM)
cargo run -q -p retrodoc-pipeline --example freeze_inputs -- <run dir> ~/retrodoc-runs/frozen/<repo> <repo> <commit> <model>   # copies the artifacts of a complete run as the frozen inputs of the replays of one pass (manifest with the model that wrote them) (no LLM)
DEEPSEEK_API_KEY=x cargo run -q -p retrodoc-pipeline --example pass_replay -- <domains|features|use-cases|rules> ~/retrodoc-runs/frozen/<repo> benchmark/<repo> <model> [runs] [--freeze-first]   # one pass alone on frozen inputs, N times: items, recall and precision (strict, then with the judge's pairs), calls, tokens, seconds; `--freeze-first` (domains, features, use-cases) keeps the result of run 1 as the frozen one for the next replay; `use-cases` and `rules` read the code of the tree named by `manifest.source_run`; `use-cases` measures the use cases (number, narratives, business language by the judge and by the deterministic score), `rules` the rules stated and how many of the `rules` of `reference.yaml` a generated rule states by the judge; `RETRODOC_KEEP_RULES=<dir>` keeps the rules of each `rules` run as `rules-<n>.yaml`; about 2k prompt tokens a run for the domains 10k for the features, 275k for the use cases and 280k for the rules of linkding
cargo run -q -p retrodoc-pipeline --example behaviour_baseline -- <run dir>...   # share of source files a behaviour reaches and calls/minutes per pass of saved runs (no LLM)
cargo run -q -p retrodoc-pipeline --example context_map_measure -- [--names] <run dir>...   # can bounded contexts be built from the file names: contexts, coverage, overlap of saved runs (no LLM)
cargo run -q -p retrodoc-pipeline --example route_owner_measure -- <run dir>...   # routes given to the context that owns their controller: moved, kept, entry points per context before/after (no LLM)
cargo run -q -p retrodoc-pipeline --example technical_usecases_measure -- <run dir>...   # use cases per technical or back-office signal, and which signal finds the administration ones (no LLM)
cargo run -q -p retrodoc-pipeline --example public_api_reach_measure -- <run dir>...   # public API entry points no route/job/webhook reaches, left out of an application (no LLM)
cargo run -q -p retrodoc-pipeline --example form_pairs_measure -- [--names] <run dir>...   # use cases that only show a resource another use case changes (candidates to merge with the one that submits) (no LLM)
cargo run -q -p retrodoc-pipeline --example rules_noise_measure -- [--sample N] [--seed S] <run dir>   # duplicate pairs of rules across features and a seeded sample to label by hand (no LLM)
cargo run -q -p retrodoc-pipeline --example code_read_overlap -- <frozen dir> <benchmark dir>   # code shared by the use cases and rules prompts of each feature, and the share of it the use case steps cite, from frozen inputs (no LLM)
cargo run -q -p retrodoc-pipeline --example rules_answer_size_measure -- <run dir>...   # characters of the rules answer per feature rebuilt from saved rules, share of the completion cap (an estimate: cut answers are never saved) (no LLM)
cargo run -q -p retrodoc-pipeline --example features_by_domain_measure -- <run dir>...   # entry points, features and share held by a feature per domain of saved runs (no LLM)
OPENROUTER_API_KEY=x cargo run -q -p retrodoc-pipeline --example placement_replay -- <model> <base url> <run dir> benchmark/<repo>/placement-sample.yaml   # placement of a labelled sample of entry points: JSON answer vs closed-answer readout (logprobs of one token) on a local OpenAI-compatible server: placed, acceptable, steadiness, time and tokens
```

Scripts and dev commands are `just` recipes (`justfile`) and `rust-script` files, not bash.

Registry note: this environment has no direct network access to `crates.io` (plain HTTP is blocked), but
`cargo build`/`cargo test`/`cargo clippy` work fine without `--offline` — cargo reaches the registry through
a path that works even though `curl https://crates.io` doesn't. Don't pass `--offline`; it fails on any
dependency not already in `~/.cargo/registry/cache`.

Manual CLI smoke test (no real OpenRouter call needed to check the error path):
```sh
cargo run -p retrodoc-cli -- init --path <target-repo>
cargo run -p retrodoc-cli -- scan --path <target-repo>
OPENROUTER_API_KEY=... cargo run -p retrodoc-cli -- generate --path <target-repo>
```

API keys may live in a git-ignored `.env` (read by the CLI at startup, `dotenvy`; variables already set win).
With `provider = "deepseek"` in `retrodoc.toml` the key is `DEEPSEEK_API_KEY`.
