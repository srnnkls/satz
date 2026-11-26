//! Predicates: defines with several clauses or calling themselves, called
//! recursive defines in queries. A call matches the predicate's facts,
//! which its clauses derive from the tree and from each other until no
//! clause adds one.

use std::collections::{BTreeMap, BTreeSet};

use super::lex::{Kind, Token};
use super::matcher::MAX_BINDINGS;
use super::pattern::{Check, Pattern, Test};
use super::read::{Reader, attr, unexpected};
use super::{Environment, Query, ReadError, RunError, view};
use crate::tree::Tree;

/// The facts of each predicate, by name: one value per parameter.
pub(super) type Facts = BTreeMap<String, BTreeSet<Vec<String>>>;

/// A predicate and the clauses that derive its facts.
#[derive(Debug, Clone)]
pub(crate) struct Predicate {
    pub(super) name: String,
    /// The parameters, without the `?`.
    params: Vec<String>,
    clauses: Vec<Query>,
}

/// The node type of a predicate's facts.
pub(super) fn fact_kind(name: &str) -> String {
    format!("define:{name}")
}

impl Reader<'_> {
    /// Reads the call of the predicate `name` opened by `open`, whose head
    /// the reader has just passed, as a pattern matching its facts.
    pub(super) fn predicate_call(
        &mut self,
        name: &str,
        open: &Token,
    ) -> Result<Pattern, ReadError> {
        self.register(name, open)?;
        let arity = self.defines[name].params.len();
        let mut pattern = Pattern::new(self.next_id, open.pos);
        self.next_id += 1;
        pattern.kind = Some(fact_kind(name));
        loop {
            let arg = self.next();
            let key = pattern.attrs.len().to_string();
            let check = match arg.kind {
                Kind::Close => break,
                Kind::Var => Check::Bind(arg.text.clone()),
                Kind::String | Kind::Symbol | Kind::Int | Kind::Float => Check::Test {
                    test: Test::Equals(arg.text.clone()),
                    negate: false,
                },
                Kind::Eof => {
                    return Err(
                        ReadError::syntax(open.pos, "unclosed (").with_hint("add the matching )")
                    );
                }
                _ => return Err(unexpected(&arg, "a ?variable or a value")),
            };
            pattern.attrs.push(attr(key, arg.pos, check));
        }
        if pattern.attrs.len() != arity {
            return Err(ReadError::syntax(
                open.pos,
                format!(
                    "({name} ...) takes {arity} arguments, got {}",
                    pattern.attrs.len()
                ),
            ));
        }
        if matches!(self.peek().kind, Kind::Quant | Kind::Capture) {
            return Err(ReadError::syntax(
                self.peek().pos,
                format!(
                    "({name} ...) is a recursive define of values, so it takes no quantifier or capture"
                ),
            )
            .with_hint("capture nodes in the patterns that bind its ?variables"));
        }
        Ok(pattern)
    }

    /// Reads the clauses of the predicate `name` once per query, and those
    /// of the predicates they call.
    fn register(&mut self, name: &str, open: &Token) -> Result<(), ReadError> {
        if self.predicates.contains_key(name) {
            return Ok(());
        }
        let define = self.defines[name].clone();
        if let Some(capture) = define.params.iter().find(|p| p.starts_with('@')) {
            return Err(ReadError::syntax(
                open.pos,
                format!(
                    "({name} ...) is a recursive define, so it takes ?variables, not {capture}"
                ),
            )
            .with_hint("capture nodes where the define is called"));
        }
        let params: Vec<String> = define
            .params
            .iter()
            .map(|p| p.trim_start_matches('?').to_owned())
            .collect();
        self.predicates.insert(
            name.to_owned(),
            Predicate {
                name: name.to_owned(),
                params: params.clone(),
                clauses: Vec::new(),
            },
        );
        let mut clauses = Vec::with_capacity(define.bodies.len());
        for body in &define.bodies {
            let mut tokens = body.clone();
            tokens.push(Token::eof(open.pos));
            let mut clause = Reader::from_tokens(tokens, self.schema);
            clause.defines = self.defines.clone();
            clause.predicates = std::mem::take(&mut self.predicates);
            clause.printed.clone_from(&params);
            clause.clause = true;
            let read = clause.query(Kind::Eof);
            self.predicates = std::mem::take(&mut clause.predicates);
            let read = read?;
            self.params.extend(read.params.clone());
            self.semantic |= read.semantic;
            clauses.push(read);
        }
        if let Some(predicate) = self.predicates.get_mut(name) {
            predicate.clauses = clauses;
        }
        Ok(())
    }
}

/// The facts of `predicates` over `tree`: every clause runs over the facts
/// so far until none adds one.
pub(super) fn facts(
    predicates: &[Predicate],
    tree: &Tree,
    environment: &Environment,
) -> Result<Facts, RunError> {
    let mut facts: Facts = predicates
        .iter()
        .map(|predicate| (predicate.name.clone(), BTreeSet::new()))
        .collect();
    let mut total = 0;
    loop {
        let data = view::data_tree(&environment.data, &facts);
        let mut grown = facts.clone();
        for predicate in predicates {
            for clause in &predicate.clauses {
                for row in clause.run_over(tree, environment, data.as_ref())? {
                    let tuple: Option<Vec<String>> = predicate
                        .params
                        .iter()
                        .map(|param| row.vars.get(param).cloned())
                        .collect();
                    let Some(tuple) = tuple else { continue };
                    if grown
                        .entry(predicate.name.clone())
                        .or_default()
                        .insert(tuple)
                    {
                        total += 1;
                    }
                }
            }
        }
        if total > MAX_BINDINGS {
            return Err(RunError::TooMany);
        }
        if grown == facts {
            return Ok(facts);
        }
        facts = grown;
    }
}
