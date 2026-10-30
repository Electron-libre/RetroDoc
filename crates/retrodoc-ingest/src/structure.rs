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
    #[cfg(feature = "php")]
    Php,
    #[cfg(feature = "go")]
    Go,
    #[cfg(feature = "c")]
    C,
    #[cfg(feature = "cpp")]
    Cpp,
    #[cfg(feature = "swift")]
    Swift,
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
    Php,
    Go,
    /// C and C++, whose headers are shared.
    C,
    Swift,
}

/// Each extension this build parses, without the dot, and its language.
const EXTENSIONS: &[(&str, Lang)] = &[
    #[cfg(feature = "ruby")]
    ("rb", Lang::Ruby),
    #[cfg(feature = "ruby")]
    ("rake", Lang::Ruby),
    #[cfg(feature = "ruby")]
    ("gemspec", Lang::Ruby),
    #[cfg(feature = "rust")]
    ("rs", Lang::Rust),
    #[cfg(feature = "javascript")]
    ("js", Lang::JavaScript),
    #[cfg(feature = "javascript")]
    ("jsx", Lang::JavaScript),
    #[cfg(feature = "javascript")]
    ("mjs", Lang::JavaScript),
    #[cfg(feature = "javascript")]
    ("cjs", Lang::JavaScript),
    #[cfg(feature = "typescript")]
    ("ts", Lang::TypeScript),
    #[cfg(feature = "typescript")]
    ("mts", Lang::TypeScript),
    #[cfg(feature = "typescript")]
    ("cts", Lang::TypeScript),
    #[cfg(feature = "typescript")]
    ("tsx", Lang::Tsx),
    #[cfg(feature = "python")]
    ("py", Lang::Python),
    #[cfg(feature = "java")]
    ("java", Lang::Java),
    #[cfg(feature = "kotlin")]
    ("kt", Lang::Kotlin),
    #[cfg(feature = "kotlin")]
    ("kts", Lang::Kotlin),
    #[cfg(feature = "scala")]
    ("scala", Lang::Scala),
    #[cfg(feature = "csharp")]
    ("cs", Lang::CSharp),
    #[cfg(feature = "php")]
    ("php", Lang::Php),
    #[cfg(feature = "go")]
    ("go", Lang::Go),
    #[cfg(feature = "c")]
    ("c", Lang::C),
    // A header may be C or C++: the C++ grammar reads both, the C one when it is alone.
    #[cfg(all(feature = "c", not(feature = "cpp")))]
    ("h", Lang::C),
    #[cfg(feature = "cpp")]
    ("h", Lang::Cpp),
    #[cfg(feature = "cpp")]
    ("cpp", Lang::Cpp),
    #[cfg(feature = "cpp")]
    ("cc", Lang::Cpp),
    #[cfg(feature = "cpp")]
    ("cxx", Lang::Cpp),
    #[cfg(feature = "cpp")]
    ("hpp", Lang::Cpp),
    #[cfg(feature = "cpp")]
    ("hh", Lang::Cpp),
    #[cfg(feature = "cpp")]
    ("hxx", Lang::Cpp),
    #[cfg(feature = "swift")]
    ("swift", Lang::Swift),
];

impl Lang {
    fn of(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        EXTENSIONS
            .iter()
            .find(|(known, _)| *known == ext)
            .map(|&(_, lang)| lang)
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
            #[cfg(feature = "php")]
            Self::Php => Family::Php,
            #[cfg(feature = "go")]
            Self::Go => Family::Go,
            #[cfg(feature = "c")]
            Self::C => Family::C,
            #[cfg(feature = "cpp")]
            Self::Cpp => Family::C,
            #[cfg(feature = "swift")]
            Self::Swift => Family::Swift,
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
            #[cfg(feature = "php")]
            Self::Php => config!(
                tree_sitter_php::LANGUAGE_PHP,
                &format!("{}\n{PHP_EXTRA_QUERY}", tree_sitter_php::TAGS_QUERY)
            ),
            #[cfg(feature = "go")]
            Self::Go => config!(tree_sitter_go::LANGUAGE, tree_sitter_go::TAGS_QUERY),
            #[cfg(feature = "c")]
            Self::C => config!(tree_sitter_c::LANGUAGE, C_QUERY),
            #[cfg(feature = "cpp")]
            Self::Cpp => config!(
                tree_sitter_cpp::LANGUAGE,
                &format!("{C_QUERY}\n{CPP_EXTRA_QUERY}")
            ),
            #[cfg(feature = "swift")]
            Self::Swift => config!(tree_sitter_swift::LANGUAGE, SWIFT_QUERY),
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

/// Constructors, enums and records, which the Java grammar's query leaves out.
#[cfg(feature = "java")]
const JAVA_EXTRA_QUERY: &str = "
(constructor_declaration name: (identifier) @name) @definition.method
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

/// The Scala grammar's query is not exported by its crate: this one keeps its types, functions (abstract
/// ones included), calls and instantiations.
#[cfg(feature = "scala")]
const SCALA_QUERY: &str = "
(class_definition name: (identifier) @name) @definition.class
(object_definition name: (identifier) @name) @definition.class
(enum_definition name: (identifier) @name) @definition.class
(trait_definition name: (identifier) @name) @definition.interface
(function_definition name: (identifier) @name) @definition.function
(function_declaration name: (identifier) @name) @definition.function
(call_expression function: (identifier) @name) @reference.call
(call_expression function: (field_expression field: (identifier) @name)) @reference.call
(call_expression function: (generic_function function: (identifier) @name)) @reference.call
(instance_expression (type_identifier) @name) @reference.class
";

/// The C# grammar's query, without its pattern that captures a bare `@module` (which `tree-sitter-tags`
/// rejects), plus the constructors, structs, records, enums, plain and generic calls it leaves out.
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
(constructor_declaration name: (identifier) @name) @definition.method
(struct_declaration name: (identifier) @name) @definition.class
(record_declaration name: (identifier) @name) @definition.class
(enum_declaration name: (identifier) @name) @definition.class
(invocation_expression function: (identifier) @name) @reference.call
(invocation_expression function: (generic_name (identifier) @name)) @reference.call
(invocation_expression function: (member_access_expression name: (generic_name (identifier) @name))) @reference.call
";

/// Enums, and the calls and instantiations of a plain name, which the PHP grammar's query leaves out.
#[cfg(feature = "php")]
const PHP_EXTRA_QUERY: &str = "
(enum_declaration name: (name) @name) @definition.class
(function_call_expression function: (name) @name) @reference.call
(object_creation_expression (name) @name) @reference.class
";

/// The C grammar's query tags a function by its declarator, one line, and no call: this one tags the
/// whole definition (prototypes left out) and the calls. C++ reads it too.
#[cfg(any(feature = "c", feature = "cpp"))]
const C_QUERY: &str = "
(struct_specifier name: (type_identifier) @name body: (_)) @definition.class
(union_specifier name: (type_identifier) @name body: (_)) @definition.class
(enum_specifier name: (type_identifier) @name body: (_)) @definition.type
(type_definition declarator: (type_identifier) @name) @definition.type
(function_definition declarator: [
  (function_declarator declarator: (identifier) @name)
  (pointer_declarator declarator: (function_declarator declarator: (identifier) @name))
  (pointer_declarator declarator: (pointer_declarator declarator: (function_declarator declarator: (identifier) @name)))
]) @definition.function
(call_expression function: (identifier) @name) @reference.call
(call_expression function: (field_expression field: (field_identifier) @name)) @reference.call
";

/// Classes, namespaces, functions returning a reference, methods defined in or out of their class
/// (destructors and operators included, whatever they return), and qualified calls.
#[cfg(feature = "cpp")]
const CPP_EXTRA_QUERY: &str = "
(class_specifier name: (type_identifier) @name body: (_)) @definition.class
(namespace_definition name: (namespace_identifier) @name) @definition.module
(function_definition declarator: (reference_declarator (function_declarator declarator: (identifier) @name))) @definition.function
(function_definition declarator: [
  (function_declarator declarator: [(field_identifier) (destructor_name) (operator_name)] @name)
  (function_declarator declarator: (qualified_identifier name: [
    (identifier) @name (destructor_name) @name (operator_name) @name
    (qualified_identifier name: [(identifier) (destructor_name) (operator_name)] @name)
  ]))
  (pointer_declarator declarator: (function_declarator declarator: (qualified_identifier name: [(identifier) (operator_name)] @name)))
  (reference_declarator (function_declarator declarator: (qualified_identifier name: [(identifier) (operator_name)] @name)))
]) @definition.method
(call_expression function: (qualified_identifier name: (identifier) @name)) @reference.call
";

/// The Swift grammar's query spans a method over its whole class and tags no call: this one tags each
/// function and initializer over its own body, and the calls. Structs and enums are `class_declaration`s
/// too.
#[cfg(feature = "swift")]
const SWIFT_QUERY: &str = "
(class_declaration name: (type_identifier) @name) @definition.class
(protocol_declaration name: (type_identifier) @name) @definition.interface
(function_declaration name: (simple_identifier) @name) @definition.function
(protocol_function_declaration name: (simple_identifier) @name) @definition.function
(init_declaration \"init\" @name) @definition.function
(call_expression . (simple_identifier) @name) @reference.call
(call_expression . (navigation_expression suffix: (navigation_suffix suffix: (simple_identifier) @name))) @reference.call
";

/// Whether `path` is in a language this build can parse.
#[must_use]
pub fn is_supported(path: &Path) -> bool {
    Lang::of(path).is_some()
}

/// Whether this build parses both files as one language: a name one defines is one the other can call.
/// JavaScript and TypeScript are one, as they import each other; so are Java, Kotlin and Scala, and C
/// and C++.
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
    EXTENSIONS.iter().map(|&(ext, _)| ext).collect()
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
            let signature = signature(text, &name);
            structure.definitions.push(Definition {
                name,
                kind: config.syntax_type_name(tag.syntax_type_id).to_string(),
                start_line,
                end_line: line_at(last_byte).max(start_line),
                signature,
            });
        } else {
            let line = line_at(tag.name_range.start);
            structure.references.push(Reference { name, line });
        }
    }
    Some(structure)
}

/// The line of a definition that declares it, trimmed and bounded: the first one naming it past its
/// annotations and attributes (`@Override`, `[HttpGet]`), else its first line.
fn signature(text: &str, name: &str) -> String {
    let first = text.lines().next().unwrap_or("");
    let declaration = text
        .lines()
        .map(str::trim)
        .find(|line| !line.starts_with(['@', '[']) && line.contains(name))
        .unwrap_or(first.trim());
    declaration.chars().take(MAX_SIGNATURE_CHARS).collect()
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

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "php")]
    #[test]
    fn reads_php_classes_methods_and_calls() {
        let source = "<?php\nclass Order {\n  function ship() {\n    check($this);\n    $this->repo->save($this);\n  }\n}\n\nenum Status {}\n";
        let found = defs("app/Models/Order.php", source);
        has(&found, "Order", "class", (2, 7));
        has(&found, "ship", "function", (3, 6));
        has(&found, "Status", "class", (9, 9));
        assert!(calls("a.php", source, "check", 4));
        assert!(calls("a.php", source, "save", 5));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "go")]
    #[test]
    fn reads_go_types_methods_and_calls() {
        let source = "package shop\n\ntype Order struct {\n\tID int\n}\n\nfunc (o Order) Ship() bool {\n\treturn check(o)\n}\n";
        let found = defs("shop/order.go", source);
        has(&found, "Order", "type", (3, 5));
        has(&found, "Ship", "method", (7, 9));
        assert!(calls("a.go", source, "check", 8));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "c")]
    #[test]
    fn reads_c_structs_whole_functions_and_calls() {
        let source = "struct order {\n  int id;\n};\n\nint ship(struct order *o) {\n  return check(o);\n}\n\nchar *name(void) {\n  return o->label(o);\n}\n";
        let found = defs("src/order.c", source);
        has(&found, "order", "class", (1, 3));
        has(&found, "ship", "function", (5, 7));
        has(&found, "name", "function", (9, 11));
        assert!(calls("a.c", source, "check", 6));
        assert!(calls("a.c", source, "label", 10));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "cpp")]
    #[test]
    fn reads_cpp_classes_methods_and_calls_and_headers_as_cpp() {
        let source = "class Order {\n  bool ship();\n};\n\nbool Order::ship() {\n  repo.save(*this);\n  return check(this);\n}\n";
        let found = defs("src/order.cpp", source);
        has(&found, "Order", "class", (1, 3));
        has(&found, "ship", "method", (5, 8));
        assert!(calls("a.cpp", source, "save", 6));
        assert!(calls("a.cpp", source, "check", 7));
        let header = defs("include/order.h", "class Order {\n  int id;\n};\n");
        has(&header, "Order", "class", (1, 3));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "swift")]
    #[test]
    fn reads_swift_classes_functions_and_calls() {
        let source = "class Order {\n  func ship() -> Bool {\n    repo.save(self)\n    return check(self)\n  }\n}\n\nprotocol Repo {}\n";
        let found = defs("Sources/Shop/Order.swift", source);
        has(&found, "Order", "class", (1, 6));
        has(&found, "ship", "function", (2, 5));
        has(&found, "Repo", "interface", (8, 8));
        assert!(calls("a.swift", source, "save", 3));
        assert!(calls("a.swift", source, "check", 4));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(all(feature = "java", feature = "csharp", feature = "swift"))]
    #[test]
    fn constructors_and_initializers_are_definitions() {
        let java = "class Order {\n  public Order(int x) {\n    init();\n  }\n}\n";
        has(&defs("A.java", java), "Order", "method", (2, 4));
        let csharp = "class Order {\n  public Order(int x) {\n    Init();\n  }\n}\n";
        has(&defs("A.cs", csharp), "Order", "method", (2, 4));
        let swift = "class Order {\n  init(qty: Int) {\n    setup()\n  }\n}\n";
        has(&defs("A.swift", swift), "init", "function", (2, 4));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(all(feature = "csharp", feature = "scala", feature = "php"))]
    #[test]
    fn generic_calls_and_plain_instantiations_are_references() {
        assert!(calls(
            "A.cs",
            "class A {\n  void B() {\n    Ok(Load<int>());\n  }\n}\n",
            "Load",
            3
        ));
        assert!(calls(
            "A.scala",
            "object A {\n  def b = foo[Int](1)\n}\n",
            "foo",
            2
        ));
        assert!(calls(
            "a.php",
            "<?php\nfunction b() {\n  new Service();\n}\n",
            "Service",
            3
        ));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(feature = "scala")]
    #[test]
    fn abstract_scala_defs_are_definitions_and_new_is_a_reference() {
        let source =
            "trait Repo {\n  def load(): Int\n}\n\nobject A {\n  val o = new Order(1)\n}\n";
        has(&defs("A.scala", source), "load", "function", (2, 2));
        assert!(calls("A.scala", source, "Order", 6));
    }

    // spec: LNG-8 (internal: the structure is a library API no command prints)
    #[cfg(all(feature = "c", feature = "cpp"))]
    #[test]
    fn c_and_cpp_functions_are_definitions_whatever_their_declarator() {
        let c = "char **names(void) {\n  return 0;\n}\n";
        has(&defs("a.c", c), "names", "function", (1, 3));
        let cpp = "Order* Order::clone() {\n  return 0;\n}\nconst std::string& Order::name() const {\n  return n;\n}\nOrder::~Order() {\n}\nvoid shop::Order::ship() {\n}\nbool Order::operator==(const Order& o) const {\n  return true;\n}\n";
        let found = defs("a.cpp", cpp);
        has(&found, "clone", "method", (1, 3));
        has(&found, "name", "method", (4, 6));
        has(&found, "~Order", "method", (7, 8));
        has(&found, "ship", "method", (9, 10));
        has(&found, "operator==", "method", (11, 13));
    }

    #[cfg(feature = "java")]
    #[test]
    fn a_signature_is_the_declaration_not_its_annotations() {
        let source = "class A {\n  @Override\n  @Deprecated\n  public void ship() {\n  }\n}\n";
        let structure = parse(Path::new("A.java"), source).unwrap();
        let ship = structure
            .definitions
            .iter()
            .find(|d| d.name == "ship")
            .unwrap();
        assert_eq!(ship.signature, "public void ship() {");
        assert_eq!(ship.start_line, 2);
    }

    #[cfg(all(feature = "c", not(feature = "cpp")))]
    #[test]
    fn without_cpp_a_header_is_read_as_c() {
        assert!(is_supported(Path::new("order.h")));
        assert!(same_language(Path::new("a.c"), Path::new("b.h")));
    }

    // spec: LNG-8 (internal: `same_language` is a library API no command prints)
    #[test]
    fn kotlin_and_scala_are_of_java_and_c_of_cpp() {
        assert_eq!(
            same_language(Path::new("a.kt"), Path::new("b.java")),
            cfg!(all(feature = "kotlin", feature = "java"))
        );
        assert_eq!(
            same_language(Path::new("a.scala"), Path::new("b.kt")),
            cfg!(all(feature = "scala", feature = "kotlin"))
        );
        assert!(!same_language(Path::new("a.cs"), Path::new("b.java")));
        assert_eq!(
            same_language(Path::new("a.c"), Path::new("b.h")),
            cfg!(feature = "c")
        );
        assert!(!same_language(Path::new("a.swift"), Path::new("b.c")));
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
        assert!(!same_language(Path::new("a.zig"), Path::new("b.zig")));
    }

    #[test]
    fn another_language_or_extension_is_not_parsed() {
        assert!(parse(Path::new("main.zig"), "const std = @import(\"std\");").is_none());
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
