//! Ruby: `require_relative` and `require` resolved to the files they name, constants to the files defining the
//! class or module of that qualified name (from the innermost namespace outwards, then the top level), and the
//! calls on `self`, with no receiver or on a constant looked up in the class, then its parent classes, then the
//! modules it includes or extends. A class reopened in several files is defined by all of them.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};

use tree_sitter::{Node, Parser};

use super::{read, Counts};

/// Longest chain of ancestors walked to find a method, against cycles the code would not run.
const MAX_ANCESTORS: usize = 64;

/// A constant as written (`Shop::Order`, `::Order`) and the namespaces around it, innermost last.
#[derive(Debug, Clone)]
struct Constant {
    path: String,
    top_level: bool,
    nesting: Vec<String>,
}

#[derive(Debug)]
enum Use {
    Constant(Constant),
    /// `Order.find`: the class, then the method looked up from it.
    OnConstant(Constant, String),
    /// `self.f`, `f(…)` or a bare `f` in the class of that qualified name.
    OnSelf(String, String),
    /// A file a `require` names, when it is one of the files.
    Require(PathBuf),
}

/// What a file says of a class or module.
#[derive(Debug, Default)]
struct Definition {
    methods: BTreeSet<String>,
    parents: Vec<Constant>,
    modules: Vec<Constant>,
}

#[derive(Debug, Default)]
struct File {
    /// Qualified name → what this file defines in it.
    definitions: BTreeMap<String, Definition>,
    uses: Vec<Use>,
}

struct Reader<'a> {
    src: &'a str,
    path: &'a Path,
    files: &'a BTreeSet<PathBuf>,
    file: File,
    /// Qualified names of the enclosing classes and modules, innermost last.
    nesting: Vec<String>,
    /// The locals and parameters of the method being read: a bare name among them is not a call.
    locals: BTreeSet<String>,
}

impl<'a> Reader<'a> {
    fn text(&self, node: Node) -> &'a str {
        &self.src[node.byte_range()]
    }

    /// A constant or `A::B` as written, and whether it starts at the top level (`::A`).
    fn constant_path(&self, node: Node) -> Option<(String, bool)> {
        match node.kind() {
            "constant" => Some((self.text(node).to_string(), false)),
            "scope_resolution" => {
                let name = self.text(node.child_by_field_name("name")?);
                match node.child_by_field_name("scope") {
                    None => Some((name.to_string(), true)),
                    Some(scope) => {
                        let (scope, top) = self.constant_path(scope)?;
                        Some((format!("{scope}::{name}"), top))
                    }
                }
            }
            _ => None,
        }
    }

    fn constant(&self, node: Node) -> Option<Constant> {
        let (path, top_level) = self.constant_path(node)?;
        Some(Constant {
            path,
            top_level,
            nesting: self.nesting.clone(),
        })
    }

    fn current(&mut self) -> Option<&mut Definition> {
        let name = self.nesting.last()?.clone();
        Some(self.file.definitions.entry(name).or_default())
    }

    /// `class` and `module`: their body read in their namespace.
    fn namespace(&mut self, node: Node) {
        let Some((path, top)) = node
            .child_by_field_name("name")
            .and_then(|n| self.constant_path(n))
        else {
            return;
        };
        let qualified = match self.nesting.last() {
            Some(outer) if !top => format!("{outer}::{path}"),
            _ => path,
        };
        let parent = node
            .child_by_field_name("superclass")
            .and_then(|s| s.named_child(0));
        let parent_constant = parent.and_then(|p| self.constant(p));
        if let Some(parent) = parent {
            self.collect(parent);
        }
        let definition = self.file.definitions.entry(qualified.clone()).or_default();
        definition.parents.extend(parent_constant);
        let outer_locals = std::mem::take(&mut self.locals);
        self.nesting.push(qualified);
        if let Some(body) = node.child_by_field_name("body") {
            self.collect(body);
        }
        self.nesting.pop();
        self.locals = outer_locals;
    }

    /// `def`: its name a method of the enclosing class, its parameters locals of its body.
    fn method(&mut self, node: Node) {
        if let Some(name) = node.child_by_field_name("name").map(|n| self.text(n)) {
            if let Some(definition) = self.current() {
                definition.methods.insert(name.to_string());
            }
        }
        let outer_locals = std::mem::take(&mut self.locals);
        if let Some(parameters) = node.child_by_field_name("parameters") {
            self.parameters(parameters);
        }
        if let Some(body) = node.child_by_field_name("body") {
            self.collect(body);
        }
        self.locals = outer_locals;
    }

    fn parameters(&mut self, parameters: Node) {
        let mut cursor = parameters.walk();
        for parameter in parameters.named_children(&mut cursor) {
            let name = if parameter.kind() == "identifier" {
                Some(parameter)
            } else {
                parameter.child_by_field_name("name")
            };
            if let Some(name) = name {
                self.locals.insert(self.text(name).to_string());
            }
            if let Some(value) = parameter.child_by_field_name("value") {
                self.collect(value);
            }
        }
    }

    /// The first string argument of a call, when it is a literal.
    fn string_argument(&self, node: Node) -> Option<&'a str> {
        let argument = node.child_by_field_name("arguments")?.named_child(0)?;
        if argument.kind() != "string" || argument.named_child_count() != 1 {
            return None;
        }
        let content = argument.named_child(0)?;
        (content.kind() == "string_content").then(|| self.text(content))
    }

    fn require(&mut self, relative: bool, name: &str) {
        let name = if Path::new(name).extension().is_some_and(|e| e == "rb") {
            name.to_string()
        } else {
            format!("{name}.rb")
        };
        let candidates: Vec<PathBuf> = if relative {
            let dir = self.path.parent().unwrap_or(Path::new(""));
            normalize(&dir.join(&name)).into_iter().collect()
        } else {
            // The load roots: `lib/`, an `app/` directory, the root.
            let mut roots: Vec<PathBuf> = vec![PathBuf::from("lib"), PathBuf::new()];
            roots.extend(
                self.files
                    .iter()
                    .filter_map(|f| {
                        let mut parts = f.components();
                        match (parts.next(), parts.next()) {
                            (Some(Component::Normal(app)), Some(Component::Normal(dir)))
                                if app == "app" =>
                            {
                                Some(Path::new("app").join(dir))
                            }
                            _ => None,
                        }
                    })
                    .collect::<BTreeSet<_>>(),
            );
            roots.into_iter().map(|root| root.join(&name)).collect()
        };
        if let Some(found) = candidates.into_iter().find(|c| self.files.contains(c)) {
            self.file.uses.push(Use::Require(found));
        }
    }

    fn call(&mut self, node: Node) {
        let method = node.child_by_field_name("method").map(|m| self.text(m));
        let receiver = node.child_by_field_name("receiver");
        match (receiver, method) {
            (None, Some(kind @ ("require" | "require_relative"))) => {
                if let Some(name) = self.string_argument(node) {
                    self.require(kind == "require_relative", name);
                }
            }
            (None, Some("include" | "extend" | "prepend")) if !self.nesting.is_empty() => {
                if let Some(arguments) = node.child_by_field_name("arguments") {
                    let mut cursor = arguments.walk();
                    let modules: Vec<Constant> = arguments
                        .named_children(&mut cursor)
                        .filter_map(|a| self.constant(a))
                        .collect();
                    if let Some(definition) = self.current() {
                        definition.modules.extend(modules);
                    }
                }
            }
            (None, Some(method)) => self.on_self(method),
            (Some(receiver), Some(method)) => {
                if receiver.kind() == "self" {
                    self.on_self(method);
                } else if let Some(constant) = self.constant(receiver) {
                    self.file
                        .uses
                        .push(Use::OnConstant(constant, method.to_string()));
                } else {
                    self.collect(receiver);
                }
            }
            (Some(receiver), None) => self.collect(receiver),
            (None, None) => {}
        }
        for field in ["arguments", "block"] {
            if let Some(part) = node.child_by_field_name(field) {
                self.collect(part);
            }
        }
    }

    fn on_self(&mut self, method: &str) {
        if let Some(class) = self.nesting.last() {
            self.file
                .uses
                .push(Use::OnSelf(class.clone(), method.to_string()));
        }
    }

    fn collect(&mut self, node: Node) {
        match node.kind() {
            "comment" => {}
            "class" | "module" => self.namespace(node),
            "method" | "singleton_method" => self.method(node),
            "call" => self.call(node),
            "block_parameters" | "lambda_parameters" => self.parameters(node),
            // Its parameters and new locals end with it.
            "block" | "do_block" | "lambda" => {
                let outer_locals = self.locals.clone();
                self.children(node);
                self.locals = outer_locals;
            }
            "assignment" | "operator_assignment" => {
                if let Some(left) = node.child_by_field_name("left") {
                    self.declare(left);
                }
                if let Some(right) = node.child_by_field_name("right") {
                    self.collect(right);
                }
            }
            "identifier" => {
                let name = self.text(node);
                if !self.locals.contains(name) {
                    self.on_self(name);
                }
            }
            "constant" | "scope_resolution" => match self.constant(node) {
                Some(constant) => self.file.uses.push(Use::Constant(constant)),
                None => {
                    if let Some(scope) = node.child_by_field_name("scope") {
                        self.collect(scope);
                    }
                }
            },
            _ => self.children(node),
        }
    }

    fn children(&mut self, node: Node) {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.named_children(&mut cursor).collect();
        for child in children {
            self.collect(child);
        }
    }

    /// The left side of an assignment: its names are locals (`a, (b, *c) = …`); an attribute or an element
    /// written (`self.a =`, `h[k] =`) is read as code.
    fn declare(&mut self, left: Node) {
        match left.kind() {
            "identifier" => {
                self.locals.insert(self.text(left).to_string());
            }
            "left_assignment_list" | "destructured_left_assignment" | "rest_assignment" => {
                let mut cursor = left.walk();
                let parts: Vec<Node> = left.named_children(&mut cursor).collect();
                for part in parts {
                    self.declare(part);
                }
            }
            _ => self.collect(left),
        }
    }
}

/// `a/b/../c.rb` → `a/c.rb`; `None` when it climbs out of the repository.
fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::Normal(name) => out.push(name),
            _ => {}
        }
    }
    Some(out)
}

/// A class or module of the repository, over every file that defines it.
#[derive(Debug, Default)]
struct Class<'a> {
    files: BTreeSet<&'a Path>,
    /// Method name → the files defining it in this class.
    methods: BTreeMap<&'a str, BTreeSet<&'a Path>>,
    parents: Vec<&'a Constant>,
    modules: Vec<&'a Constant>,
}

struct Program<'a> {
    classes: BTreeMap<&'a str, Class<'a>>,
}

impl<'a> Program<'a> {
    fn new(files: &'a BTreeMap<PathBuf, File>) -> Self {
        let mut classes: BTreeMap<&str, Class> = BTreeMap::new();
        for (path, file) in files {
            for (name, definition) in &file.definitions {
                let class = classes.entry(name).or_default();
                class.files.insert(path);
                for method in &definition.methods {
                    class
                        .methods
                        .entry(method)
                        .or_default()
                        .insert(path.as_path());
                }
                class.parents.extend(&definition.parents);
                class.modules.extend(&definition.modules);
            }
        }
        Self { classes }
    }

    /// The qualified name `constant` designates: from the innermost namespace outwards, then the top level.
    fn lookup(&self, constant: &Constant) -> Option<&'a str> {
        let candidates = constant
            .nesting
            .iter()
            .rev()
            .filter(|_| !constant.top_level)
            .map(|outer| format!("{outer}::{}", constant.path))
            .chain(std::iter::once(constant.path.clone()));
        for candidate in candidates {
            if let Some((name, _)) = self.classes.get_key_value(candidate.as_str()) {
                return Some(name);
            }
        }
        None
    }

    /// The files defining `method` in `class`, else in its parents, then its modules, nearest first.
    fn method(&self, class: &'a str, method: &str) -> BTreeSet<&'a Path> {
        let mut queue = VecDeque::from([class]);
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        while let Some(name) = queue.pop_front() {
            if seen.len() >= MAX_ANCESTORS || !seen.insert(name) {
                continue;
            }
            let Some(class) = self.classes.get(name) else {
                continue;
            };
            if let Some(files) = class.methods.get(method) {
                return files.clone();
            }
            for ancestor in class.parents.iter().chain(&class.modules) {
                queue.extend(self.lookup(ancestor));
            }
        }
        BTreeSet::new()
    }

    fn targets(&self, used: &Use) -> Vec<PathBuf> {
        let owned = |files: BTreeSet<&Path>| files.into_iter().map(Path::to_path_buf).collect();
        match used {
            Use::Require(file) => vec![file.clone()],
            Use::Constant(constant) => self
                .lookup(constant)
                .and_then(|name| self.classes.get(name))
                .map(|class| owned(class.files.clone()))
                .unwrap_or_default(),
            Use::OnConstant(constant, method) => {
                let Some(name) = self.lookup(constant) else {
                    return Vec::new();
                };
                let mut files = self.classes[name].files.clone();
                files.extend(self.method(name, method));
                owned(files)
            }
            Use::OnSelf(class, method) => owned(self.method(class, method)),
        }
    }
}

/// Adds to `references` the resolved references of the Ruby files among `files`.
pub(super) fn resolve(repo_root: &Path, files: &[PathBuf], references: &mut Counts) {
    let ruby: BTreeSet<PathBuf> = files
        .iter()
        .filter(|f| f.extension().is_some_and(|e| e == "rb"))
        .cloned()
        .collect();
    if ruby.is_empty() {
        return;
    }
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_ruby::LANGUAGE.into())
        .is_err()
    {
        return;
    }
    let mut parsed = BTreeMap::new();
    for path in &ruby {
        let Some(src) = read(repo_root, path) else {
            continue;
        };
        let Some(tree) = parser.parse(&src, None) else {
            continue;
        };
        let mut reader = Reader {
            src: &src,
            path,
            files: &ruby,
            file: File::default(),
            nesting: Vec::new(),
            locals: BTreeSet::new(),
        };
        reader.collect(tree.root_node());
        parsed.insert(path.clone(), reader.file);
    }
    let program = Program::new(&parsed);
    for (path, file) in &parsed {
        let counts = references.entry(path.clone()).or_default();
        for target in file.uses.iter().flat_map(|u| program.targets(u)) {
            *counts.entry(target).or_insert(0) += 1;
        }
    }
}
