//! The structure of a source file read with tree-sitter (ADR 0022): its definitions, with the lines
//! they span and their signature, and the names it references. No LLM, no guess: a real parse, for the
//! languages whose grammar is compiled in (one cargo feature each, all on by default). The grammars'
//! own `tags.scm` queries say what a definition and a reference are, so a language is added with its
//! extensions and its query. Any other file gets `None` and keeps the lexical paths.

use std::path::Path;
#[cfg_attr(not(feature = "grammar"), allow(unused_imports))]
use std::sync::OnceLock;

#[cfg_attr(not(feature = "grammar"), allow(unused_imports))]
use tree_sitter::Language;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

/// Longest signature kept, in characters.
const MAX_SIGNATURE_CHARS: usize = 200;

/// A class, module, function, method… with the lines it spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub name: String,
    /// What the grammar's query calls it: `class`, `method`, `function`, `module`…
    pub kind: String,
    /// First line, 1-based.
    pub start_line: u32,
    /// Last line, 1-based and inclusive.
    pub end_line: u32,
    /// First line of the definition, trimmed.
    pub signature: String,
}

/// A name used at a line (a call, in most grammars).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    /// 1-based.
    pub line: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Structure {
    pub definitions: Vec<Definition>,
    pub references: Vec<Reference>,
}

/// The languages this build can parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    #[cfg(feature = "ruby")]
    Ruby,
    #[cfg(feature = "rust")]
    Rust,
    #[cfg(feature = "javascript")]
    JavaScript,
    #[cfg(feature = "typescript")]
    TypeScript,
    #[cfg(feature = "typescript")]
    Tsx,
    #[cfg(feature = "python")]
    Python,
    #[cfg(feature = "java")]
    Java,
    #[cfg(feature = "kotlin")]
    Kotlin,
    #[cfg(feature = "scala")]
    Scala,
    #[cfg(feature = "csharp")]
    CSharp,
}

/// Languages that call each other: a name one defines is one the other can call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // the families of the languages left out of this build
enum Family {
    Ruby,
    Rust,
    /// JavaScript and its TypeScript dialects, which import each other.
    Web,
    Python,
    /// Java, Kotlin and Scala.
    Jvm,
    CSharp,
}

impl Lang {
    fn of(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            #[cfg(feature = "ruby")]
            "rb" | "rake" | "gemspec" => Some(Self::Ruby),
            #[cfg(feature = "rust")]
            "rs" => Some(Self::Rust),
            #[cfg(feature = "javascript")]
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            #[cfg(feature = "typescript")]
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            #[cfg(feature = "typescript")]
            "tsx" => Some(Self::Tsx),
            #[cfg(feature = "python")]
            "py" => Some(Self::Python),
            #[cfg(feature = "java")]
            "java" => Some(Self::Java),
            #[cfg(feature = "kotlin")]
            "kt" | "kts" => Some(Self::Kotlin),
            #[cfg(feature = "scala")]
            "scala" => Some(Self::Scala),
            #[cfg(feature = "csharp")]
            "cs" => Some(Self::CSharp),
            _ => None,
        }
    }

    fn family(self) -> Family {
        match self {
            #[cfg(feature = "ruby")]
            Self::Ruby => Family::Ruby,
            #[cfg(feature = "rust")]
            Self::Rust => Family::Rust,
            #[cfg(feature = "javascript")]
            Self::JavaScript => Family::Web,
            #[cfg(feature = "typescript")]
            Self::TypeScript | Self::Tsx => Family::Web,
            #[cfg(feature = "python")]
            Self::Python => Family::Python,
            #[cfg(feature = "java")]
            Self::Java => Family::Jvm,
            #[cfg(feature = "kotlin")]
            Self::Kotlin => Family::Jvm,
            #[cfg(feature = "scala")]
            Self::Scala => Family::Jvm,
            #[cfg(feature = "csharp")]
            Self::CSharp => Family::CSharp,
        }
    }

    /// The compiled query of the language, built once.
    fn config(self) -> Option<&'static TagsConfiguration> {
        #[allow(unused_macros)] // no language compiled in
        macro_rules! config {
            ($language:expr, $query:expr) => {{
                static CONFIG: OnceLock<Option<TagsConfiguration>> = OnceLock::new();
                CONFIG
                    .get_or_init(|| {
                        TagsConfiguration::new(Language::from($language), $query, "").ok()
                    })
                    .as_ref()
            }};
        }
        match self {
            #[cfg(feature = "ruby")]
            Self::Ruby => config!(tree_sitter_ruby::LANGUAGE, tree_sitter_ruby::TAGS_QUERY),
            #[cfg(feature = "rust")]
            Self::Rust => config!(tree_sitter_rust::LANGUAGE, tree_sitter_rust::TAGS_QUERY),
            #[cfg(feature = "javascript")]
            Self::JavaScript => config!(
                tree_sitter_javascript::LANGUAGE,
                tree_sitter_javascript::TAGS_QUERY
            ),
            #[cfg(feature = "typescript")]
            Self::TypeScript => config!(
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
                &typescript_query()
            ),
            #[cfg(feature = "typescript")]
            Self::Tsx => config!(tree_sitter_typescript::LANGUAGE_TSX, &typescript_query()),
            #[cfg(feature = "python")]
            Self::Python => config!(tree_sitter_python::LANGUAGE, tree_sitter_python::TAGS_QUERY),
            #[cfg(feature = "java")]
            Self::Java => config!(
                tree_sitter_java::LANGUAGE,
                &format!("{}\n{JAVA_EXTRA_QUERY}", tree_sitter_java::TAGS_QUERY)
            ),
            #[cfg(feature = "kotlin")]
            Self::Kotlin => config!(tree_sitter_kotlin_ng::LANGUAGE, KOTLIN_QUERY),
            #[cfg(feature = "scala")]
            Self::Scala => config!(tree_sitter_scala::LANGUAGE, SCALA_QUERY),
            #[cfg(feature = "csharp")]
            Self::CSharp => config!(tree_sitter_c_sharp::LANGUAGE, &csharp_query()),
        }
    }
}

/// The TypeScript grammar's query only holds what JavaScript lacks (signatures, interfaces, modules):
/// the JavaScript one is added for classes, functions and calls.
#[cfg(feature = "typescript")]
fn typescript_query() -> String {
    format!(
        "{}\n{}\n{TS_EXTRA_QUERY}",
        tree_sitter_typescript::TAGS_QUERY,
        tree_sitter_javascript::TAGS_QUERY
    )
}

/// Classes and enums, whose name is a `type_identifier` in TypeScript.
#[cfg(feature = "typescript")]
const TS_EXTRA_QUERY: &str = "
(class_declaration name: (type_identifier) @name) @definition.class
(enum_declaration name: (identifier) @name) @definition.class
";

/// Enums and records, which the Java grammar's query leaves out.
#[cfg(feature = "java")]
const JAVA_EXTRA_QUERY: &str = "
(enum_declaration name: (identifier) @name) @definition.class
(record_declaration name: (identifier) @name) @definition.class
";

/// No Kotlin grammar compatible with this `tree-sitter` ships a query: this one is ours. Interfaces are
/// `class_declaration`s too; a method is a `function`, as Kotlin calls it.
#[cfg(feature = "kotlin")]
const KOTLIN_QUERY: &str = "
(class_declaration name: (identifier) @name) @definition.class
(object_declaration name: (identifier) @name) @definition.class
(function_declaration name: (identifier) @name) @definition.function
(call_expression . (identifier) @name) @reference.call
(call_expression . (navigation_expression (identifier) @name .)) @reference.call
";

/// The Scala grammar's query is not exported by its crate: this one keeps its types, functions and calls.
#[cfg(feature = "scala")]
const SCALA_QUERY: &str = "
(class_definition name: (identifier) @name) @definition.class
(object_definition name: (identifier) @name) @definition.class
(enum_definition name: (identifier) @name) @definition.class
(trait_definition name: (identifier) @name) @definition.interface
(function_definition name: (identifier) @name) @definition.function
(call_expression function: (identifier) @name) @reference.call
(call_expression function: (field_expression field: (identifier) @name)) @reference.call
";

/// The C# grammar's query, without its pattern that captures a bare `@module` (which `tree-sitter-tags`
/// rejects), plus the structs, records, enums and plain calls it leaves out.
#[cfg(feature = "csharp")]
fn csharp_query() -> String {
    let own: Vec<&str> = tree_sitter_c_sharp::TAGS_QUERY
        .lines()
        .filter(|line| !line.trim_end().ends_with(" @module"))
        .collect();
    format!("{}\n{CSHARP_EXTRA_QUERY}", own.join("\n"))
}

#[cfg(feature = "csharp")]
const CSHARP_EXTRA_QUERY: &str = "
(struct_declaration name: (identifier) @name) @definition.class
(record_declaration name: (identifier) @name) @definition.class
(enum_declaration name: (identifier) @name) @definition.class
(invocation_expression function: (identifier) @name) @reference.call
";

/// Whether `path` is in a language this build can parse.
#[must_use]
pub fn is_supported(path: &Path) -> bool {
    Lang::of(path).is_some()
}

/// Whether this build parses both files as one language: a name one defines is one the other can call.
/// JavaScript and TypeScript are one, as they import each other; so are Java, Kotlin and Scala.
#[must_use]
pub fn same_language(a: &Path, b: &Path) -> bool {
    match (Lang::of(a), Lang::of(b)) {
        (Some(a), Some(b)) => a.family() == b.family(),
        _ => false,
    }
}

/// Whether files with this extension (with or without the dot, any case) can be parsed.
#[must_use]
pub fn is_supported_extension(extension: &str) -> bool {
    Lang::of(Path::new(&format!(
        "x.{}",
        extension.trim_start_matches('.')
    )))
    .is_some()
}

/// The extensions of the languages this build can parse, without the dot.
#[must_use]
pub fn supported_extensions() -> Vec<&'static str> {
    [
        #[cfg(feature = "ruby")]
        &["rb", "rake", "gemspec"][..],
        #[cfg(feature = "rust")]
        &["rs"][..],
        #[cfg(feature = "javascript")]
        &["js", "jsx", "mjs", "cjs"][..],
        #[cfg(feature = "typescript")]
        &["ts", "mts", "cts", "tsx"][..],
        #[cfg(feature = "python")]
        &["py"][..],
        #[cfg(feature = "java")]
        &["java"][..],
        #[cfg(feature = "kotlin")]
        &["kt", "kts"][..],
        #[cfg(feature = "scala")]
        &["scala"][..],
        #[cfg(feature = "csharp")]
        &["cs"][..],
    ]
    .concat()
}

/// The definitions and references of `source`, read as the language `path`'s extension names. `None` for
/// a language without a grammar here or a source the parser gives up on; a file with syntax errors still
/// gives what could be read.
#[must_use]
pub fn parse(path: &Path, source: &str) -> Option<Structure> {
    let config = Lang::of(path)?.config()?;
    let mut context = TagsContext::new();
    let (tags, _) = context
        .generate_tags(config, source.as_bytes(), None)
        .ok()?;
    // Byte offset of the start of each line, to turn offsets into 1-based lines.
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let line_at = |offset: usize| {
        u32::try_from(line_starts.partition_point(|&start| start <= offset)).unwrap_or(u32::MAX)
    };
    let mut structure = Structure::default();
    for tag in tags.flatten() {
        // A tag whose range is not on text boundaries is skipped, not the whole file.
        let Some(name) = source.get(tag.name_range.clone()).map(str::to_string) else {
            continue;
        };
        if tag.is_definition {
            let Some(text) = source.get(tag.range.clone()) else {
                continue;
            };
            // `range` is the whole definition; `span` is only the line of its name.
            let start_line = line_at(tag.range.start);
            let last_byte = tag.range.end.saturating_sub(1).max(tag.range.start);
            structure.definitions.push(Definition {
                name,
                kind: config.syntax_type_name(tag.syntax_type_id).to_string(),
                start_line,
                end_line: line_at(last_byte).max(start_line),
                signature: signature(text),
            });
        } else {
            let line = line_at(tag.name_range.start);
            structure.references.push(Reference { name, line });
        }
    }
    Some(structure)
}

/// The first line of a definition, trimmed and bounded.
fn signature(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    first.chars().take(MAX_SIGNATURE_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)] // only used by the per-language tests
    fn defs(path: &str, source: &str) -> Vec<(String, String, u32, u32)> {
        parse(Path::new(path), source)
            .unwrap()
            .definitions
            .into_iter()
            .map(|d| (d.name, d.kind, d.start_line, d.end_line))
            .collect()
    }

    #[allow(dead_code)]
    fn has(found: &[(String, String, u32, u32)], name: &str, kind: &str, lines: (u32, u32)) {
        assert!(
            found.contains(&(name.to_string(), kind.to_string(), lines.0, lines.1)),
            "{name} ({kind}) at {lines:?} not in {found:?}"
        );
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn reads_ruby_classes_and_methods_with_their_lines() {
        let source = "class Invoice\n  def total\n    items.sum\n  end\n\n  def self.build\n    new\n  end\nend\n";
        let found = defs("app/models/invoice.rb", source);
        has(&found, "Invoice", "class", (1, 9));
        has(&found, "total", "method", (2, 4));
        has(&found, "build", "method", (6, 8));
        let structure = parse(Path::new("a.rb"), source).unwrap();
        assert_eq!(structure.definitions[1].signature, "def total");
        assert!(structure
            .references
            .iter()
            .any(|r| r.name == "sum" && r.line == 3));
    }

    #[cfg(feature = "rust")]
    #[test]
    fn reads_rust_items_and_calls() {
        let source = "pub struct Order {\n    id: u32,\n}\n\npub fn ship(order: &Order) -> bool {\n    check(order)\n}\n";
        let found = defs("src/lib.rs", source);
        has(&found, "Order", "class", (1, 3));
        has(&found, "ship", "function", (5, 7));
        let structure = parse(Path::new("a.rs"), source).unwrap();
        assert_eq!(
            structure.definitions[1].signature,
            "pub fn ship(order: &Order) -> bool {"
        );
        assert!(structure
            .references
            .iter()
            .any(|r| r.name == "check" && r.line == 6));
    }

    #[cfg(feature = "javascript")]
    #[test]
    fn reads_javascript_classes_and_functions() {
        let source =
            "class Cart {\n  add(item) {\n    return save(item);\n  }\n}\n\nfunction pay() {}\n";
        let found = defs("src/cart.js", source);
        has(&found, "Cart", "class", (1, 5));
        has(&found, "add", "method", (2, 4));
        has(&found, "pay", "function", (7, 7));
    }

    #[cfg(feature = "typescript")]
    #[test]
    fn reads_typescript_and_tsx() {
        let source = "interface User {\n  id: number;\n}\n\nexport class Repo {\n  find(id: number): User {\n    return load(id);\n  }\n}\n";
        let found = defs("src/repo.ts", source);
        has(&found, "User", "interface", (1, 3));
        has(&found, "Repo", "class", (5, 9));
        let tsx = defs("src/view.tsx", "function View() {\n  return <div />;\n}\n");
        has(&tsx, "View", "function", (1, 3));
    }

    #[cfg(feature = "python")]
    #[test]
    fn reads_python_classes_and_functions() {
        let source = "class Basket:\n    def add(self, item):\n        return store(item)\n\ndef checkout():\n    pass\n";
        let found = defs("shop/basket.py", source);
        has(&found, "Basket", "class", (1, 3));
        has(&found, "add", "function", (2, 3));
        has(&found, "checkout", "function", (5, 6));
    }

    /// Whether `name` is referenced at `line` in `source`, read as `path`'s language.
    #[allow(dead_code)]
    fn calls(path: &str, source: &str, name: &str, line: u32) -> bool {
        parse(Path::new(path), source)
            .unwrap()
            .references
            .iter()
            .any(|r| r.name == name && r.line == line)
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "java")]
    #[test]
    fn reads_java_classes_methods_and_calls() {
        let source = "class Order {\n  void ship() {\n    check(this);\n  }\n}\n\ninterface Repo {}\n\nrecord Line(int qty) {}\n";
        let found = defs("src/main/java/shop/Order.java", source);
        has(&found, "Order", "class", (1, 5));
        has(&found, "ship", "method", (2, 4));
        has(&found, "Repo", "interface", (7, 7));
        has(&found, "Line", "class", (9, 9));
        assert!(calls("a.java", source, "check", 3));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "kotlin")]
    #[test]
    fn reads_kotlin_classes_objects_functions_and_calls() {
        let source = "class Order(val id: Int) {\n  fun ship(): Boolean {\n    return check(this)\n  }\n}\n\nobject Registry {\n  fun all() = repo.load()\n}\n";
        let found = defs("src/main/kotlin/shop/Order.kt", source);
        has(&found, "Order", "class", (1, 5));
        has(&found, "ship", "function", (2, 4));
        has(&found, "Registry", "class", (7, 9));
        has(&found, "all", "function", (8, 8));
        assert!(calls("a.kt", source, "check", 3));
        assert!(calls("a.kt", source, "load", 8));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "scala")]
    #[test]
    fn reads_scala_classes_traits_objects_and_calls() {
        let source = "class Order {\n  def ship(): Boolean = {\n    check(this)\n  }\n}\n\ntrait Repo\n\nobject Shop {\n  def all = repo.load()\n}\n";
        let found = defs("app/shop/Order.scala", source);
        has(&found, "Order", "class", (1, 5));
        has(&found, "ship", "function", (2, 4));
        has(&found, "Repo", "interface", (7, 7));
        has(&found, "Shop", "class", (9, 11));
        assert!(calls("a.scala", source, "check", 3));
        assert!(calls("a.scala", source, "load", 10));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "csharp")]
    #[test]
    fn reads_csharp_classes_methods_and_calls() {
        let source = "namespace Shop {\n  class Order {\n    void Ship() {\n      Check(this);\n      repo.Save(this);\n    }\n  }\n\n  record Line(int Qty);\n}\n";
        let found = defs("Shop/Order.cs", source);
        has(&found, "Order", "class", (2, 7));
        has(&found, "Ship", "method", (3, 6));
        has(&found, "Line", "class", (9, 9));
        assert!(calls("a.cs", source, "Check", 4));
        assert!(calls("a.cs", source, "Save", 5));
    }

    // spec: LNG-8 (internal: `same_language` is a library API no command prints)
    #[test]
    fn kotlin_and_scala_are_of_java() {
        assert_eq!(
            same_language(Path::new("a.kt"), Path::new("b.java")),
            cfg!(all(feature = "kotlin", feature = "java"))
        );
        assert_eq!(
            same_language(Path::new("a.scala"), Path::new("b.kt")),
            cfg!(all(feature = "scala", feature = "kotlin"))
        );
        assert!(!same_language(Path::new("a.cs"), Path::new("b.java")));
    }

    #[test]
    fn extensions_are_matched_with_or_without_the_dot_in_any_case() {
        assert_eq!(is_supported_extension(".RB"), cfg!(feature = "ruby"));
        assert!(!is_supported_extension("zig"));
        for ext in supported_extensions() {
            assert!(is_supported_extension(ext), "{ext}");
        }
    }

    #[test]
    fn a_file_is_of_its_own_language_and_typescript_is_of_javascript() {
        assert_eq!(
            same_language(Path::new("a.rb"), Path::new("b.rake")),
            cfg!(feature = "ruby")
        );
        assert!(!same_language(Path::new("a.rb"), Path::new("b.py")));
        assert_eq!(
            same_language(Path::new("a.tsx"), Path::new("b.ts")),
            cfg!(feature = "typescript")
        );
        assert_eq!(
            same_language(Path::new("a.tsx"), Path::new("b.js")),
            cfg!(all(feature = "typescript", feature = "javascript"))
        );
        assert!(!same_language(Path::new("a.go"), Path::new("b.go")));
    }

    #[test]
    fn another_language_or_extension_is_not_parsed() {
        assert!(parse(Path::new("main.go"), "package main").is_none());
        assert!(parse(Path::new("README"), "text").is_none());
        assert!(!is_supported(Path::new("a.zig")));
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn a_file_with_a_syntax_error_still_gives_what_could_be_read() {
        let found = defs("a.rb", "class Ok\n  def fine\n  end\nend\n\ndef broken(\n");
        assert!(found.iter().any(|d| d.0 == "Ok"));
    }
}
