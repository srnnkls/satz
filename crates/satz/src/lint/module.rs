//! The module reader: a module's `(rule …)`, `(define …)` and `(import …)`
//! forms.

use std::collections::BTreeMap;
use std::fmt;

use super::{RuleId, Severity};
use crate::query::{
    Defines, Kind, ParamKind, Query, ReadError, ReadErrorKind, Reader, Schema, Token, suggest,
    unexpected,
};
use crate::template::{Sigil, placeholders};

/// A lint rule a module declares: a query and how to report its rows.
#[derive(Debug, Clone)]
pub(crate) struct Rule {
    pub(crate) id: RuleId,
    /// Warning unless `:severity error`.
    pub(crate) severity: Severity,
    /// The message template, with `{@capture}`, `{?var}` and `{$param}`
    /// placeholders.
    pub(crate) message: String,
    /// The captures a finding points at, the first one bound winning.
    pub(crate) at: Vec<String>,
    /// The captures of related locations.
    pub(crate) related: Vec<String>,
    /// The variable whose text narrows the finding's position.
    pub(crate) focus: Option<String>,
    /// The value capture holding the finding's score.
    pub(crate) score: Option<String>,
    /// The scoring method's name.
    pub(crate) method: Option<String>,
    /// The two captures whose shared phrases the finding reports.
    pub(crate) shared: Option<(String, String)>,
    pub(crate) query: Query,
}

impl Rule {
    /// The params the rule reads: its query's, plus `{$param}`s of its
    /// message, which read as text.
    pub(crate) fn params(&self) -> BTreeMap<String, ParamKind> {
        let mut params = self.query.params().clone();
        for placeholder in placeholders(&self.message) {
            if placeholder.sigil == Sigil::Param {
                params
                    .entry(placeholder.name.to_owned())
                    .or_insert(ParamKind::Text);
            }
        }
        params
    }
}

/// A module's rules, its defines and the modules it imports.
#[derive(Debug, Clone)]
pub(crate) struct Module {
    /// Rules in source order.
    pub(crate) rules: Vec<Rule>,
    /// The module's own defines, without the imported ones.
    pub(crate) defines: Defines,
    /// Imported module names in source order.
    pub(crate) imports: Vec<String>,
}

impl Module {
    /// Reads the module `source` against `schema`; `import` returns the
    /// defines of a module an `(import name)` form names.
    pub(crate) fn read<E: fmt::Display>(
        source: &str,
        schema: &Schema,
        mut import: impl FnMut(&str) -> Result<Defines, E>,
    ) -> Result<Self, ReadError> {
        let mut reader = Reader::new(source, schema)?;
        let mut module = Self {
            rules: Vec::new(),
            defines: Defines::new(),
            imports: Vec::new(),
        };
        while reader.peek().kind != Kind::Eof {
            let open = reader.next();
            let form = reader.next();
            if open.kind != Kind::Open || form.kind != Kind::Symbol {
                return Err(ReadError::syntax(
                    open.pos,
                    "a module holds (rule ...), (define ...) and (import ...) forms",
                ));
            }
            match form.text.as_str() {
                "import" => {
                    let name = reader.next();
                    if !matches!(name.kind, Kind::Symbol | Kind::String) {
                        return Err(unexpected(&name, "a module name"));
                    }
                    let close = reader.next();
                    if close.kind != Kind::Close {
                        return Err(unexpected(&close, ") closing (import"));
                    }
                    let defines = import(&name.text)
                        .map_err(|e| ReadError::syntax(name.pos, e.to_string()))?;
                    reader.import(&name, defines)?;
                    module.imports.push(name.text);
                }
                "define" => {
                    let (name, define) = reader.define(&open)?;
                    module.defines.insert(name, define);
                }
                "rule" => module.rules.push(rule(&mut reader, source, &open)?),
                other => {
                    return Err(ReadError::syntax(
                        form.pos,
                        format!("unknown module form ({other} ...)"),
                    )
                    .with_hint(suggest(other, &["rule", "define", "import"])));
                }
            }
        }
        Ok(module)
    }
}

/// Reads the `(rule …)` form opened by `open`, whose head the reader has
/// just passed.
fn rule(reader: &mut Reader<'_>, source: &str, open: &Token) -> Result<Rule, ReadError> {
    let id = reader.next();
    let valid = id.kind == Kind::Symbol
        && id.text.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .text
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-');
    let rule_id = RuleId::new(id.text.clone())
        .ok()
        .filter(|_| valid)
        .ok_or_else(|| {
            ReadError::syntax(
                id.pos,
                "a rule needs an id of lowercase letters, digits and hyphens",
            )
            .with_hint("e.g. (rule large-file ...)")
        })?;
    let mut head = Head::default();
    while reader.peek().kind == Kind::Key {
        head.read_key(reader)?;
    }
    let message = head
        .message
        .take()
        .filter(|m| !m.is_empty())
        .ok_or_else(|| ReadError::syntax(open.pos, format!("rule {} needs a :message", id.text)))?;
    reader.printed = placeholders(&message)
        .into_iter()
        .filter(|p| p.sigil == Sigil::Var)
        .map(|p| p.name.to_owned())
        .collect();
    let start = reader.peek().pos;
    let mut query = reader.query(Kind::Close)?;
    let close = reader.next();
    query.set_source(
        source
            .get(start..close.pos.max(start))
            .unwrap_or_default()
            .trim(),
    );
    head.check(&query, &message, &id.text, open.pos)?;
    Ok(Rule {
        id: rule_id,
        severity: head.severity.unwrap_or(Severity::Warning),
        message,
        at: head.at,
        related: head.related,
        focus: head.focus,
        score: head.score,
        method: head.method,
        shared: head.shared,
        query,
    })
}

/// A rule's keys before its query.
#[derive(Default)]
struct Head {
    severity: Option<Severity>,
    message: Option<String>,
    at: Vec<String>,
    related: Vec<String>,
    focus: Option<String>,
    score: Option<String>,
    method: Option<String>,
    shared: Option<(String, String)>,
}

impl Head {
    /// Reads one `:key value` of a rule.
    fn read_key(&mut self, reader: &mut Reader<'_>) -> Result<(), ReadError> {
        let key = reader.next();
        let value = reader.next();
        let expect = |kinds: &[Kind], want: &str| {
            if kinds.contains(&value.kind) {
                Ok(value.text.clone())
            } else {
                Err(unexpected(&value, want))
            }
        };
        match key.text.as_str() {
            "severity" => {
                let severity = value.text.parse();
                let invalid = |_| ReadError::syntax(value.pos, ":severity is warning or error");
                self.severity = Some(severity.map_err(invalid)?);
            }
            "message" => self.message = Some(expect(&[Kind::String], "a \"message\"")?),
            "at" | "related" => {
                let want = format!("a @capture for :{}", key.text);
                let mut captures = vec![expect(&[Kind::Capture], &want)?];
                while reader.peek().kind == Kind::Capture {
                    captures.push(reader.next().text);
                }
                if key.text == "related" {
                    self.related = captures;
                } else {
                    self.at = captures;
                }
            }
            "focus" => self.focus = Some(expect(&[Kind::Var], "a ?variable for :focus")?),
            "score" => {
                let want = "a @capture for :score, as in :score @score";
                self.score = Some(expect(&[Kind::Capture], want)?);
            }
            "method" => {
                let want = "a method name for :method";
                self.method = Some(expect(&[Kind::Symbol, Kind::String], want)?);
            }
            "shared" => {
                let second = reader.next();
                if value.kind != Kind::Capture || second.kind != Kind::Capture {
                    return Err(unexpected(
                        &value,
                        "two @captures for :shared, as in :shared @copy @original",
                    ));
                }
                self.shared = Some((value.text, second.text));
            }
            other => {
                const KEYS: [&str; 8] = [
                    "severity", "message", "at", "related", "focus", "score", "method", "shared",
                ];
                return Err(
                    ReadError::syntax(key.pos, format!("a rule has no key :{other}"))
                        .with_hint(suggest(other, &KEYS)),
                );
            }
        }
        Ok(())
    }

    /// Checks that the captures the rule names are captured by its query.
    fn check(&self, query: &Query, message: &str, id: &str, pos: usize) -> Result<(), ReadError> {
        let unbound = |message: String| ReadError::new(ReadErrorKind::Unbound, pos, message);
        if let Some(score) = &self.score
            && !query.is_value(score)
        {
            return Err(unbound(format!(
                "rule {id} scores with @{score}, which its query does not capture as a value"
            ))
            .with_hint("capture the score, as in (near ?t 0.7 @score)"));
        }
        let pointed = self
            .at
            .iter()
            .chain(&self.related)
            .chain(self.shared.iter().flat_map(|(a, b)| [a, b]));
        for capture in pointed {
            if !query.captures().contains(capture) {
                return Err(unbound(format!(
                    "rule {id} points at @{capture}, which its query does not capture"
                )));
            }
        }
        for placeholder in placeholders(message) {
            if placeholder.sigil == Sigil::Capture
                && !query.captures().iter().any(|c| c == placeholder.name)
            {
                return Err(unbound(format!(
                    "rule {id}'s message uses @{}, which its query does not capture",
                    placeholder.name
                )));
            }
        }
        Ok(())
    }
}
