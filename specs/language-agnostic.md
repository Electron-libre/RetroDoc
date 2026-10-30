---
id: LNG
status: draft
adrs: [0008, 0013, 0022, 0027, 0036]
---

# Language and framework agnosticism

## Intent

RetroDoc documents a repository whatever its language and framework: what it knows of a stack (an extension list,
a manifest reader, a schema reader, a grammar) only makes a pass cheaper or sharper, and the pipeline works without
it. The most frequent stacks, the reference set below, are each proven to go through the whole pipeline.

Reference set, the stacks this spec names, taken from the most used languages and web frameworks of the Stack
Overflow Developer Survey and reviewed once a year (ADR 0036):

| Language | Extensions | Frameworks |
|---|---|---|
| JavaScript | `js`, `jsx`, `mjs`, `cjs` | Express |
| TypeScript | `ts`, `tsx`, `mts`, `cts` | NestJS, Next.js |
| Python | `py` | Django, Flask, FastAPI |
| Java | `java` | Spring Boot |
| Kotlin | `kt`, `kts` | Spring Boot, Ktor |
| C# | `cs` | ASP.NET Core |
| PHP | `php` | Laravel, Symfony |
| Go | `go` | `net/http`, Gin |
| Ruby | `rb` | Rails |
| Rust | `rs` | Axum, Actix |
| C, C++ | `c`, `h`, `cpp`, `cc`, `hpp` | none |
| Swift | `swift` | Vapor |
| Scala | `scala` | Play |

## Requirements

- **LNG-1** (deterministic) `generate` must finish with exit code 0 and write the functional docs on a repository
  whose language no built-in table knows (no known extension, no known manifest, no known schema format, no
  grammar), the roles call naming its extension in `source_extensions`.
- **LNG-2** (deterministic) A file whose extension the roles call names in `source_extensions` must be read by the
  extraction passes as a source file, like a file of a built-in extension.
- **LNG-3** (deterministic) A long file of a language this build cannot parse must be cut at the lines the
  boundary regex of `roles.yaml` matches, else at blank lines.
- **LNG-4** (deterministic) The instructions of each prompt must be the same text whatever the language and the
  framework of the repository: only the content read from the repository differs.
- **LNG-5** (deterministic) The walker must classify as source, with no LLM call, a file of each extension of the
  reference set.
- **LNG-6** (deterministic) The walker must classify as test a file of the test layout of each reference language
  (`src/test/` and `*Test.java`, `*Tests.cs`, `*_test.go`, `*Test.php`, `test_*.py`, `*.spec.ts`, `*_spec.rb`).
- **LNG-7** (deterministic) The product brief must read the manifest of each reference language (`package.json`,
  `pyproject.toml`, `requirements.txt`, `pom.xml`, `build.gradle`, `build.gradle.kts`, `*.csproj`,
  `composer.json`, `go.mod`, `Gemfile`, `Cargo.toml`, `Package.swift`, `build.sbt`) at the root of the repository.
- **LNG-8** (deterministic) The structure must give the definitions of a file of each language of the reference set.
- **LNG-9** (deterministic) `generate` with a fake LLM must finish with exit code 0 and no warning on a minimal
  application of each framework of the reference set (one entry point, one model, one test).
- **LNG-10** (deterministic) A pass with a reader of one stack must still give its output, from the generic path,
  on a repository with no file that reader knows (the reader of `schema.rb`, of a manifest, a grammar).

## Scenarios

- (LNG-1) Given an Elixir repository (`mix.exs`, `lib/**/*.ex`) and a fake LLM whose roles answer names `ex` in
  `source_extensions`, when `generate` runs, then it exits 0 and the entry points pass reads the `.ex` files.
- (LNG-2) Given `source_extensions: [".EX"]`, then `lib/shop.ex` is read as source (dot and case ignored).
- (LNG-2) Given `source_extensions` naming `ex` and a file `test/shop_test.ex`, then it is a test file, not a
  source file.
- (LNG-3) Given a 20 KB `.ex` file (under the cap of 8 parts) and a boundary regex that matches no line, then it is cut at blank lines.
- (LNG-4) Given a Django fixture and a Spring Boot fixture, when the prompts of `generate` are captured, then their
  instruction parts are equal and only the file lists, extracts and paths differ.
- (LNG-5) Given `build.gradle.kts`, `settings.gradle.kts` and `Package.swift`, then they are not source files, even
  when the roles call names `kts` or `swift`: a build script is read as a manifest (LNG-7), never as code of the
  application; `src/main/kotlin/Order.kts` is a source file.
- (LNG-6) Given `src/main/java/shop/OrderTest.java` outside any test directory, then it is a test file; given
  `src/main/java/shop/Contest.java`, then it is a source file (the suffix is `Test` with a capital, after a word);
  given `src/components/SpeedTest.tsx`, then it is a source file (the capital suffix is the convention of Java,
  Kotlin, Groovy, C#, PHP, Scala and Swift only).
- (LNG-7) Given a `.csproj` file at the root under any name (`Shop.Api.csproj`), then it is read.
- (LNG-7) Given a manifest that does not parse, then it is skipped and the brief is still written.
- (LNG-10) Given a Prisma schema (`prisma/schema.prisma`) and no `schema.rb` nor SQL DDL, then the glossary is
  still given, from the business files.

## Non-goals

- The same quality of docs on every stack: it depends on what a model writes and is measured by the quality
  benchmark, not required here.
- An adapter per framework (a route parser, an ORM parser): rejected by `PLAN.md` §7.1, a framework is recognised by
  the LLM from the tree and the manifests.
- Reading view templates (`.erb`, `.blade.php`, `.cshtml`) as source.
- A language outside the reference set beyond the generic path of LNG-1 to LNG-3.
- Generated, vendored or minified code.
