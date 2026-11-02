---
id: DLG
status: accepted
adrs: [0023, 0040, 0041]
---

# Language of the generated documentation

## Intent

A team that reads another language than English sets it once in `retrodoc.toml` and gets the generated docs written
in it, without losing the incremental re-run nor the links between the passes. Without the setting the docs and the
saved results stay those of today.

## Requirements

- **DLG-1** (deterministic) A `retrodoc.toml` with no `language` key in `[output]` must read no language, and every
  pass must send the prompts and compare the prints it sent and compared before the key existed.
- **DLG-2** (deterministic) `[output] language` must take a language name, sent as written (`"French"`), or a
  two-letter tag of a common language (`fr`, `de`, `es`, `it`, `pt`, `nl`, `en`), sent as its English name; an empty
  value must fail to load with an error that names the key.
- **DLG-3** (deterministic) With a language set, the system prompt of every writing pass (`brief`, `contexts`,
  `glossary`, `entry-points`, `actors`, `domains`, `features`, `use-cases`, `rules`, `confidence`, `reconcile`) must
  end with one instruction that names the language, asks for every free text of the answer in it, and asks to keep
  as given the code identifiers, the file paths and the names the prompt gives; the reading passes (`sources`,
  `roles`, `business-files`) and the benchmark judge must not receive it.
- **DLG-4** (deterministic) Changing the language, or setting or removing it, must ask every writing pass again
  once, except the map of the contexts, which stays reused until `contexts --force` as today; a second `generate`
  with the same language must call no LLM.
- **DLG-5** (deterministic) The slug of a name with accented Latin letters must keep their base letter
  (`Préférences des clients` → `preferences-des-clients`), and the slug of an ASCII name must
  not change.
- **DLG-6** (deterministic) `init` must write the `language` key commented out in `[output]`, with an example.
- **DLG-7** (deterministic) With `[output] language` naming French (found as DLG-9 says), the fixed text of
  the Markdown docs `generate` and `render` write (headings, labels, notes of the sections, the index for agents, the
  rules index) must be the French one; with no language or any other language, the English one of today.
- **DLG-8** (deterministic) The paths, slugs and anchors of the docs must not depend on the language, and a second
  `render` with the same artifacts and language must write no file.
- **DLG-9** (deterministic) The two-letter tag of `[output] language` must be found from its tag, its English name or
  its native name (`Français`, `Deutsch`, `Español`, `Italiano`, `Português`, `Nederlands`, `English`), ignoring case
  and accents, or from a regional tag (the two letters, a `-` or `_`, then a region of two letters or three digits: `fr-FR`, `fr_FR`,
  `pt-BR`, `es-419`) by its first two letters; the name the prompts are
  given stays the one DLG-2 says, so these spellings ask no pass again.

## Scenarios

- (DLG-1) Given a run made before the key existed, when `generate` runs again with the same config, then no pass is
  asked again.
- (DLG-2) Given `language = "fr"`, when a writing pass is asked, then its instruction names French.
- (DLG-3) Given `language = "French"`, when `generate` runs, then the narratives, steps and glossary definitions are
  in French while the entity names keep the spelling of the code.
- (DLG-4) Given a finished run in English, when the user sets `language = "French"` and runs `generate`, then the
  writing passes are asked again and the reading passes reuse their results; a third run calls nothing.
- (DLG-7) Given `language = "fr"`, when the docs are written, then a use case page with a narrative reads
  `## Ce qui se passe` and `**Confiance :**` instead of `## What happens` and `**Confidence:**`.
- (DLG-7) Given `language = "German"`, when the docs are written, then the fixed text stays English.
- (DLG-7, DLG-9) Given `language = "Français"`, `"francais"` or `"fr_FR"`, when the docs are written, then their fixed
  text is the French one.
- (DLG-9) Given `language = "pt-BR"`, then the tag is `pt` and the prompts name `pt-BR`.
- (DLG-9) Given `language = "zh-TW"` or `"Nihongo"`, then there is no tag.
- (DLG-9) Given `language = "en-dash"`, then there is no tag: `dash` is not a region.
- (DLG-5) Given a domain named `Suivi des livraisons` then one named `Préférences des clients`, when the
  docs are written, then their folders are `suivi-des-livraisons` and `preferences-des-clients`.

## Non-goals

- Translating docs already generated without asking the passes again: a language change redoes the writing passes.
- Fixed text in a language other than English and French: a language with no table keeps the English one.
- The coverage report (`_retrodoc/coverage-report.md`), the output of the CLI and the text of the MCP tools stay in
  English.
- Scripts other than Latin in slugs (a name in Cyrillic or CJK falls back to what `slugify` does today).
- Detecting the language of the repository to pick one by default.
- Keeping the region of a regional tag in the `lang` of the web pages (WEB-78): the fixed text has no regional
  variant.
- Native names written in another script than Latin, and names of more languages than those of `LANGUAGE_TAGS`.
