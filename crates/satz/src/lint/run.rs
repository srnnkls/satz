//! Running a plan: each rule's queries over the corpus, their rows turned
//! into diagnostics.

use std::collections::{BTreeMap, BTreeSet};
use std::iter;

use super::compile::Compiled;
use super::load::{Clause, Method};
use super::module::{Placeholder, Sigil, placeholders};
use super::{
    Corpus, Diagnostic, Example, Expect, Outcome, Plan, RuleId, RunError, Score, Selection,
};
use crate::query::{self, Capture, Environment, Hit, Row, Table, Value};
use crate::similarity::{Shingles, fold};
use crate::tree::{NodeRef, Origin, Position, Tree};

/// How many shared phrases a finding reports.
const SHARED_PHRASES: usize = 5;

impl Plan {
    /// Runs the selected rules that settings leave enabled over `corpus`,
    /// returning their findings ordered by file, line, column, rule and
    /// message, without repeats.
    ///
    /// A rule's own params and data override the environment's. Rules
    /// comparing texts under a semantic model run only when the settings
    /// enable them, and findings in a file whose frontmatter names the rule
    /// under the disable key are dropped. Within one module, a finding
    /// scored by cosine is dropped where a finding scored by jaccard or
    /// containment points at the same place, so a semantic rule reports
    /// only what the lexical ones miss.
    ///
    /// # Errors
    /// Returns [`RunError::Query`] naming the rule whose query failed.
    pub fn run(&self, corpus: &Corpus, rules: &Selection) -> Result<Vec<Diagnostic>, RunError> {
        let mut environment = corpus.environment.clone();
        let mut found = Vec::new();
        let mut lexical = BTreeSet::new();
        for (meta, compiled) in self.rules.iter().zip(&self.compiled) {
            if self.disabled.contains(&meta.id) || !rules.includes(meta.id.as_str()) {
                continue;
            }
            scope(
                &mut environment,
                &corpus.environment,
                &compiled.params,
                &compiled.data,
            );
            let diagnostics = self
                .evaluate(compiled, &corpus.tree, &environment)
                .map_err(|source| RunError::Query {
                    rule: meta.id.clone(),
                    source,
                })?;
            for diagnostic in diagnostics {
                if matches!(
                    diagnostic.score(),
                    Some(Score::Jaccard { .. } | Score::Containment { .. })
                ) {
                    lexical.insert((meta.module.as_str(), place(&diagnostic)));
                }
                found.push((meta.module.as_str(), diagnostic));
            }
        }
        let mut found: Vec<Diagnostic> = found
            .into_iter()
            .filter(|(module, diagnostic)| {
                !matches!(diagnostic.score(), Some(Score::Cosine { .. }))
                    || !lexical.contains(&(*module, place(diagnostic)))
            })
            .map(|(_, diagnostic)| diagnostic)
            .collect();
        found.sort_by(|a, b| order(a).cmp(&order(b)));
        found.dedup_by(|a, b| order(a) == order(b));
        Ok(found)
    }

    /// Runs one example against a corpus the caller built from its text,
    /// with the example's params and data over the rule's.
    ///
    /// # Errors
    /// Returns [`RunError::UnknownRule`] when the plan lacks the example's
    /// rule, and [`RunError::Query`] when the rule's query fails.
    pub fn run_example(&self, example: &Example, corpus: &Corpus) -> Result<Outcome, RunError> {
        let compiled = self
            .rules
            .iter()
            .position(|rule| rule.id == example.rule)
            .and_then(|index| self.compiled.get(index))
            .ok_or_else(|| RunError::UnknownRule(example.rule.clone()))?;
        let mut params = compiled.params.clone();
        params.extend(example.params.clone());
        let mut data = compiled.data.clone();
        data.extend(example.data.clone());
        let mut environment = corpus.environment.clone();
        scope(&mut environment, &corpus.environment, &params, &data);
        let found = self
            .evaluate(compiled, &corpus.tree, &environment)
            .map_err(|source| RunError::Query {
                rule: example.rule.clone(),
                source,
            })?;
        Ok(if found.is_empty() == (example.expect == Expect::Passes) {
            Outcome::Pass
        } else {
            Outcome::Fail { found: found.len() }
        })
    }

    fn evaluate(
        &self,
        compiled: &Compiled,
        tree: &Tree,
        environment: &Environment,
    ) -> Result<Vec<Diagnostic>, query::RunError> {
        let mut found = Vec::new();
        for clause in &compiled.clauses {
            let query = &clause.rule.query;
            if query.is_semantic() && !self.semantic || query.impossible(environment) {
                continue;
            }
            if let Some(name) = clause
                .rule
                .params()
                .into_keys()
                .find(|name| !environment.params.contains_key(name))
            {
                return Err(query::RunError::MissingParam(name));
            }
            for row in query.run(tree, environment)? {
                found.extend(self.diagnostic(clause, &row, tree, environment));
            }
        }
        Ok(found)
    }

    fn diagnostic(
        &self,
        clause: &Clause,
        row: &Row,
        tree: &Tree,
        environment: &Environment,
    ) -> Option<Diagnostic> {
        let rule = &clause.rule;
        let at = rule
            .at
            .iter()
            .chain(rule.query.captures())
            .find_map(|name| node(row, tree, name))?;
        if self.disabled_in_file(at, &rule.id) {
            return None;
        }
        let origin = match rule.focus.as_ref().and_then(|var| row.vars.get(var)) {
            Some(value) => focus(&at.node().origin, &at.node().text, value),
            None => at.node().origin.clone(),
        };
        let related = rule
            .related
            .iter()
            .find_map(|name| node(row, tree, name))
            .map(|related| vec![related.node().origin.clone()])
            .unwrap_or_default();
        let shared = rule
            .shared
            .as_ref()
            .and_then(|(a, b)| Some(shared_phrases(node(row, tree, a)?, node(row, tree, b)?)))
            .unwrap_or_default();
        let message = render(&rule.message, row, tree, &environment.params)
            .replace("{shared}", &shared.join("; "));
        let diagnostic =
            Diagnostic::new(rule.id.clone(), origin, rule.severity, message).with_related(related);
        Some(match score(clause, row, shared, environment) {
            Some(score) => diagnostic.with_score(score),
            None => diagnostic,
        })
    }

    /// Whether the file holding `at` names `rule` under the disable key of
    /// its frontmatter.
    fn disabled_in_file(&self, at: NodeRef<'_>, rule: &RuleId) -> bool {
        let Some(file) = iter::once(at)
            .chain(at.ancestors())
            .find(|node| node.node().kind == "file")
        else {
            return false;
        };
        file.child(0)
            .filter(|head| head.node().kind == "frontmatter")
            .is_some_and(|head| {
                head.children().any(|entry| {
                    let attrs = &entry.node().attrs;
                    attrs.get("key") == Some(&self.disable_key)
                        && attrs.get("value").is_some_and(|value| {
                            value.split(',').any(|id| id.trim() == rule.as_str())
                        })
                })
            })
    }
}

/// Sets the environment's params and data to the corpus's with a rule's
/// over them.
fn scope(
    environment: &mut Environment,
    corpus: &Environment,
    params: &BTreeMap<String, Value>,
    data: &BTreeMap<String, Table>,
) {
    environment.params.clone_from(&corpus.params);
    environment
        .params
        .extend(params.iter().map(|(k, v)| (k.clone(), v.clone())));
    environment.data.clone_from(&corpus.data);
    environment
        .data
        .extend(data.iter().map(|(k, v)| (k.clone(), v.clone())));
}

/// Where a finding points: its file, line and column.
fn place(diagnostic: &Diagnostic) -> (String, Option<(u32, u32)>) {
    let origin = diagnostic.origin();
    (
        origin.file().to_owned(),
        origin.position().map(|p| (p.line(), p.column())),
    )
}

fn order(diagnostic: &Diagnostic) -> (&str, Option<(u32, u32)>, &str, &str) {
    let origin = diagnostic.origin();
    (
        origin.file(),
        origin.position().map(|p| (p.line(), p.column())),
        diagnostic.rule().as_str(),
        diagnostic.message(),
    )
}

/// The first node `name` captured in `row`.
fn node<'t>(row: &Row, tree: &'t Tree, name: &str) -> Option<NodeRef<'t>> {
    match hit(row, name)? {
        Hit::Node(id) => tree.get(*id),
        Hit::Row(_) => None,
    }
}

fn hit<'r>(row: &'r Row, name: &str) -> Option<&'r Hit> {
    match row.captures.get(name)? {
        Capture::Nodes(hits) | Capture::Collected(hits) => hits.first(),
        Capture::Value(_) | Capture::Values(_) => None,
    }
}

fn shared_phrases(a: NodeRef<'_>, b: NodeRef<'_>) -> Vec<String> {
    let a = Shingles::of(&a.node().text, Shingles::DEFAULT_SIZE);
    let b = Shingles::of(&b.node().text, Shingles::DEFAULT_SIZE);
    a.shared(&b, SHARED_PHRASES)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn score(
    clause: &Clause,
    row: &Row,
    shared: Vec<String>,
    environment: &Environment,
) -> Option<Score> {
    let value = number(row.value(clause.rule.score.as_deref()?)?)?;
    Some(match clause.method? {
        Method::Jaccard => Score::Jaccard { value, shared },
        Method::Containment => Score::Containment { value, shared },
        Method::Cosine => Score::Cosine {
            value,
            model: environment
                .embed
                .as_ref()
                .map(|model| model.model().to_owned())
                .unwrap_or_default(),
        },
    })
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => Some(*n),
        Value::Text(text) => text.parse().ok(),
    }
}

/// `origin` moved to where `value` first appears in the node's `text`,
/// matched case-insensitively as `contains` matches.
fn focus(origin: &Origin, text: &str, value: &str) -> Origin {
    let (Some(position), Some(index)) = (origin.position(), find_folded(text, value)) else {
        return origin.clone();
    };
    let before = &text[..index];
    let (line, column) = match before.rfind('\n') {
        Some(newline) => (
            position
                .line()
                .saturating_add(count(before.matches('\n').count())),
            count(index - newline),
        ),
        None => (
            position.line(),
            position.column().saturating_add(count(index)),
        ),
    };
    Position::new(
        position.span(),
        line..=position.end_line().max(line),
        column,
    )
    .map_or_else(|_| origin.clone(), |at| origin.with_position(Some(at)))
}

/// The byte offset of the first place in `text` that matches `value` char by
/// lower-cased char; none for an empty `value`.
fn find_folded(text: &str, value: &str) -> Option<usize> {
    let needle: Vec<char> = value.chars().map(fold).collect();
    if needle.is_empty() {
        return None;
    }
    let haystack: Vec<(usize, char)> = text.char_indices().map(|(i, c)| (i, fold(c))).collect();
    haystack
        .windows(needle.len())
        .find(|window| window.iter().map(|&(_, c)| c).eq(needle.iter().copied()))
        .map(|window| window[0].0)
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// `message` with its placeholders filled from `row` and `params`.
fn render(message: &str, row: &Row, tree: &Tree, params: &BTreeMap<String, Value>) -> String {
    let mut rendered = String::with_capacity(message.len());
    let mut last = 0;
    for placeholder in placeholders(message) {
        rendered.push_str(&message[last..placeholder.span.start]);
        rendered.push_str(&fill(&placeholder, row, tree, params));
        last = placeholder.span.end;
    }
    rendered.push_str(&message[last..]);
    rendered
}

fn fill(
    placeholder: &Placeholder<'_>,
    row: &Row,
    tree: &Tree,
    params: &BTreeMap<String, Value>,
) -> String {
    let name = placeholder.name;
    match placeholder.sigil {
        Sigil::Param => params.get(name).map(Value::to_string).unwrap_or_default(),
        Sigil::Var => row
            .vars
            .get(name)
            .map(|value| format(&Value::Text(value.clone()), placeholder.key))
            .unwrap_or_default(),
        Sigil::Capture => {
            if let Some(value) = row.value(name) {
                return format(value, placeholder.key);
            }
            match hit(row, name) {
                Some(Hit::Node(id)) => tree
                    .get(*id)
                    .map(|node| field(node, placeholder.key))
                    .unwrap_or_default(),
                Some(Hit::Row(attrs)) => placeholder
                    .key
                    .and_then(|key| attrs.get(key))
                    .cloned()
                    .unwrap_or_default(),
                None => String::new(),
            }
        }
    }
}

/// A value under a format: `percent`, or a number of decimals.
fn format(value: &Value, spec: Option<&str>) -> String {
    match (number(value), spec) {
        (Some(n), Some("percent")) => format!("{:.1}%", n * 100.0),
        (Some(n), Some(spec)) => spec
            .parse::<usize>()
            .map_or_else(|_| value.to_string(), |digits| format!("{n:.digits$}")),
        _ => value.to_string(),
    }
}

/// A captured node's `key`: its location `file:line` without one, its
/// `line`, `column`, trimmed `text` or `lines` read from its origin, or an
/// attribute.
fn field(node: NodeRef<'_>, key: Option<&str>) -> String {
    let node = node.node();
    let position = node.origin.position();
    let line = position.map_or(1, Position::line);
    match key {
        None => format!("{}:{line}", node.origin.file()),
        Some("line") => line.to_string(),
        Some("column") => position.map_or(1, Position::column).to_string(),
        Some("text") => node.text.trim().to_owned(),
        Some("lines") => query::lines(node).to_string(),
        Some(key) => node.attrs.get(key).cloned().unwrap_or_default(),
    }
}
