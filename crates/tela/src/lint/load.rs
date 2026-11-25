//! Module files, tela's std modules and the layers', read into rule groups.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as _;
use std::fmt::Write as _;

use super::header::{Fault, Header};
use super::{CompileError, Expect, InlineRule, Layer, RuleId, Spec};
use crate::frontend::Frontend;
use crate::markdown::{Frontmatter, Markdown};
use crate::query::{self, Defines, Module, Schema, Table, Value};
use crate::tree::{NodeRef, Tree};

/// tela's std modules by name within the std prefix.
const STD: [(&str, &str); 4] = [
    ("duplicates", include_str!("../../std/duplicates.md")),
    ("links", include_str!("../../std/links.md")),
    ("similarity", include_str!("../../std/similarity.md")),
    ("structure", include_str!("../../std/structure.md")),
];

/// How a rule scores its findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Method {
    Jaccard,
    Containment,
    Cosine,
}

impl Method {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "jaccard" => Some(Self::Jaccard),
            "containment" => Some(Self::Containment),
            "cosine" => Some(Self::Cosine),
            _ => None,
        }
    }
}

/// One `(rule …)` form and the method its score follows.
#[derive(Debug, Clone)]
pub(super) struct Clause {
    pub(super) rule: query::Rule,
    pub(super) method: Option<Method>,
}

/// The clauses of one rule id, with the settings they share.
#[derive(Debug, Clone)]
pub(super) struct Group {
    pub(super) id: RuleId,
    pub(super) module: String,
    pub(super) clauses: Vec<Clause>,
    pub(super) params: BTreeMap<String, Value>,
    pub(super) data: BTreeMap<String, Table>,
    pub(super) meta: BTreeMap<String, String>,
    pub(super) samples: Vec<Sample>,
}

/// An example as its module writes it, its settings still untyped.
#[derive(Debug, Clone)]
pub(super) struct Sample {
    pub(super) line: u32,
    pub(super) expect: Expect,
    pub(super) text: String,
    pub(super) settings: Vec<(String, String)>,
}

/// A module read: its rule groups and the defines it exports.
struct Loaded {
    groups: Vec<Group>,
    defines: Defines,
}

/// Reads every module of the std prefix and the layers, resolving imports.
pub(super) struct Loader<'a> {
    files: BTreeMap<String, &'a str>,
    schema: &'a Schema,
    meta_keys: &'a BTreeSet<String>,
    markdown: Markdown,
    loaded: BTreeMap<String, Loaded>,
    loading: BTreeSet<String>,
}

impl<'a> Loader<'a> {
    /// A loader of the std modules under `spec`'s std label and the files of
    /// `layers`, each named by its path without `.md`, under its package.
    pub(super) fn new(layers: &'a [Layer], spec: &'a Spec) -> Result<Self, CompileError> {
        let std = STD
            .iter()
            .map(|&(name, text)| (format!("{}/{name}", spec.labels.std), text));
        let layered = layers.iter().flat_map(|layer| {
            layer.files().iter().map(move |file| {
                let stem = file.path.strip_suffix(".md").unwrap_or(&file.path);
                let name = match layer.package() {
                    Some(package) => format!("{package}/{stem}"),
                    None => stem.to_owned(),
                };
                (name, file.text.as_str())
            })
        });
        let mut files = BTreeMap::new();
        for (name, text) in std.chain(layered) {
            if files.contains_key(&name) {
                return Err(CompileError::DuplicateModule(name));
            }
            files.insert(name, text);
        }
        Ok(Self {
            files,
            schema: &spec.schema,
            meta_keys: &spec.meta_keys,
            markdown: Markdown::new(),
            loaded: BTreeMap::new(),
            loading: BTreeSet::new(),
        })
    }

    /// The rule groups of every module, in module name order.
    pub(super) fn groups(mut self) -> Result<Vec<Group>, CompileError> {
        let names: Vec<String> = self.files.keys().cloned().collect();
        for name in &names {
            self.load(name)?;
        }
        Ok(names
            .iter()
            .filter_map(|name| self.loaded.remove(name))
            .flat_map(|loaded| loaded.groups)
            .collect())
    }

    fn load(&mut self, name: &str) -> Result<&Loaded, CompileError> {
        if !self.loaded.contains_key(name) {
            let text = self.files.get(name).copied().unwrap_or_default();
            self.loading.insert(name.to_owned());
            let loaded = self.read(name, text);
            self.loading.remove(name);
            self.loaded.insert(name.to_owned(), loaded?);
        }
        Ok(&self.loaded[name])
    }

    /// The defines of the module an `(import name)` names.
    fn defines(&mut self, name: &str) -> Result<Defines, String> {
        if !self.files.contains_key(name) {
            return Err(format!("no module {name:?}"));
        }
        if self.loading.contains(name) {
            return Err(format!("module {name:?} imports itself"));
        }
        self.load(name)
            .map(|loaded| loaded.defines.clone())
            .map_err(|error| chain(&error))
    }

    fn read(&mut self, name: &str, text: &str) -> Result<Loaded, CompileError> {
        let malformed = |message: String| CompileError::Malformed {
            module: name.to_owned(),
            message,
        };
        let tree = self.markdown.parse(name, text);
        if let Some(problem) = tree.problems().next() {
            return Err(malformed(problem.node().text.clone()));
        }
        let header = match Frontmatter::parse(name, text).value {
            Some(head) => {
                Header::read(&head.text, self.meta_keys).map_err(|fault| match fault {
                    Fault::UnknownKey(key) => CompileError::UnknownMetaKey {
                        module: name.to_owned(),
                        key,
                    },
                    Fault::Invalid(message) => malformed(message),
                })?
            }
            None => Header::default(),
        };
        let (source, blocks) = hq_source(&tree);
        let mut samples = samples(&tree);
        let schema = self.schema;
        let module = Module::read(&source, schema, |import| self.defines(import))
            .map_err(|error| located(name, &source, &blocks, error))?;
        let mut groups: Vec<Group> = Vec::new();
        for rule in module.rules {
            let method = match &rule.score {
                Some(_) => Some(rule.method.as_deref().and_then(Method::parse).ok_or_else(
                    || CompileError::Method {
                        module: name.to_owned(),
                        rule: rule.id.clone(),
                    },
                )?),
                None => None,
            };
            let clause = Clause { rule, method };
            if let Some(group) = groups.iter_mut().find(|g| g.id == clause.rule.id) {
                group.clauses.push(clause);
                continue;
            }
            groups.push(Group {
                id: clause.rule.id.clone(),
                module: name.to_owned(),
                params: header.params.clone(),
                data: header.data.clone(),
                meta: header.meta(clause.rule.id.as_str()),
                samples: samples.remove(clause.rule.id.as_str()).unwrap_or_default(),
                clauses: vec![clause],
            });
        }
        Ok(Loaded {
            groups,
            defines: module.defines,
        })
    }
}

/// The rule group of an inline rule, read as a one-rule module.
pub(super) fn inline(rule: &InlineRule, spec: &Spec) -> Result<Group, CompileError> {
    let mut head = format!("(rule {} :message {}", rule.id, quote(&rule.message));
    if let Some(severity) = rule.severity {
        let _ = write!(head, " :severity {severity}");
    }
    for (key, captures) in [("at", &rule.at), ("related", &rule.related)] {
        let captures = captures.as_deref().unwrap_or_default();
        if !captures.trim().is_empty() {
            let _ = write!(head, " :{key}");
        }
        for capture in captures.split_whitespace() {
            let _ = write!(head, " @{}", capture.trim_start_matches('@'));
        }
    }
    head.push('\n');
    let source = format!("{head}{}\n)", rule.query);
    let failed = |error: query::Error| {
        let offset = error
            .offset()
            .checked_sub(head.len())
            .map_or(0, |offset| offset.min(rule.query.len()));
        CompileError::Query {
            module: spec.labels.inline.clone(),
            rule: rule.id.clone(),
            source: error.at(offset),
        }
    };
    let no_imports = |name: &str| Err(format!("an inline rule imports no module, not {name:?}"));
    let module = Module::read(&source, &spec.schema, no_imports).map_err(failed)?;
    let [read] = <[query::Rule; 1]>::try_from(module.rules).map_err(|_| {
        failed(query::Error::syntax(
            head.len() + rule.query.len(),
            "the query closes its rule early",
        ))
    })?;
    Ok(Group {
        id: rule.id.clone(),
        module: spec.labels.inline.clone(),
        clauses: vec![Clause {
            rule: read,
            method: None,
        }],
        params: rule.params.clone(),
        data: rule.data.clone(),
        meta: BTreeMap::new(),
        samples: Vec::new(),
    })
}

/// `text` as a tq string.
fn quote(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        match c {
            '"' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// An `hq` block's place: its offset in the joined source and its first
/// line in the module file.
struct Block {
    start: usize,
    line: u32,
}

/// The module's `hq` blocks joined into one source, with where each starts.
fn hq_source(tree: &Tree) -> (String, Vec<Block>) {
    let mut source = String::new();
    let mut blocks = Vec::new();
    for code in tree
        .root()
        .descendants()
        .filter(|node| fenced(*node, &["hq"]))
    {
        blocks.push(Block {
            start: source.len(),
            line: content_line(code),
        });
        source.push_str(&code.node().text);
        source.push('\n');
    }
    (source, blocks)
}

/// A read error moved into the block holding it.
fn located(module: &str, source: &str, blocks: &[Block], error: query::Error) -> CompileError {
    let offset = error.offset().min(source.len());
    let (line, error) = match blocks.iter().rev().find(|block| block.start <= offset) {
        Some(block) => {
            let before = source.get(block.start..offset).unwrap_or_default();
            let newlines = u32::try_from(before.matches('\n').count()).unwrap_or(u32::MAX);
            (
                block.line.saturating_add(newlines),
                error.at(offset - block.start),
            )
        }
        None => (1, error),
    };
    CompileError::Read {
        module: module.to_owned(),
        line,
        source: error,
    }
}

/// The examples by rule id: `md` code blocks under a `### Matches` or
/// `### Passes` section of the rule's section.
fn samples(tree: &Tree) -> BTreeMap<String, Vec<Sample>> {
    let mut samples: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
    for code in tree
        .root()
        .descendants()
        .filter(|node| fenced(*node, &["md", "markdown"]))
    {
        let Some(section) = code.ancestors().find(|n| n.node().kind == "section") else {
            continue;
        };
        let attrs = &section.node().attrs;
        let Some(rule) = section.parent().filter(|p| p.node().kind == "section") else {
            continue;
        };
        if attrs.get("level").map(String::as_str) != Some("3") {
            continue;
        }
        let expect = match attrs.get("title").map(|t| t.to_lowercase()).as_deref() {
            Some("matches") => Expect::Matches,
            Some("passes") => Expect::Passes,
            _ => continue,
        };
        let settings = code
            .node()
            .attrs
            .get("info")
            .map(|info| {
                info.split_whitespace()
                    .skip(1)
                    .filter_map(|field| field.split_once('='))
                    .map(|(key, value)| (key.to_owned(), value.to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let id = rule.node().attrs.get("title").cloned().unwrap_or_default();
        samples.entry(id).or_default().push(Sample {
            line: content_line(code),
            expect,
            text: code.node().text.clone(),
            settings,
        });
    }
    samples
}

fn fenced(node: NodeRef<'_>, langs: &[&str]) -> bool {
    let node = node.node();
    node.kind == "code"
        && node
            .attrs
            .get("lang")
            .is_some_and(|lang| langs.contains(&lang.as_str()))
}

/// The module line of a fenced block's first content line.
fn content_line(code: NodeRef<'_>) -> u32 {
    code.node()
        .origin
        .line()
        .map_or(1, |line| line.saturating_add(1))
}

/// An error with its sources, as one line.
fn chain(error: &CompileError) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let _ = write!(message, ": {cause}");
        source = cause.source();
    }
    message
}
