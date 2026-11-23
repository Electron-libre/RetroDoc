//! Python: imports resolved to the files of their modules (absolute from the repository root or from a directory
//! holding a top-level package, relative by their dots), the names they bind followed where the file uses them,
//! and the calls on `self`, `cls` or a known class looked up in the class, then its ancestors.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use tree_sitter::{Node, Parser};

use super::{read, Counts};

/// Longest chain of ancestors walked to find a method, against cycles the code would not run.
const MAX_ANCESTORS: usize = 64;

/// The files of the repository by module name, and the module name of each file.
#[derive(Debug, Default)]
struct Modules {
    by_name: BTreeMap<String, PathBuf>,
    /// The name relative to the file's package root, used for its relative imports.
    own_name: BTreeMap<PathBuf, String>,
}

impl Modules {
    fn new(files: &[PathBuf]) -> Self {
        let packages: BTreeSet<&Path> = files
            .iter()
            .filter(|f| f.file_name().is_some_and(|n| n == "__init__.py"))
            .filter_map(|f| f.parent())
            .collect();
        let mut modules = Self::default();
        for file in files {
            // The package root: the first directory up that is not a package.
            let mut root = file.parent().unwrap_or(Path::new(""));
            while packages.contains(root) {
                match root.parent() {
                    Some(parent) => root = parent,
                    None => break,
                }
            }
            let Some(own) = module_name(file.strip_prefix(root).unwrap_or(file)) else {
                continue;
            };
            modules
                .by_name
                .entry(own.clone())
                .or_insert_with(|| file.clone());
            if let Some(from_root) = module_name(file) {
                modules
                    .by_name
                    .entry(from_root)
                    .or_insert_with(|| file.clone());
            }
            modules.own_name.insert(file.clone(), own);
        }
        modules
    }

    /// The longest module `start` then `rest` names, with how many of `rest` it took.
    fn longest(&self, start: &str, rest: &[String]) -> Option<(String, usize)> {
        let mut name = start.to_string();
        let mut found = self.by_name.contains_key(&name).then(|| (name.clone(), 0));
        for (taken, part) in rest.iter().enumerate() {
            name = format!("{name}.{part}");
            if self.by_name.contains_key(&name) {
                found = Some((name.clone(), taken + 1));
            }
        }
        found
    }

    /// The absolute module name of an import written in `file` (`..models` → `shop.models`).
    fn absolute(&self, file: &Path, dots: usize, name: &str) -> Option<String> {
        if dots == 0 {
            return Some(name.to_string());
        }
        let own = self.own_name.get(file)?;
        let mut package: Vec<&str> = own.split('.').collect();
        if file.file_name().is_none_or(|n| n != "__init__.py") {
            package.pop();
        }
        for _ in 1..dots {
            package.pop()?;
        }
        if !name.is_empty() {
            package.push(name);
        }
        Some(package.join("."))
    }
}

/// `shop/orders.py` → `shop.orders`, `shop/__init__.py` → `shop`.
fn module_name(path: &Path) -> Option<String> {
    let stem = path.to_str()?.strip_suffix(".py")?;
    let mut parts: Vec<&str> = stem.split('/').collect();
    if parts.last() == Some(&"__init__") {
        parts.pop();
    }
    (!parts.is_empty()).then(|| parts.join("."))
}

/// What an imported name stands for.
#[derive(Debug, Clone)]
enum Binding {
    Module(String),
    /// A name of a module: a class, a function, a constant.
    Symbol(String, String),
}

/// A use of names: a dotted chain whose head is a name (`queries.open_orders`), or a method on `self` or `cls`.
#[derive(Debug)]
enum Use {
    Chain(Vec<String>),
    OnSelf { class: String, method: String },
}

#[derive(Debug, Default)]
struct Class {
    bases: Vec<Vec<String>>,
    methods: BTreeSet<String>,
}

#[derive(Debug, Default)]
struct File {
    bindings: BTreeMap<String, Binding>,
    /// The modules its imports name, one per imported name.
    imported: Vec<String>,
    classes: BTreeMap<String, Class>,
    uses: Vec<Use>,
}

/// Reads one parsed file.
struct Reader<'a> {
    src: &'a str,
    path: &'a Path,
    modules: &'a Modules,
    file: File,
}

impl<'a> Reader<'a> {
    fn text(&self, node: Node) -> &'a str {
        &self.src[node.byte_range()]
    }

    /// `a.b.c` as its names, when it is names all the way down.
    fn chain(&self, node: Node) -> Option<Vec<String>> {
        match node.kind() {
            "identifier" => Some(vec![self.text(node).to_string()]),
            "attribute" => {
                let mut chain = self.chain(node.child_by_field_name("object")?)?;
                chain.push(
                    self.text(node.child_by_field_name("attribute")?)
                        .to_string(),
                );
                Some(chain)
            }
            _ => None,
        }
    }

    fn bind(&mut self, alias: String, binding: Binding, imported: String) {
        self.file.imported.push(imported);
        self.file.bindings.insert(alias, binding);
    }

    /// `import a.b.c` binds `a`; `import a.b as x` binds `x` to `a.b`.
    fn import(&mut self, node: Node) {
        let mut cursor = node.walk();
        for name in node.children_by_field_name("name", &mut cursor) {
            let (dotted, alias) = if name.kind() == "aliased_import" {
                let Some(dotted) = name.child_by_field_name("name") else {
                    continue;
                };
                let alias = name.child_by_field_name("alias").map(|a| self.text(a));
                (self.text(dotted), alias)
            } else {
                (self.text(name), None)
            };
            let head = dotted.split('.').next().unwrap_or(dotted);
            let (alias, module) = alias.map_or((head, head), |alias| (alias, dotted));
            self.bind(
                alias.to_string(),
                Binding::Module(module.to_string()),
                dotted.to_string(),
            );
        }
    }

    /// `from m import n as y`: a module `m.n` when there is one, else the name `n` of `m`.
    fn import_from(&mut self, node: Node) {
        let Some(module) = node.child_by_field_name("module_name") else {
            return;
        };
        let (dots, name) = if module.kind() == "relative_import" {
            let mut cursor = module.walk();
            let mut dots = 0;
            let mut name = "";
            for child in module.children(&mut cursor) {
                match child.kind() {
                    "import_prefix" => dots = self.text(child).matches('.').count(),
                    "dotted_name" => name = self.text(child),
                    _ => {}
                }
            }
            (dots, name)
        } else {
            (0, self.text(module))
        };
        let Some(module) = self.modules.absolute(self.path, dots, name) else {
            return;
        };
        let mut cursor = node.walk();
        let names: Vec<Node> = node.children_by_field_name("name", &mut cursor).collect();
        for name in names {
            let (imported, alias) = if name.kind() == "aliased_import" {
                let Some(imported) = name.child_by_field_name("name") else {
                    continue;
                };
                let alias = name.child_by_field_name("alias").map(|a| self.text(a));
                (self.text(imported), alias)
            } else {
                (self.text(name), None)
            };
            let alias = alias.unwrap_or(imported).to_string();
            let submodule = format!("{module}.{imported}");
            if self.modules.by_name.contains_key(&submodule) {
                self.bind(alias, Binding::Module(submodule.clone()), submodule);
            } else {
                self.bind(
                    alias,
                    Binding::Symbol(module.clone(), imported.to_string()),
                    module.clone(),
                );
            }
        }
    }

    fn class(&mut self, node: Node, outer: Option<&str>) {
        let Some(name) = node.child_by_field_name("name").map(|n| self.text(n)) else {
            return;
        };
        let mut class = Class::default();
        if let Some(bases) = node.child_by_field_name("superclasses") {
            let mut cursor = bases.walk();
            for base in bases.named_children(&mut cursor) {
                if let Some(chain) = self.chain(base) {
                    class.bases.push(chain);
                }
                self.collect(base, outer);
            }
        }
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.named_children(&mut cursor) {
                let function = if child.kind() == "decorated_definition" {
                    child.child_by_field_name("definition")
                } else {
                    Some(child)
                };
                if let Some(function) = function.filter(|f| f.kind() == "function_definition") {
                    if let Some(method) = function.child_by_field_name("name") {
                        class.methods.insert(self.text(method).to_string());
                    }
                }
            }
            self.collect(body, Some(name));
        }
        self.file.classes.insert(name.to_string(), class);
    }

    /// The types and default values of a function's parameters, and its body; not the names it binds.
    fn function(&mut self, node: Node, class: Option<&str>) {
        if let Some(parameters) = node.child_by_field_name("parameters") {
            let mut cursor = parameters.walk();
            for parameter in parameters.named_children(&mut cursor) {
                for field in ["type", "value"] {
                    if let Some(part) = parameter.child_by_field_name(field) {
                        self.collect(part, class);
                    }
                }
            }
        }
        for field in ["return_type", "body"] {
            if let Some(part) = node.child_by_field_name(field) {
                self.collect(part, class);
            }
        }
    }

    fn collect(&mut self, node: Node, class: Option<&str>) {
        match node.kind() {
            "import_statement" => self.import(node),
            "import_from_statement" => self.import_from(node),
            "future_import_statement" | "comment" => {}
            "class_definition" => self.class(node, class),
            "function_definition" => self.function(node, class),
            "keyword_argument" => {
                if let Some(value) = node.child_by_field_name("value") {
                    self.collect(value, class);
                }
            }
            "identifier" | "attribute" => match self.chain(node) {
                Some(chain) => {
                    let on_self = matches!(chain[0].as_str(), "self" | "cls");
                    let used = match (class, on_self, chain.get(1)) {
                        (Some(class), true, Some(method)) => Use::OnSelf {
                            class: class.to_string(),
                            method: method.clone(),
                        },
                        _ => Use::Chain(chain),
                    };
                    self.file.uses.push(used);
                }
                None => {
                    if let Some(object) = node.child_by_field_name("object") {
                        self.collect(object, class);
                    }
                }
            },
            _ => {
                let mut cursor = node.walk();
                let children: Vec<Node> = node.named_children(&mut cursor).collect();
                for child in children {
                    self.collect(child, class);
                }
            }
        }
    }
}

/// A class: its file and its name.
type ClassRef = (PathBuf, String);

/// Where a chain leads: a file, and the class it names there with the names left after it.
type Followed<'c> = (PathBuf, Option<(ClassRef, &'c [String])>);

/// The parsed files, and how they resolve their names.
struct Program<'a> {
    modules: &'a Modules,
    files: BTreeMap<PathBuf, File>,
}

impl Program<'_> {
    fn module_file(&self, name: &str) -> Option<&PathBuf> {
        self.modules.by_name.get(name)
    }

    fn class(&self, class: &ClassRef) -> Option<&Class> {
        self.files.get(&class.0)?.classes.get(&class.1)
    }

    /// Where a chain written in `file` leads: the file it names, and the class it names there with the names left
    /// after it.
    fn follow<'c>(&self, file: &Path, chain: &'c [String]) -> Option<Followed<'c>> {
        let (head, rest) = chain.split_first()?;
        let parsed = self.files.get(file)?;
        let (target, rest): (PathBuf, &[String]) = match parsed.bindings.get(head) {
            Some(Binding::Module(name)) => {
                let (module, taken) = self.modules.longest(name, rest)?;
                (self.module_file(&module)?.clone(), &rest[taken..])
            }
            Some(Binding::Symbol(module, name)) => {
                let target = self.module_file(module)?.clone();
                let class = (target.clone(), name.clone());
                let found = self.class(&class).map(|_| (class, rest));
                return Some((target, found));
            }
            None if parsed.classes.contains_key(head) => {
                let class = (file.to_path_buf(), head.clone());
                return Some((file.to_path_buf(), Some((class, rest))));
            }
            None => return None,
        };
        let class = rest.split_first().and_then(|(name, after)| {
            let class = (target.clone(), name.clone());
            self.class(&class).map(|_| (class, after))
        });
        Some((target, class))
    }

    /// The file defining `method` in `class`, else in its ancestors, nearest first.
    fn method(&self, class: ClassRef, method: &str) -> Option<PathBuf> {
        let mut queue = VecDeque::from([class]);
        let mut seen: BTreeSet<ClassRef> = BTreeSet::new();
        while let Some(class) = queue.pop_front() {
            if seen.len() >= MAX_ANCESTORS || !seen.insert(class.clone()) {
                continue;
            }
            let Some(definition) = self.class(&class) else {
                continue;
            };
            if definition.methods.contains(method) {
                return Some(class.0);
            }
            for base in &definition.bases {
                if let Some((_, Some((parent, [])))) = self.follow(&class.0, base) {
                    queue.push_back(parent);
                }
            }
        }
        None
    }

    /// The files `file` references through `used`.
    fn targets(&self, file: &Path, used: &Use) -> Vec<PathBuf> {
        match used {
            Use::OnSelf { class, method } => self
                .method((file.to_path_buf(), class.clone()), method)
                .into_iter()
                .collect(),
            Use::Chain(chain) => {
                let Some((target, class)) = self.follow(file, chain) else {
                    return Vec::new();
                };
                let mut targets = vec![target];
                if let Some((class, [method, ..])) = class {
                    targets.extend(self.method(class, method));
                }
                targets
            }
        }
    }
}

/// Adds to `references` the resolved references of the Python files among `files`.
pub(super) fn resolve(repo_root: &Path, files: &[PathBuf], references: &mut Counts) {
    let python: Vec<PathBuf> = files
        .iter()
        .filter(|f| f.extension().is_some_and(|e| e == "py"))
        .cloned()
        .collect();
    if python.is_empty() {
        return;
    }
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .is_err()
    {
        return;
    }
    let modules = Modules::new(&python);
    let mut parsed = BTreeMap::new();
    for path in &python {
        let Some(src) = read(repo_root, path) else {
            continue;
        };
        let Some(tree) = parser.parse(&src, None) else {
            continue;
        };
        let mut reader = Reader {
            src: &src,
            path,
            modules: &modules,
            file: File::default(),
        };
        reader.collect(tree.root_node(), None);
        parsed.insert(path.clone(), reader.file);
    }
    let program = Program {
        modules: &modules,
        files: parsed,
    };
    for (path, file) in &program.files {
        let counts = references.entry(path.clone()).or_default();
        let imported = file
            .imported
            .iter()
            .filter_map(|m| program.modules.longest(m, &[]))
            .filter_map(|(m, _)| program.module_file(&m).cloned());
        let used = file.uses.iter().flat_map(|u| program.targets(path, u));
        for target in imported.chain(used) {
            *counts.entry(target).or_insert(0) += 1;
        }
    }
}
