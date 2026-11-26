//! The reader: tokens into a checked [`Query`].

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

use super::EditForm;
use super::define::{Define, Defines};
use super::group::Group;
use super::lex::{Kind, Token, lex};
use super::pattern::{
    Attr, Chain, Check, Comparison, Expr, Link, Measure, Negation, Op, Operand, Pattern, Quant,
    Relate, Relation, RelationKind, Similarity, Submatch, Test,
};
use super::predicate::Predicate;
use super::sort::SortKey;
use super::{ParamKind, Query, ReadError, ReadErrorKind, Schema, TEXT_KEYS};
use crate::edit::Action;
use crate::template::{FILTERS, Sigil, placeholders};

/// The quantifiers a nested pattern takes.
const NESTED: &str = "?*+";

/// Resolves the module an `(import name)` names to its defines.
pub(crate) type Importer<'s> = Box<dyn FnMut(&str) -> Result<Defines, String> + 's>;

pub(crate) struct Reader<'s> {
    pub(super) schema: &'s Schema,
    pub(super) importer: Option<Importer<'s>>,
    pub(super) tokens: Vec<Token>,
    pub(super) at: usize,
    pub(super) captures: Vec<String>,
    pub(super) values: BTreeSet<String>,
    pub(super) numbers: BTreeSet<String>,
    pub(super) semantic: bool,
    pub(super) params: BTreeMap<String, ParamKind>,
    pub(super) defines: BTreeMap<String, Define>,
    pub(super) expanded: usize,
    pub(super) sorts: Vec<SortKey>,
    pub(crate) printed: Vec<String>,
    pub(super) next_id: usize,
    /// The recursive defines the query calls, read once each.
    pub(super) predicates: BTreeMap<String, Predicate>,
    /// Whether the query is a recursive define's clause, whose parameters
    /// print.
    pub(super) clause: bool,
}

/// Whether `token` opens a form that is not a pattern: `not` or a relation.
pub(super) fn is_form(token: &Token) -> bool {
    token.kind == Kind::Symbol
        && (token.text == "not" || RelationKind::from_symbol(&token.text).is_some())
}

impl<'s> Reader<'s> {
    pub(crate) fn new(source: &str, schema: &'s Schema) -> Result<Self, ReadError> {
        Ok(Self::from_tokens(lex(source)?, schema))
    }

    /// A reader of `tokens`, which end in [`Kind::Eof`].
    pub(super) fn from_tokens(tokens: Vec<Token>, schema: &'s Schema) -> Self {
        Self {
            schema,
            importer: None,
            tokens,
            at: 0,
            captures: Vec::new(),
            values: BTreeSet::new(),
            numbers: BTreeSet::new(),
            semantic: false,
            params: BTreeMap::new(),
            defines: BTreeMap::new(),
            expanded: 0,
            sorts: Vec::new(),
            printed: Vec::new(),
            next_id: 0,
            predicates: BTreeMap::new(),
            clause: false,
        }
    }

    pub(crate) fn peek(&self) -> &Token {
        self.peek_at(0)
    }

    /// The token `n` ahead, or the end of the query past it.
    pub(super) fn peek_at(&self, n: usize) -> &Token {
        self.tokens
            .get(self.at + n)
            .or_else(|| self.tokens.last())
            .unwrap_or(&self.tokens[0])
    }

    pub(crate) fn next(&mut self) -> Token {
        let token = self.peek().clone();
        if token.kind != Kind::Eof {
            self.at += 1;
        }
        token
    }

    /// Reads the patterns up to `end` into a checked query.
    pub(crate) fn query(&mut self, end: Kind) -> Result<Query, ReadError> {
        self.captures.clear();
        self.values.clear();
        self.numbers.clear();
        self.semantic = false;
        self.params.clear();
        let start = self.peek().pos;
        let mut members = Vec::new();
        let mut absent = Vec::new();
        let mut group = None;
        let mut edits = Vec::new();
        while self.peek().kind != end {
            if self.peek().kind == Kind::Eof {
                return Err(
                    ReadError::syntax(start, "unclosed (rule").with_hint("add the matching )")
                );
            }
            let opens = self.peek().kind == Kind::Open;
            let head = self.peek_at(1).clone();
            if opens && head.kind == Kind::Symbol && self.is_predicate(&head.text) {
                let open = self.next();
                self.next();
                members.push(self.predicate_call(&head.text, &open)?);
                continue;
            }
            if opens
                && head.kind == Kind::Symbol
                && let Some(define) = self.defines.get(&head.text).cloned()
            {
                let open = self.next();
                self.next();
                self.expand(&define, &open)?;
                continue;
            }
            if opens && (head.is_symbol("define") || head.is_symbol("import")) {
                self.leading_form(&head, members.is_empty() && absent.is_empty())?;
                continue;
            }
            if opens && self.trailing(&head, &mut group, &mut edits)? {
                continue;
            }
            if group.is_some() || !edits.is_empty() {
                let form = if group.is_some() { "group" } else { "edits" };
                return Err(ReadError::syntax(
                    self.peek().pos,
                    format!("({form} ...) follows the patterns of a query"),
                )
                .with_hint(format!("move the pattern before ({form} ...)")));
            }
            if opens && head.is_symbol("not") {
                absent.push(self.absent()?);
                continue;
            }
            members.push(self.pattern("?")?);
        }
        if members.is_empty() {
            return Err(
                ReadError::syntax(start, "a query needs a pattern outside (not ...)")
                    .with_hint(r#"try (section :id "NAME" (heading) @h)"#),
            );
        }
        let mut captures = std::mem::take(&mut self.captures);
        if captures.is_empty() {
            if (members.len() > 1 || !absent.is_empty()) && !self.clause {
                return Err(ReadError::syntax(
                    start,
                    "a query of several patterns prints its captures; name one with @",
                )
                .with_hint("e.g. (link :url ?u) @l (code :text ?u) @c"));
            }
            captures.push(String::new());
        }
        self.check_edits(&edits, &captures)?;
        let mut query = Query {
            source: String::new(),
            members,
            absent,
            captures,
            values: std::mem::take(&mut self.values),
            numbers: std::mem::take(&mut self.numbers),
            semantic: self.semantic,
            group,
            params: std::mem::take(&mut self.params),
            sorts: self.sorts.clone(),
            printed: self.printed.clone(),
            unit: self.schema.unit().cloned(),
            edits,
            predicates: if self.clause {
                Vec::new()
            } else {
                std::mem::take(&mut self.predicates).into_values().collect()
            },
        };
        query.check()?;
        Ok(query)
    }

    /// Reads the `(group …)` or edit form whose head is `head` into `group`
    /// or `edits`, returning whether there was one.
    fn trailing(
        &mut self,
        head: &Token,
        group: &mut Option<Group>,
        edits: &mut Vec<EditForm>,
    ) -> Result<bool, ReadError> {
        if head.kind != Kind::Symbol {
            return Ok(false);
        }
        if head.text == "group" {
            let open = self.next();
            self.next();
            if group.is_some() || !edits.is_empty() {
                return Err(ReadError::syntax(
                    open.pos,
                    "a query takes one (group ...) and no edits with it",
                )
                .with_hint("list every key and aggregate in one (group ...)"));
            }
            *group = Some(self.group(&open)?);
            return Ok(true);
        }
        let Some(action) = Action::from_symbol(&head.text) else {
            return Ok(false);
        };
        let open = self.next();
        self.next();
        if group.is_some() {
            return Err(
                ReadError::syntax(open.pos, "a grouped query makes no edits")
                    .with_hint("drop the (group ...) or the edit"),
            );
        }
        let capture = self.next();
        if capture.kind != Kind::Capture {
            return Err(unexpected(
                &capture,
                &format!("a @capture, as in ({} @c ...)", head.text),
            ));
        }
        let mut template = String::new();
        if action != Action::Delete {
            let text = self.next();
            if text.kind != Kind::String {
                return Err(unexpected(
                    &text,
                    &format!(r#"a "template", as in ({} @c "{{@c.text}}")"#, head.text),
                ));
            }
            template = text.text;
        }
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(&close, &format!(") closing ({}", head.text)));
        }
        self.printed.extend(
            placeholders(&template)
                .into_iter()
                .filter(|p| p.sigil == Sigil::Var)
                .map(|p| p.name.to_owned()),
        );
        edits.push(EditForm {
            action,
            capture: capture.text,
            template,
            pos: capture.pos,
        });
        Ok(true)
    }

    /// Fails when an edit names a capture that holds no nodes, or its
    /// template one the query lacks.
    fn check_edits(&self, edits: &[EditForm], captures: &[String]) -> Result<(), ReadError> {
        for edit in edits {
            if !captures.contains(&edit.capture) || self.values.contains(&edit.capture) {
                return Err(ReadError::new(
                    ReadErrorKind::Unbound,
                    edit.pos,
                    format!(
                        "@{} captures no node, so there is nothing to edit",
                        edit.capture
                    ),
                )
                .with_hint("edit a node capture, as in (heading) @h (replace @h \"...\")"));
            }
            if let Some((name, _)) = placeholders(&edit.template)
                .into_iter()
                .flat_map(|p| p.filters)
                .find(|(name, _)| !FILTERS.contains(name))
            {
                return Err(ReadError::syntax(
                    edit.pos,
                    format!("the template filters with {name:?}, which is no filter"),
                )
                .with_hint(format!("filters: {}", FILTERS.join(" "))));
            }
            if let Some(unknown) = placeholders(&edit.template)
                .into_iter()
                .find(|p| p.sigil == Sigil::Capture && !captures.iter().any(|c| c == p.name))
            {
                return Err(ReadError::new(
                    ReadErrorKind::Unbound,
                    edit.pos,
                    format!(
                        "the template names @{}, which the query does not capture",
                        unknown.name
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Reads a top-level `(not …)`, whose `(` the reader is at.
    fn absent(&mut self) -> Result<Pattern, ReadError> {
        self.at += 2;
        let inner = self.peek_at(1).clone();
        let pattern = if self.peek().kind == Kind::Open
            && inner.kind == Kind::Symbol
            && self.is_predicate(&inner.text)
        {
            if self.clause {
                return Err(ReadError::syntax(
                    inner.pos,
                    format!(
                        "({} ...) is negated inside a define, where its facts are still growing",
                        inner.text
                    ),
                )
                .with_hint("negate it in the query that calls the define"));
            }
            let open = self.next();
            self.next();
            self.predicate_call(&inner.text, &open)?
        } else {
            self.pattern("")?
        };
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(&close, ") closing (not"));
        }
        Ok(pattern)
    }

    /// Reads the `(define …)` or `(import …)` form whose head is `head`,
    /// which must come `first`, before every pattern.
    fn leading_form(&mut self, head: &Token, first: bool) -> Result<(), ReadError> {
        let open = self.next();
        self.next();
        if !first {
            return Err(ReadError::syntax(
                open.pos,
                format!("({} ...) comes before the patterns of a query", head.text),
            )
            .with_hint(format!("move the {} to the start", head.text)));
        }
        if head.text == "define" {
            self.define(&open).map(drop)
        } else {
            self.import_named()
        }
    }

    /// Reads `name)` of an `(import name)` and makes its defines callable.
    fn import_named(&mut self) -> Result<(), ReadError> {
        let name = self.next();
        if name.kind != Kind::Symbol {
            return Err(unexpected(
                &name,
                "a module name, as in (import std/structure)",
            ));
        }
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(&close, ") closing (import"));
        }
        let Some(importer) = self.importer.as_mut() else {
            return Err(ReadError::syntax(
                name.pos,
                "(import ...) needs modules, and this query reads none",
            ));
        };
        let defines = importer(&name.text).map_err(|message| {
            ReadError::syntax(name.pos, message).with_hint("std modules are named std/NAME")
        })?;
        self.import(&name, defines)
    }

    fn pattern(&mut self, quants: &str) -> Result<Pattern, ReadError> {
        let open = self.next();
        let mut pattern = Pattern::new(self.next_id, open.pos);
        self.next_id += 1;
        match open.kind {
            Kind::OpenAlt => {
                while self.peek().kind != Kind::CloseAlt {
                    if self.peek().kind == Kind::Eof {
                        return Err(ReadError::syntax(open.pos, "unclosed [")
                            .with_hint("close the alternation with ]"));
                    }
                    pattern.alts.push(self.pattern(NESTED)?);
                }
                self.next();
                if pattern.alts.is_empty() {
                    return Err(ReadError::syntax(open.pos, "empty alternation")
                        .with_hint("list patterns inside [ ], e.g. [(code) (quote)]"));
                }
            }
            Kind::Open => {
                let head = self.next();
                if head.kind != Kind::Symbol {
                    return Err(ReadError::syntax(head.pos, "expected a type after (")
                        .with_hint(format!("types: {}", self.type_names().join(" "))));
                }
                if self.is_predicate(&head.text) {
                    return Err(ReadError::syntax(
                        head.pos,
                        format!(
                            "({} ...) is a recursive define, so it is called at the top level of a query",
                            head.text
                        ),
                    )
                    .with_hint("call it beside the pattern, joined on a ?variable"));
                }
                if let Some(define) = self.defines.get(&head.text).cloned() {
                    self.expand(&define, &open)?;
                    return self.pattern(quants);
                }
                if head.is_symbol("group") {
                    return Err(ReadError::syntax(
                        head.pos,
                        "(group ...) belongs at the top level of a query, after its patterns",
                    )
                    .with_hint("e.g. (code :lang ?l) @c (group ?l (count @c @n))"));
                }
                if is_form(&head) {
                    return Err(ReadError::syntax(
                        head.pos,
                        format!("({} ...) belongs inside a pattern", head.text),
                    )
                    .with_hint(format!(
                        r#"e.g. (link :url "X" ({} (section) @t))"#,
                        head.text
                    )));
                }
                if !self.schema.has_type(&head.text) {
                    return Err(ReadError::new(
                        ReadErrorKind::UnknownType,
                        head.pos,
                        format!("unknown type {:?}", head.text),
                    )
                    .with_hint(suggest(&head.text, &self.type_names())));
                }
                if head.text != "_" {
                    pattern.kind = Some(head.text.clone());
                }
                self.body(&mut pattern, &head.text, &open)?;
            }
            _ => return Err(unexpected(&open, "a pattern such as (heading)")),
        }
        let quant = self.peek().clone();
        if quant.kind == Kind::Quant {
            if !quants.contains(quant.text.as_str()) {
                let mut hint = "nest it in another pattern, as (section (code)+ @c)".to_owned();
                if quants.contains('?') {
                    hint.push_str(", or use ? for an optional top-level pattern");
                }
                return Err(ReadError::syntax(
                    quant.pos,
                    "quantifiers apply to patterns nested in another pattern",
                )
                .with_hint(hint));
            }
            self.next();
            pattern.quant = Quant::from_symbol(&quant.text);
        }
        let capture = self.peek().clone();
        if capture.kind == Kind::Capture {
            self.next();
            self.name(&mut pattern, &capture)?;
        }
        Ok(pattern)
    }

    fn type_names(&self) -> Vec<&'s str> {
        self.schema
            .types
            .keys()
            .map(String::as_str)
            .filter(|&name| name != "_")
            .collect()
    }

    fn name(&mut self, pattern: &mut Pattern, token: &Token) -> Result<(), ReadError> {
        if self.values.contains(&token.text) {
            return Err(ReadError::syntax(
                token.pos,
                format!(
                    "@{} already names a value, so it cannot name a node",
                    token.text
                ),
            )
            .with_hint("give the node capture another name"));
        }
        pattern.capture = Some(token.text.clone());
        if !self.captures.contains(&token.text) {
            self.captures.push(token.text.clone());
        }
        Ok(())
    }

    /// Registers the value capture `token`.
    pub(super) fn value(&mut self, token: &Token) -> Result<(), ReadError> {
        if self.captures.contains(&token.text) {
            return Err(ReadError::syntax(
                token.pos,
                format!(
                    "@{} is already captured; a value capture takes a name of its own",
                    token.text
                ),
            )
            .with_hint(format!("rename one of the @{} captures", token.text)));
        }
        self.captures.push(token.text.clone());
        self.values.insert(token.text.clone());
        Ok(())
    }

    fn body(&mut self, pattern: &mut Pattern, kind: &str, open: &Token) -> Result<(), ReadError> {
        let mut anchors = Anchors::default();
        loop {
            let token = self.peek().clone();
            match token.kind {
                Kind::Close => {
                    self.next();
                    if anchors.connect {
                        if let Some(chain) = pattern.chains.last_mut() {
                            chain.last_child = true;
                        }
                    } else if anchors.first_child {
                        return Err(ReadError::syntax(
                            token.pos,
                            "anchor . needs a pattern after it",
                        ));
                    }
                    return Ok(());
                }
                Kind::Eof => {
                    return Err(
                        ReadError::syntax(open.pos, "unclosed (").with_hint("add the matching )")
                    );
                }
                Kind::Key => {
                    let mut attr = self.attr(kind)?;
                    let capture = self.peek().clone();
                    if capture.kind == Kind::Capture {
                        self.next();
                        self.value(&capture)?;
                        attr.capture = Some((capture.text, capture.pos));
                    }
                    pattern.attrs.push(attr);
                    anchors.after_pattern = false;
                }
                Kind::Capture => {
                    return Err(ReadError::syntax(
                        token.pos,
                        format!(
                            "@{} captures what precedes it, but nothing it can capture does",
                            token.text
                        ),
                    )
                    .with_hint(
                        "capture a pattern, as in (section (code) @c), or a value, as in (code :lang ?l @l)",
                    ));
                }
                Kind::Anchor => {
                    self.next();
                    if anchors.connect || anchors.first_child {
                        return Err(ReadError::syntax(token.pos, "two anchors in a row"));
                    }
                    anchors.connect = anchors.after_pattern;
                    anchors.first_child = !anchors.after_pattern;
                }
                Kind::Open | Kind::OpenAlt | Kind::Direct => {
                    self.nested(pattern, &token, &mut anchors)?;
                }
                _ => {
                    let found = self.next();
                    return Err(unexpected(&found, "a key, a nested pattern or )"));
                }
            }
        }
    }

    /// Reads the nested pattern, negation or relation at `token` into
    /// `pattern`, chaining siblings by `anchors`.
    fn nested(
        &mut self,
        pattern: &mut Pattern,
        token: &Token,
        anchors: &mut Anchors,
    ) -> Result<(), ReadError> {
        let direct = token.kind == Kind::Direct;
        if direct {
            self.next();
        }
        let form = self.peek_at(1).clone();
        if self.peek().kind == Kind::Open && is_form(&form) {
            if direct || anchors.connect || anchors.first_child {
                return Err(ReadError::syntax(
                    token.pos,
                    format!("({} ...) cannot be anchored or marked direct", form.text),
                ));
            }
            if form.text == "not" {
                pattern.nots.push(self.negation()?);
            } else {
                pattern.relations.push(self.relation()?);
            }
            anchors.after_pattern = false;
            return Ok(());
        }
        let child = self.pattern(NESTED)?;
        if anchors.connect {
            if direct {
                return Err(ReadError::syntax(
                    token.pos,
                    "> marks the first pattern of a sibling chain only",
                ));
            }
            if let Some(chain) = pattern.chains.last_mut() {
                chain.links.push(Link {
                    pattern: child,
                    direct: false,
                });
            }
        } else {
            pattern.chains.push(Chain {
                links: vec![Link {
                    pattern: child,
                    direct,
                }],
                first_child: anchors.first_child,
                last_child: false,
            });
        }
        *anchors = Anchors {
            after_pattern: true,
            ..Anchors::default()
        };
        Ok(())
    }

    fn negation(&mut self) -> Result<Negation, ReadError> {
        self.at += 2;
        let inner = self.peek_at(1);
        let negation = if self.peek().kind == Kind::Open && is_form(inner) && inner.text != "not" {
            Negation::Relation(self.relation()?)
        } else {
            Negation::Pattern(self.pattern(NESTED)?)
        };
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(&close, ") closing (not"));
        }
        Ok(negation)
    }

    fn relation(&mut self) -> Result<Relation, ReadError> {
        self.next();
        let form = self.next();
        if self.schema.unit().is_none() {
            return Err(ReadError::new(
                ReadErrorKind::NoUnit,
                form.pos,
                format!("({} ...) relates units, and the schema has none", form.text),
            )
            .with_hint("register a unit in the schema, or match links with (link :url ...)"));
        }
        let target = self.pattern(NESTED)?;
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(&close, &format!(") closing ({}", form.text)));
        }
        let kind = RelationKind::from_symbol(&form.text)
            .ok_or_else(|| unexpected(&form, "reaches, inbound, to or from"))?;
        Ok(Relation {
            kind,
            target: Box::new(target),
        })
    }

    fn attr(&mut self, kind: &str) -> Result<Attr, ReadError> {
        let key = self.next();
        if !self.schema.has_key(kind, &key.text) {
            let known: Vec<&str> = self
                .schema
                .types
                .get(kind)
                .into_iter()
                .flatten()
                .map(String::as_str)
                .chain(TEXT_KEYS)
                .collect();
            return Err(ReadError::new(
                ReadErrorKind::UnknownKey,
                key.pos,
                format!("{kind} has no key :{}", key.text),
            )
            .with_hint(format!(
                "{}; keys: :{}",
                suggest(&key.text, &known),
                known.join(" :")
            )));
        }
        let value = self.next();
        if value.kind == Kind::Open {
            self.form(key)
        } else {
            self.valued(key, value, false, None)
        }
    }

    /// Reads the `(form …)` value of the attribute `key`.
    fn form(&mut self, key: Token) -> Result<Attr, ReadError> {
        let form = self.next();
        let name = if form.kind == Kind::Direct {
            ">"
        } else {
            form.text.as_str()
        };
        let (value, negate, relate) = match name {
            "not" => (self.next(), true, None),
            ">" | ">=" | "<" | "<=" | "older" => return self.comparison(key, &form, name),
            "+" | "-" | "*" | "/" => {
                let expr = self.apply(&form)?;
                return compared(key, &form, Comparison::Equal, expr);
            }
            "contains" | "covers" | "within" | "after" | "before" => {
                let value = self.next();
                if value.kind != Kind::Var {
                    return Err(unexpected(
                        &value,
                        &format!("a ?variable, as in ({name} ?x)"),
                    ));
                }
                let relate = match name {
                    "contains" => Relate::Contains,
                    "covers" => Relate::Covers,
                    "within" => Relate::Within,
                    "after" => Relate::After,
                    _ => Relate::Before,
                };
                (value, false, Some(relate))
            }
            "near" | "overlap" | "similar" => {
                let value = self.next();
                let similarity = self.similarity(name, &value)?;
                (value, false, Some(Relate::Similarity(similarity)))
            }
            _ => {
                return Err(unexpected(
                    &form,
                    "not, after, before, contains, covers, near, overlap, similar, older, a comparison or arithmetic, as in (not ?x), (near ?t 0.7), (> 500) or (+ ?l 1)",
                ));
            }
        };
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(&close, &format!(") closing ({name}")));
        }
        self.valued(key, value, negate, relate)
    }

    /// Reads `(> n)`, `(>= n)`, `(< n)`, `(<= n)` or `(older days)` after
    /// its `form`.
    fn comparison(&mut self, key: Token, form: &Token, name: &str) -> Result<Attr, ReadError> {
        if key.text == "contains" || key.text == "matches" {
            return Err(ReadError::syntax(
                form.pos,
                format!(":{} takes a string, not a comparison", key.text),
            ));
        }
        let expr = self.expr(name)?;
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(&close, &format!(") closing ({name}")));
        }
        let test = match (Comparison::from_symbol(name), expr) {
            (Some(comparison), Expr::Operand(operand)) => Test::Compare(comparison, operand),
            (None, Expr::Operand(operand)) => Test::Older(operand),
            (Some(comparison), expr) => return compared(key, form, comparison, expr),
            (None, _) => {
                return Err(ReadError::syntax(
                    form.pos,
                    "(older days) takes a number or $param",
                ));
            }
        };
        let negate = false;
        Ok(attr(key.text, 0, Check::Test { test, negate }))
    }

    /// A number, `$param`, `?variable` or `(+ …)`, `(- …)`, `(* …)` or
    /// `(/ …)` over them, inside `(form …)`.
    fn expr(&mut self, form: &str) -> Result<Expr, ReadError> {
        match self.peek().kind {
            Kind::Var => Ok(Expr::Var(self.next().text)),
            Kind::Open => {
                self.next();
                let op = self.next();
                self.apply(&op)
            }
            _ => Ok(Expr::Operand(self.operand(form)?)),
        }
    }

    /// The arguments of the arithmetic `op`, whose `(` and head the reader
    /// has just passed, up to its `)`.
    fn apply(&mut self, op: &Token) -> Result<Expr, ReadError> {
        let Some(kind) = Op::from_symbol(&op.text) else {
            return Err(unexpected(op, "+, -, * or /"));
        };
        let mut args = Vec::new();
        while self.peek().kind != Kind::Close {
            if self.peek().kind == Kind::Eof {
                return Err(ReadError::syntax(op.pos, "unclosed (").with_hint("add the matching )"));
            }
            args.push(self.expr(&op.text)?);
        }
        self.next();
        let least = if kind == Op::Subtract { 1 } else { 2 };
        if args.len() < least {
            return Err(ReadError::syntax(
                op.pos,
                format!("({} ...) takes at least {least} arguments", op.text),
            ));
        }
        Ok(Expr::Apply(kind, args))
    }

    /// The attribute `key` with `value`, negated or related to it.
    fn valued(
        &mut self,
        key: Token,
        value: Token,
        negate: bool,
        relate: Option<Relate>,
    ) -> Result<Attr, ReadError> {
        if value.kind == Kind::Var {
            if key.text == "contains" || key.text == "matches" {
                return Err(ReadError::syntax(
                    value.pos,
                    format!(":{} takes a string, not a variable", key.text),
                )
                .with_hint("bind the text with :text ?x"));
            }
            let var = value.text;
            let check = match (negate, relate) {
                (true, _) => Check::Constrain {
                    var,
                    relate: Relate::Not,
                },
                (false, Some(relate)) => Check::Constrain { var, relate },
                (false, None) => Check::Bind(var),
            };
            return Ok(attr(key.text, value.pos, check));
        }
        let test = match value.kind {
            Kind::Param if key.text == "matches" => {
                return Err(ReadError::syntax(
                    value.pos,
                    ":matches takes a /regexp/ or string, not a $param",
                ));
            }
            Kind::Param => {
                self.params
                    .entry(value.text.clone())
                    .or_insert(ParamKind::Text);
                let test = Test::Param(value.text);
                return Ok(attr(key.text, 0, Check::Test { test, negate }));
            }
            Kind::Regex if key.text == "contains" => {
                return Err(ReadError::syntax(
                    value.pos,
                    ":contains takes a string; match a /regexp/ with :matches",
                ));
            }
            Kind::Regex => return regexp(key.text, &value, &value.text, negate),
            Kind::String | Kind::Symbol | Kind::Int | Kind::Float => {
                literal(self.schema.is_numeric(&key.text), &key.text, &value)?
            }
            Kind::Range if self.schema.is_numeric(&key.text) => Test::Range(value.lo, value.hi),
            Kind::Range => {
                return Err(ReadError::syntax(
                    value.pos,
                    "ranges apply to :level, :words, :lines and :chars only",
                ));
            }
            _ => return Err(unexpected(&value, &format!("a value for :{}", key.text))),
        };
        if key.text == "matches"
            && let Test::Equals(text) = &test
        {
            return regexp(key.text, &value, text, negate);
        }
        Ok(attr(key.text, 0, Check::Test { test, negate }))
    }

    /// The number or `$param` operand of `(form …)`.
    pub(super) fn operand(&mut self, form: &str) -> Result<Operand, ReadError> {
        let operand = self.next();
        match operand.kind {
            Kind::Param => {
                self.params.insert(operand.text.clone(), ParamKind::Number);
                Ok(Operand::Param(operand.text))
            }
            Kind::Int | Kind::Float => Ok(Operand::Number(operand.text.parse().unwrap_or(0.0))),
            _ => Err(unexpected(
                &operand,
                &format!(
                    "a number, $param, ?variable or arithmetic, as in ({form} 500) or ({form} (+ ?l 1))"
                ),
            )),
        }
    }

    fn similarity(&mut self, name: &str, value: &Token) -> Result<Similarity, ReadError> {
        let measure = match name {
            "near" => Measure::Near,
            "overlap" => Measure::Overlap,
            _ => Measure::Similar,
        };
        let example = match measure {
            Measure::Near => "0.7",
            Measure::Overlap => "0.9",
            Measure::Similar => "0.8",
        };
        if value.kind != Kind::Var {
            return Err(unexpected(
                value,
                &format!("a ?variable, as in ({name} ?x {example})"),
            ));
        }
        let limit = self.next();
        let threshold = if limit.kind == Kind::Param {
            self.params.insert(limit.text.clone(), ParamKind::Number);
            Operand::Param(limit.text)
        } else {
            let t =
                limit.text.parse::<f64>().ok().filter(|&t| {
                    matches!(limit.kind, Kind::Int | Kind::Float) && t > 0.0 && t <= 1.0
                });
            let Some(t) = t else {
                return Err(ReadError::syntax(
                    limit.pos,
                    format!("({name} ?x T) takes a threshold above 0 and at most 1"),
                )
                .with_hint(format!("e.g. ({name} ?t 0.7)")));
            };
            Operand::Number(t)
        };
        let mut score = None;
        let next = self.peek().clone();
        match next.kind {
            Kind::Capture => {
                self.next();
                self.value(&next)?;
                self.numbers.insert(next.text.clone());
                score = Some((next.text, next.pos));
            }
            Kind::Var => {
                let t = &next.text;
                return Err(ReadError::syntax(
                    next.pos,
                    format!("a score is output, so it is a capture: @{t}, not ?{t}"),
                )
                .with_hint(format!("e.g. ({name} ?t 0.7 @{t})")));
            }
            _ => {}
        }
        self.semantic |= measure == Measure::Similar;
        Ok(Similarity {
            measure,
            threshold,
            score,
        })
    }
}

/// Whether the last pattern read was a sibling an anchor connects to, and
/// the anchors waiting for the next.
#[derive(Default)]
struct Anchors {
    after_pattern: bool,
    connect: bool,
    first_child: bool,
}

/// The test of a literal value: a number or range for a numeric key, else
/// equality.
fn literal(numeric: bool, key: &str, value: &Token) -> Result<Test, ReadError> {
    if !numeric {
        return Ok(Test::Equals(value.text.clone()));
    }
    let n = value
        .text
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| {
            ReadError::syntax(value.pos, format!(":{key} takes a number or a range"))
                .with_hint(format!(":{key} 2, :{key} 1..3 or :{key} 80.."))
        })?;
    Ok(Test::Range(n, n))
}

/// The attribute `key` compared with `expr`, which reads a variable.
fn compared(
    key: Token,
    form: &Token,
    comparison: Comparison,
    expr: Expr,
) -> Result<Attr, ReadError> {
    if key.text == "contains" || key.text == "matches" {
        return Err(ReadError::syntax(
            form.pos,
            format!(":{} takes a string, not arithmetic", key.text),
        ));
    }
    let Some(var) = expr.vars().first().map(|&var| var.to_owned()) else {
        return Err(
            ReadError::syntax(form.pos, "arithmetic without a ?variable is a constant")
                .with_hint("write the number, or use a ?variable, as in (+ ?l 1)"),
        );
    };
    let relate = Relate::Compare(comparison, expr);
    Ok(attr(key.text, form.pos, Check::Constrain { var, relate }))
}

pub(super) fn attr(key: String, pos: usize, check: Check) -> Attr {
    Attr {
        key,
        pos,
        check,
        capture: None,
    }
}

/// The attribute matching `pattern`, binding its named groups.
fn regexp(key: String, value: &Token, pattern: &str, negate: bool) -> Result<Attr, ReadError> {
    let regex = Regex::new(pattern)
        .map_err(|e| ReadError::syntax(value.pos, format!("invalid regular expression: {e}")))?;
    let mut groups = Vec::new();
    for name in regex.capture_names().flatten() {
        if negate {
            return Err(ReadError::syntax(
                value.pos,
                format!("(not /regexp/) cannot bind ?{name} from a group"),
            )
            .with_hint("drop the group name, as in (?:...)"));
        }
        let mut group = Submatch {
            name: name.to_owned(),
            var: Some(name.to_owned()),
            value: String::new(),
        };
        if let Some(bound) = value.groups.get(name) {
            group.var = bound.strip_prefix('?').map(str::to_owned);
            if group.var.is_none() {
                bound.trim_start_matches('=').clone_into(&mut group.value);
            }
        }
        groups.push(group);
    }
    let check = if groups.is_empty() {
        Check::Test {
            test: Test::Regex(regex),
            negate,
        }
    } else {
        Check::Groups { regex, groups }
    };
    Ok(attr(key, value.pos, check))
}

pub(crate) fn unexpected(token: &Token, want: &str) -> ReadError {
    let found = if token.kind == Kind::Eof {
        "end of query".to_owned()
    } else {
        format!("{:?}", token.text)
    };
    ReadError::syntax(token.pos, format!("expected {want}, found {found}"))
}

/// A hint naming the candidate closest to `word`.
pub(crate) fn suggest(word: &str, candidates: &[&str]) -> String {
    let mut best = None;
    let mut distance = 3;
    for &candidate in candidates {
        if word.starts_with(candidate) || candidate.starts_with(word) {
            return format!("did you mean {candidate}?");
        }
        let d = levenshtein(word.as_bytes(), candidate.as_bytes());
        if d < distance {
            (best, distance) = (Some(candidate), d);
        }
    }
    match best {
        Some(best) => format!("did you mean {best}?"),
        None => format!("valid: {}", candidates.join(" ")),
    }
}

fn levenshtein(a: &[u8], b: &[u8]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, &x) in a.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, &y) in b.iter().enumerate() {
            let cost = usize::from(x != y);
            let next = (row[j + 1] + 1).min(row[j] + 1).min(previous + cost);
            previous = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}
