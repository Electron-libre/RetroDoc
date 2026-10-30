---
id: LNG
status: accepted
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
- **LNG-11** (deterministic) The product brief must read the libraries of the Gradle version catalog
  `gradle/libs.versions.toml` as the dependencies of a manifest.
- **LNG-12** (deterministic) The product brief must read the `*.csproj` files the root `.sln` or `.slnx` lists, else
  those one or two directory levels below the root, test projects left out.
- **LNG-13** (deterministic) The walker must classify as test a file under a .NET test project directory (a name
  ending in `Tests`: `Shop.Tests/`, `Shop.UnitTests/`) or a Gradle test source set (`src/integrationTest/kotlin/`,
  `src/testFixtures/java/`).
- **LNG-14** (deterministic) The walker must not classify as source a file under the root `project/` directory of
  an sbt build (a `build.sbt` at the root).
- **LNG-15** (deterministic) `scan` and `roles` must count as source a file whose extension the saved `roles.yaml`
  names in `source_extensions`, like the passes of `generate` read it.
- **LNG-16** (deterministic) When files remain that no rule of `roles.yaml` matches, the roles pass must make one
  LLM call over their paths alone (no content) and give them the roles of its answer.
- **LNG-17** (deterministic) The call of LNG-16 must leave `roles.yaml` as it was and the role of every file a rule
  of `roles.yaml` matches.
- **LNG-18** (deterministic) The roles pass must not send again the path of a file the call of LNG-16 was given.
- **LNG-19** (deterministic) The call of LNG-16 must send at most 400 paths, and the roles pass must warn of the
  count of files it leaves unclassified.

- **LNG-20** (deterministic) The promotion of LNG-2 and LNG-15 must leave out a file whose extension is one of
  templates, markup, stylesheets or data (`erb`, `haml`, `slim`, `jbuilder`, `html`, `css`, `scss`, `json`,
  `yaml`... the full list in `walker.rs`), whatever `source_extensions` names, and `scan`, `roles` and `generate` must print
  the extensions they left out.

## Scenarios

- (LNG-1) Given an Elixir repository (`mix.exs`, `lib/**/*.ex`) and a fake LLM whose roles answer names `ex` in
  `source_extensions`, when `generate` runs, then it exits 0 and the entry points are read from the `.ex` files.
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
- (LNG-11) Given a catalog whose `[libraries]` give a `module`, a `group` and `name`, and a
  `"group:artifact:version"` string, then each gives its `group:artifact`; given a `build.gradle.kts` that names its
  dependencies only through the catalog (`implementation(libs.spring.web)`), then the brief still has dependencies.
- (LNG-12) Given a root `Shop.sln` listing `src\Shop.Api\Shop.Api.csproj` and `tests\Shop.Tests\Shop.Tests.csproj`,
  then `src/Shop.Api/Shop.Api.csproj` is read and the test project is not; given no `.sln`, then
  `src/Shop.Api/Shop.Api.csproj` is read, and a `.csproj` three levels down is not; a path of the `.sln` outside
  the repository, or a project the walk ignores (`.gitignore`, `extra_ignore`), is not read; a path of the `.sln` in
  another case (`src\shop.api\Shop.Api.csproj`) is read; a `.sln` listing no `.csproj` falls back to the projects
  one or two levels down; a root `Shop.Tests.csproj` is not read.
- (LNG-13) Given `Shop.Tests/Fakes/FakeClock.cs`, `src/integrationTest/kotlin/OrderIT.kt` and
  `src/testFixtures/java/OrderBuilder.java`, then they are test files; given
  `src/main/java/shop/Contests/Draw.java`, then it is a source file (`Tests` with a capital, after a word;
  `Contests` has none); given `src/speedTest/Runner.ts`, then it is a source file (a source set holds a JVM language
  directory).
- (LNG-14) Given `build.sbt` and `project/Dependencies.scala`, then the latter is not a source file, even when the
  roles call names `scala`; given `project/Dependencies.scala` with no `build.sbt`, then it is a source file.
- (LNG-8) Given a file of each language of the reference set with a type, a method spanning several lines and a
  call in it, then the structure gives the type and the method with their first and last lines, and the call.
- (LNG-8) Given a Kotlin file and a Java file, then they are one language for the slices (a name one defines is one
  the other can call); so are Scala and Java, C and C++.
- (LNG-10) Given a Prisma schema (`prisma/schema.prisma`) and no `schema.rb` nor SQL DDL, then the glossary is
  still given, from the business files.
- (LNG-15) Given a saved `roles.yaml` naming `ex` and `lib/shop/order.ex`, then `scan` counts it as source and
  says one file was promoted; with no `roles.yaml`, it is counted as other.
- (LNG-16) Given rules that match no `.coffee` file and an answer giving `app/assets/**/*.coffee` the role `view`,
  then those files are `view` and the added rules are saved in `roles-unmatched.yaml`, hand-editable.
- (LNG-16) Given rules that match no file (an empty `rules` list), then no call is made: the rules are unusable.
- (LNG-16) Given a call that fails, then the pass goes on with the rules of `roles.yaml`, with a warning.
- (LNG-17) Given a hand-written `roles.yaml` with a comment and an answer whose rule `app/**` would also match
  `app/models/order.rb`, matched by `app/models/**` (`model`), then that file stays `model` and `roles.yaml` keeps
  its bytes.
- (LNG-18) Given a second `roles` or `generate` with no file added, then no call is made; given an answer that
  leaves `Rakefile` unclassified, then no later call lists it; given a new `.coffee` file an added rule matches,
  then no call is made; given a new unmatched `Procfile`, then one call is made, listing `Procfile` alone.
- (LNG-19) Given 454 unmatched files, then the call lists the 400 shallowest, the warning gives the count left
  unclassified, and the next run sends the 54 others.

- (LNG-20) Given `source_extensions: [rb, erb, jbuilder, scss]`, then `app/views/orders/show.html.erb`,
  `app/views/orders/show.json.jbuilder` and `app/assets/stylesheets/app.scss` stay other files and `scan` prints
  `erb, jbuilder, scss`; `app/models/order.rb` is source.
- (LNG-20) Given `source_extensions: [vue, svelte, astro]`, then their files are promoted: they hold code and
  markup together.

## Non-goals

- The same quality of docs on every stack: it depends on what a model writes and is measured by the quality
  benchmark, not required here.
- An adapter per framework (a route parser, an ORM parser): rejected by `docs/findings.md` §2, a framework is recognised by
  the LLM from the tree and the manifests.
- Reading view templates (`.erb`, `.blade.php`, `.cshtml`) as source.
- A language outside the reference set beyond the generic path of LNG-1 to LNG-3.
- Generated, vendored or minified code.
- Telling the test libraries of a Gradle version catalog from the others: the catalog does not say which
  configuration uses them.
- A domain class named like a test (`BloodTest.java`, `LabTest.php`): read as a test by the suffix of LNG-6.
- Reading a long file past its first 8 parts: the end is dropped with a warning, whatever the language.
- Reading the content of an unmatched file to give it a role: the call of LNG-16 sees its path only.
