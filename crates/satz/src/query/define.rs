//! Defines: named patterns with parameters, `(define (name ?var @capture …)
//! pattern…)`, and their expansion where a pattern calls them.

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

use super::lex::{Kind, Token};
use super::read::{Reader, is_form, unexpected};
use super::{ReadError, ReadErrorKind};

/// How often a source may expand defines before it is taken as recursive.
const MAX_EXPANSIONS: usize = 1000;

/// A named pattern with parameters: `(define (name ?var @capture …) pattern…)`,
/// one body per clause.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Define {
    pub(super) name: String,
    pub(super) params: Vec<String>,
    pub(super) bodies: Vec<Vec<Token>>,
}

/// Defines by name, as a module exports them to its importers.
pub(crate) type Defines = BTreeMap<String, Define>;

impl Reader<'_> {
    /// Reads the `(define …)` form opened by `open`, whose head the reader
    /// has just passed, and makes it callable by what follows.
    pub(crate) fn define(&mut self, open: &Token) -> Result<(String, Define), ReadError> {
        let signature = self.next();
        if signature.kind != Kind::Open {
            return Err(unexpected(
                &signature,
                "a signature, as in (define (titled ?t) ...)",
            ));
        }
        let name = self.next();
        if name.kind != Kind::Symbol || is_form(&name) || self.schema.has_type(&name.text) {
            return Err(ReadError::syntax(
                name.pos,
                format!("{:?} cannot name a pattern", name.text),
            )
            .with_hint("pick a name that is not a type or form"));
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
            return Err(ReadError::syntax(
                open.pos,
                format!("(define ({} ...)) has no pattern", name.text),
            ));
        }
        if let Some(t) = body
            .iter()
            .find(|t| t.kind == Kind::Capture && !params.contains(&t.sigil()))
        {
            return Err(ReadError::new(
                ReadErrorKind::Unbound,
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
        let define = match self.defines.get_mut(&name.text) {
            Some(known) if known.params == params => {
                known.bodies.push(body);
                known.clone()
            }
            Some(_) => {
                return Err(ReadError::syntax(
                    name.pos,
                    format!("(define ({} ...)) again with other parameters", name.text),
                )
                .with_hint("give every clause of a define the same signature"));
            }
            None => {
                let define = Define {
                    name: name.text.clone(),
                    params,
                    bodies: vec![body],
                };
                self.defines.insert(name.text.clone(), define.clone());
                define
            }
        };
        Ok((name.text, define))
    }

    /// Whether the define `name` is a predicate: it has several clauses, or
    /// calls itself through its bodies.
    pub(super) fn is_predicate(&self, name: &str) -> bool {
        let Some(define) = self.defines.get(name) else {
            return false;
        };
        if define.bodies.len() > 1 {
            return true;
        }
        let mut seen = BTreeSet::new();
        let mut stack: Vec<&str> = self.calls(define).collect();
        while let Some(callee) = stack.pop() {
            if callee == name {
                return true;
            }
            if seen.insert(callee)
                && let Some(next) = self.defines.get(callee)
            {
                stack.extend(self.calls(next));
            }
        }
        false
    }

    /// The defines the bodies of `define` call.
    fn calls<'d>(&'d self, define: &'d Define) -> impl Iterator<Item = &'d str> {
        define.bodies.iter().flat_map(move |body| {
            body.windows(2).filter_map(move |pair| {
                (pair[0].kind == Kind::Open
                    && pair[1].kind == Kind::Symbol
                    && self.defines.contains_key(&pair[1].text))
                .then_some(pair[1].text.as_str())
            })
        })
    }

    /// Makes callable the `defines` that the `(import …)` naming `name`
    /// brings.
    pub(crate) fn import(&mut self, name: &Token, defines: Defines) -> Result<(), ReadError> {
        for (key, define) in defines {
            if self.defines.contains_key(&key) {
                return Err(ReadError::syntax(
                    name.pos,
                    format!(
                        "(import {}) defines {key}, which is already defined here",
                        name.text
                    ),
                )
                .with_hint("rename one of the defines"));
            }
            self.defines.insert(key, define);
        }
        Ok(())
    }

    /// Skips one bracketed pattern with its quantifier and capture.
    fn skip(&mut self) -> Result<(), ReadError> {
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
                        ReadError::syntax(open.pos, "unclosed (").with_hint("add the matching )")
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
    pub(super) fn expand(&mut self, define: &Define, open: &Token) -> Result<(), ReadError> {
        self.expanded += 1;
        if self.expanded > MAX_EXPANSIONS {
            return Err(ReadError::syntax(
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
                return Err(
                    ReadError::syntax(open.pos, "unclosed (").with_hint("add the matching )")
                );
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
            return Err(ReadError::syntax(
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
        let mut body = Vec::with_capacity(define.bodies[0].len());
        for token in &define.bodies[0] {
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
}

/// Maps each named group of the define body's regexp `token` to the variable
/// or value the call passes for it, or to a variable local to this expansion.
fn bind_groups(
    token: &mut Token,
    define: &Define,
    args: &[Vec<Token>],
    suffix: &str,
    open: &Token,
) -> Result<(), ReadError> {
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
                    return Err(ReadError::syntax(
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
