//! Modules: `(rule …)`, `(define …)` and `(import …)` forms.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

use regex::Regex;

use super::lex::{Kind, Token};
use super::read::{Reader, is_form, suggest, unexpected};
use super::{Error, ErrorKind, Query, Schema};
use crate::lint::{RuleId, Severity};

/// How often a module may expand defines before it is taken as recursive.
const MAX_EXPANSIONS: usize = 1000;

/// A named pattern with parameters: `(define (name ?var @capture …) pattern…)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Define {
    name: String,
    params: Vec<String>,
    body: Vec<Token>,
}

impl Define {
    /// The name a pattern calls it by.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its parameters with their sigils, such as `?t` or `@c`.
    #[must_use]
    pub fn params(&self) -> &[String] {
        &self.params
    }
}

/// Defines by name, as a module exports them to its importers.
pub type Defines = BTreeMap<String, Define>;

/// A lint rule a module declares: a query and how to report its rows.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Rule {
    /// The rule id.
    pub id: RuleId,
    /// The severity, warning unless `:severity error`.
    pub severity: Severity,
    /// The message template, with `{@capture}`, `{?var}` and `{$param}`
    /// placeholders.
    pub message: String,
    /// The captures a finding points at, the first one bound winning.
    pub at: Vec<String>,
    /// The captures of related locations.
    pub related: Vec<String>,
    /// The variable whose text narrows the finding's position.
    pub focus: Option<String>,
    /// The value capture holding the finding's score.
    pub score: Option<String>,
    /// The scoring method's name.
    pub method: Option<String>,
    /// The two captures whose shared phrases the finding reports.
    pub shared: Option<(String, String)>,
    /// The query.
    pub query: Query,
    /// The byte offset of the rule in its module.
    pub offset: usize,
}

impl Rule {
    /// The params the rule reads: its query's, plus `{$param}`s of its
    /// message, true where the query compares the param as a number.
    #[must_use]
    pub fn params(&self) -> BTreeMap<String, bool> {
        let mut params = self.query.params().clone();
        for placeholder in placeholders(&self.message) {
            if placeholder.sigil == Sigil::Param {
                params.entry(placeholder.name.to_owned()).or_insert(false);
            }
        }
        params
    }
}

/// A module's rules, its defines and the modules it imports.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Module {
    /// Rules in source order.
    pub rules: Vec<Rule>,
    /// The module's own defines, without the imported ones.
    pub defines: Defines,
    /// Imported module names in source order.
    pub imports: Vec<String>,
}

impl Module {
    /// Reads the module `source` against `schema`; `import` returns the
    /// defines of a module an `(import name)` form names.
    ///
    /// # Errors
    /// Returns [`Error`] when the source is not a valid module, a query does
    /// not read, or `import` fails, at the offset of the failing form.
    pub fn read<E: fmt::Display>(
        source: &str,
        schema: &Schema,
        mut import: impl FnMut(&str) -> Result<Defines, E>,
    ) -> Result<Self, Error> {
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
                return Err(Error::syntax(
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
                    let defines =
                        import(&name.text).map_err(|e| Error::syntax(name.pos, e.to_string()))?;
                    for (key, define) in defines {
                        if reader.defines.contains_key(&key) {
                            return Err(Error::syntax(
                                name.pos,
                                format!(
                                    "(import {}) defines {key}, which is already defined here",
                                    name.text
                                ),
                            )
                            .with_hint("rename one of the defines"));
                        }
                        reader.defines.insert(key, define);
                    }
                    module.imports.push(name.text);
                }
                "define" => {
                    let define = reader.define(&open)?;
                    reader.defines.insert(define.name.clone(), define.clone());
                    module.defines.insert(define.name.clone(), define);
                }
                "rule" => module.rules.push(reader.rule(source, &open)?),
                other => {
                    return Err(Error::syntax(
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

impl Reader<'_> {
    fn define(&mut self, open: &Token) -> Result<Define, Error> {
        let signature = self.next();
        if signature.kind != Kind::Open {
            return Err(unexpected(
                &signature,
                "a signature, as in (define (titled ?t) ...)",
            ));
        }
        let name = self.next();
        if name.kind != Kind::Symbol || is_form(&name) || self.schema.has_type(&name.text) {
            return Err(
                Error::syntax(name.pos, format!("{:?} cannot name a pattern", name.text))
                    .with_hint("pick a name that is not a type or form"),
            );
        }
        let mut params = Vec::new();
        while matches!(self.peek().kind, Kind::Var | Kind::Capture) {
            params.push(self.next().sigil());
        }
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(
                &close,
                "?variables, @captures and ) closing the signature",
            ));
        }
        let start = self.at;
        while self.peek().kind != Kind::Close {
            self.skip()?;
        }
        let body = self.tokens[start..self.at].to_vec();
        if body.is_empty() {
            return Err(Error::syntax(
                open.pos,
                format!("(define ({} ...)) has no pattern", name.text),
            ));
        }
        if let Some(t) = body
            .iter()
            .find(|t| t.kind == Kind::Capture && !params.contains(&t.sigil()))
        {
            return Err(Error::new(
                ErrorKind::Unbound,
                t.pos,
                format!(
                    "@{} in (define ({} ...)) is not a parameter",
                    t.text, name.text
                ),
            )
            .with_hint(format!(
                "add @{} to the signature so callers name the capture",
                t.text
            )));
        }
        self.next();
        Ok(Define {
            name: name.text,
            params,
            body,
        })
    }

    /// Skips one bracketed pattern with its quantifier and capture.
    fn skip(&mut self) -> Result<(), Error> {
        let open = self.next();
        if !matches!(open.kind, Kind::Open | Kind::OpenAlt) {
            return Err(unexpected(&open, "a pattern"));
        }
        let mut depth = 1;
        while depth > 0 {
            match self.next().kind {
                Kind::Open | Kind::OpenAlt => depth += 1,
                Kind::Close | Kind::CloseAlt => depth -= 1,
                Kind::Eof => {
                    return Err(
                        Error::syntax(open.pos, "unclosed (").with_hint("add the matching )")
                    );
                }
                _ => {}
            }
        }
        while matches!(self.peek().kind, Kind::Quant | Kind::Capture) {
            self.next();
        }
        Ok(())
    }

    /// Replaces the call of `define` opened by `open`, whose head the reader
    /// has just passed, with the define's body.
    pub(super) fn expand(&mut self, define: &Define, open: &Token) -> Result<(), Error> {
        self.expanded += 1;
        if self.expanded > MAX_EXPANSIONS {
            return Err(Error::syntax(
                open.pos,
                format!(
                    "({} ...) expands without end; a define may not use itself",
                    define.name
                ),
            ));
        }
        let start = self.at - 2;
        let mut args: Vec<Vec<Token>> = Vec::new();
        while self.peek().kind != Kind::Close {
            if self.peek().kind == Kind::Eof {
                return Err(Error::syntax(open.pos, "unclosed (").with_hint("add the matching )"));
            }
            let from = self.at;
            if matches!(self.peek().kind, Kind::Open | Kind::OpenAlt) {
                self.skip()?;
            } else {
                self.next();
            }
            args.push(self.tokens[from..self.at].to_vec());
        }
        let end = self.at;
        if args.len() != define.params.len() {
            return Err(Error::syntax(
                open.pos,
                format!(
                    "({} ...) takes {} arguments, got {}",
                    define.name,
                    define.params.len(),
                    args.len()
                ),
            ));
        }
        let suffix = format!("_{}", self.expanded);
        let mut body = Vec::with_capacity(define.body.len());
        for token in &define.body {
            let mut token = token.clone();
            if token.kind == Kind::Regex {
                bind_groups(&mut token, define, &args, &suffix, open)?;
            }
            if matches!(token.kind, Kind::Var | Kind::Capture) {
                if let Some(i) = define.params.iter().position(|p| *p == token.sigil()) {
                    body.extend(args[i].iter().cloned());
                    continue;
                }
                token.text.push_str(&suffix);
            }
            token.pos = open.pos;
            body.push(token);
        }
        self.tokens.splice(start..=end, body);
        self.at = start;
        Ok(())
    }

    fn rule(&mut self, source: &str, open: &Token) -> Result<Rule, Error> {
        let id = self.next();
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
                Error::syntax(
                    id.pos,
                    "a rule needs an id of lowercase letters, digits and hyphens",
                )
                .with_hint("e.g. (rule large-file ...)")
            })?;
        let mut head = Head::default();
        while self.peek().kind == Kind::Key {
            self.rule_key(&mut head)?;
        }
        let message = head
            .message
            .take()
            .filter(|m| !m.is_empty())
            .ok_or_else(|| Error::syntax(open.pos, format!("rule {} needs a :message", id.text)))?;
        self.printed = placeholders(&message)
            .into_iter()
            .filter(|p| p.sigil == Sigil::Var)
            .map(|p| p.name.to_owned())
            .collect();
        let start = self.peek().pos;
        let mut query = self.query(Kind::Close)?;
        let close = self.next();
        source
            .get(start..close.pos.max(start))
            .unwrap_or_default()
            .trim()
            .clone_into(&mut query.source);
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
            offset: open.pos,
        })
    }

    /// Reads one `:key value` of a rule into `head`.
    fn rule_key(&mut self, head: &mut Head) -> Result<(), Error> {
        let key = self.next();
        let value = self.next();
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
                let invalid = |_| Error::syntax(value.pos, ":severity is warning or error");
                head.severity = Some(severity.map_err(invalid)?);
            }
            "message" => head.message = Some(expect(&[Kind::String], "a \"message\"")?),
            "at" | "related" => {
                let want = format!("a @capture for :{}", key.text);
                let mut captures = vec![expect(&[Kind::Capture], &want)?];
                while self.peek().kind == Kind::Capture {
                    captures.push(self.next().text);
                }
                if key.text == "related" {
                    head.related = captures;
                } else {
                    head.at = captures;
                }
            }
            "focus" => head.focus = Some(expect(&[Kind::Var], "a ?variable for :focus")?),
            "score" => {
                let want = "a @capture for :score, as in :score @score";
                head.score = Some(expect(&[Kind::Capture], want)?);
            }
            "method" => {
                let want = "a method name for :method";
                head.method = Some(expect(&[Kind::Symbol, Kind::String], want)?);
            }
            "shared" => {
                let second = self.next();
                if value.kind != Kind::Capture || second.kind != Kind::Capture {
                    return Err(unexpected(
                        &value,
                        "two @captures for :shared, as in :shared @copy @original",
                    ));
                }
                head.shared = Some((value.text, second.text));
            }
            other => {
                const KEYS: [&str; 8] = [
                    "severity", "message", "at", "related", "focus", "score", "method", "shared",
                ];
                return Err(
                    Error::syntax(key.pos, format!("a rule has no key :{other}"))
                        .with_hint(suggest(other, &KEYS)),
                );
            }
        }
        Ok(())
    }
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
    /// Checks that the captures the rule names are captured by its query.
    fn check(&self, query: &Query, message: &str, id: &str, pos: usize) -> Result<(), Error> {
        let unbound = |message: String| Error::new(ErrorKind::Unbound, pos, message);
        if let Some(score) = &self.score
            && !query.values.contains(score)
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
            if !query.captures.contains(capture) {
                return Err(unbound(format!(
                    "rule {id} points at @{capture}, which its query does not capture"
                )));
            }
        }
        for placeholder in placeholders(message) {
            if placeholder.sigil == Sigil::Capture
                && !query.captures.iter().any(|c| c == placeholder.name)
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

/// Maps each named group of the define body's regexp `token` to the variable
/// or value the call passes for it, or to a variable local to this expansion.
fn bind_groups(
    token: &mut Token,
    define: &Define,
    args: &[Vec<Token>],
    suffix: &str,
    open: &Token,
) -> Result<(), Error> {
    let Ok(regex) = Regex::new(&token.text) else {
        return Ok(());
    };
    for name in regex.capture_names().flatten() {
        let mut bound = format!("?{name}{suffix}");
        if let Some(i) = define.params.iter().position(|p| *p == format!("?{name}")) {
            bound = match args[i].as_slice() {
                [arg] if arg.kind == Kind::Var => format!("?{}", arg.text),
                [arg] if matches!(arg.kind, Kind::String | Kind::Symbol | Kind::Int) => {
                    format!("={}", arg.text)
                }
                _ => {
                    return Err(Error::syntax(
                        open.pos,
                        format!(
                            "({} ...) binds ?{name} from a regexp group, so it takes a ?variable or a value there",
                            define.name
                        ),
                    ));
                }
            };
        }
        token.groups.insert(name.to_owned(), bound);
    }
    Ok(())
}

/// What a message placeholder reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sigil {
    /// `{@capture}`, or `{@capture.key}`.
    Capture,
    /// `{?var}`.
    Var,
    /// `{$param}`.
    Param,
}

/// A `{@capture.key}`, `{?var}` or `{$param}` placeholder of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder<'a> {
    /// What it reads.
    pub sigil: Sigil,
    /// The capture, variable or param name.
    pub name: &'a str,
    /// The key after the dot, such as `line` in `{@x.line}`.
    pub key: Option<&'a str>,
    /// The byte span of the whole placeholder, braces included.
    pub span: Range<usize>,
}

/// The placeholders of `message`, in order.
#[must_use]
pub fn placeholders(message: &str) -> Vec<Placeholder<'_>> {
    let bytes = message.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(offset) = message[i..].find('{') {
        let start = i + offset;
        match placeholder(message, start) {
            Some(placeholder) => {
                i = placeholder.span.end;
                found.push(placeholder);
            }
            None => i = start + 1,
        }
        if i >= bytes.len() {
            break;
        }
    }
    found
}

fn placeholder(message: &str, start: usize) -> Option<Placeholder<'_>> {
    let bytes = message.as_bytes();
    let sigil = match bytes.get(start + 1)? {
        b'@' => Sigil::Capture,
        b'?' => Sigil::Var,
        b'$' => Sigil::Param,
        _ => return None,
    };
    let name_start = start + 2;
    let name_end = run(bytes, name_start, |c, first| {
        c.is_ascii_alphabetic() || c == b'_' || !first && (c.is_ascii_digit() || c == b'-')
    });
    if name_end == name_start {
        return None;
    }
    let mut key = None;
    let mut end = name_end;
    if bytes.get(end) == Some(&b'.') {
        let key_end = run(bytes, end + 1, |c, first| {
            c.is_ascii_alphanumeric() || c == b'_' || !first && (c == b'.' || c == b'-')
        });
        if key_end == end + 1 {
            return None;
        }
        key = Some(&message[end + 1..key_end]);
        end = key_end;
    }
    (bytes.get(end) == Some(&b'}')).then(|| Placeholder {
        sigil,
        name: &message[name_start..name_end],
        key,
        span: start..end + 1,
    })
}

fn run(bytes: &[u8], start: usize, accept: impl Fn(u8, bool) -> bool) -> usize {
    let mut end = start;
    while end < bytes.len() && accept(bytes[end], end == start) {
        end += 1;
    }
    end
}
