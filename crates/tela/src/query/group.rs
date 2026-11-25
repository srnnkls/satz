//! `(group …)`: keys and aggregates over a query's matches.

use std::collections::{BTreeMap, BTreeSet};

use super::check::Mention;
use super::lex::{Kind, Token};
use super::pattern::{Comparison, Operand, Pattern};
use super::read::{Reader, suggest, unexpected};
use super::{Error, ErrorKind, Query};

#[derive(Debug, Clone)]
pub(super) struct Group {
    pub(super) keys: Vec<GroupKey>,
    pub(super) aggregates: Vec<Aggregate>,
}

#[derive(Debug, Clone)]
pub(super) struct GroupKey {
    pub(super) input: Input,
    pub(super) pos: usize,
}

/// What a group key or an aggregate reads.
#[derive(Debug, Clone)]
pub(super) enum Input {
    Var(String),
    /// A capture, and whether it captures a value rather than nodes.
    Capture(String, bool),
}

impl Input {
    pub(super) fn var(&self) -> Option<&str> {
        match self {
            Self::Var(var) => Some(var),
            Self::Capture(..) => None,
        }
    }

    pub(super) fn capture(&self) -> Option<&str> {
        match self {
            Self::Capture(name, _) => Some(name),
            Self::Var(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Op {
    Count,
    Sum,
    Min,
    Max,
}

impl Op {
    const NAMES: [&str; 4] = ["count", "sum", "min", "max"];

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "count" => Some(Self::Count),
            "sum" => Some(Self::Sum),
            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            _ => None,
        }
    }

    pub(super) fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// The node keys the aggregate reads from a capture.
    fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Count => &[],
            Self::Sum => &["words", "lines", "chars"],
            Self::Min | Self::Max => &["words", "lines", "chars", "level"],
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Aggregate {
    pub(super) op: Op,
    pub(super) key: Option<String>,
    pub(super) input: Input,
    pub(super) out: Option<String>,
    pub(super) filter: Option<(Comparison, Operand)>,
    pub(super) pos: usize,
}

impl Reader<'_> {
    pub(super) fn group(&mut self, open: &Token) -> Result<Group, Error> {
        let mut group = Group {
            keys: Vec::new(),
            aggregates: Vec::new(),
        };
        loop {
            let token = self.next();
            match token.kind {
                Kind::Close => return Ok(group),
                Kind::Eof => {
                    return Err(
                        Error::syntax(open.pos, "unclosed (group").with_hint("add the matching )")
                    );
                }
                Kind::Var | Kind::Capture => {
                    if !group.aggregates.is_empty() {
                        return Err(
                            Error::syntax(token.pos, "group keys precede the aggregates")
                                .with_hint("e.g. (group ?l (count @c @n))"),
                        );
                    }
                    let input = if token.kind == Kind::Var {
                        Input::Var(token.text)
                    } else {
                        Input::Capture(token.text, false)
                    };
                    group.keys.push(GroupKey {
                        input,
                        pos: token.pos,
                    });
                }
                Kind::Open => group.aggregates.push(self.aggregate()?),
                _ => {
                    return Err(unexpected(
                        &token,
                        "a ?variable, a @capture, an aggregate such as (count @c @n) or )",
                    ));
                }
            }
        }
    }

    fn aggregate(&mut self) -> Result<Aggregate, Error> {
        let op_token = self.next();
        let op = Op::from_name(&op_token.text)
            .filter(|_| op_token.kind == Kind::Symbol)
            .ok_or_else(|| {
                Error::syntax(
                    op_token.pos,
                    format!("unknown aggregate {:?}", op_token.text),
                )
                .with_hint(suggest(&op_token.text, &Op::NAMES))
            })?;
        let key = self.aggregate_key(op)?;
        let input = self.aggregate_input(op, key.as_deref())?;
        let out = self.aggregate_out(op)?;
        let filter = self.aggregate_filter(op)?;
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(&close, &format!(") closing ({}", op.name())));
        }
        Ok(Aggregate {
            op,
            key,
            input,
            out,
            filter,
            pos: op_token.pos,
        })
    }

    fn aggregate_key(&mut self, op: Op) -> Result<Option<String>, Error> {
        if self.peek().kind != Kind::Key {
            return Ok(None);
        }
        let token = self.next();
        let (name, keys) = (op.name(), op.keys());
        if keys.contains(&token.text.as_str()) {
            return Ok(Some(token.text));
        }
        let hint = if keys.is_empty() {
            "count takes no key; it counts nodes or values".to_owned()
        } else {
            format!("({name} ...) takes :{}", keys.join(" :"))
        };
        Err(Error::syntax(
            token.pos,
            format!("({name} ...) has no key :{}", token.text),
        )
        .with_hint(hint))
    }

    fn aggregate_input(&mut self, op: Op, key: Option<&str>) -> Result<Input, Error> {
        let operand = self.next();
        let name = op.name();
        match operand.kind {
            Kind::Capture => {
                let c = &operand.text;
                match op.keys().first() {
                    Some(example) if key.is_none() => Err(Error::syntax(
                        operand.pos,
                        format!("({name} @{c}) needs a :key"),
                    )
                    .with_hint(format!("e.g. ({name} :{example} @{c} @n)"))),
                    _ => Ok(Input::Capture(operand.text, false)),
                }
            }
            Kind::Var => {
                let v = &operand.text;
                if op == Op::Sum {
                    return Err(Error::syntax(
                        operand.pos,
                        format!("(sum ...) takes a @capture, not ?{v}"),
                    )
                    .with_hint("equal values of distinct nodes would collapse; sum a key over a capture, as in (sum :words @p @n)"));
                }
                if let Some(key) = key {
                    return Err(Error::syntax(
                        operand.pos,
                        format!("({name} :{key} ...) reads the key from a @capture, not ?{v}"),
                    )
                    .with_hint(format!("drop :{key} to compare the values of ?{v}")));
                }
                Ok(Input::Var(operand.text))
            }
            _ => Err(unexpected(
                &operand,
                &format!("a @capture or ?variable, as in ({name} @c @n)"),
            )),
        }
    }

    fn aggregate_out(&mut self, op: Op) -> Result<Option<String>, Error> {
        let next = self.peek().clone();
        match next.kind {
            Kind::Capture => {
                self.next();
                self.value(&next)?;
                self.numbers.insert(next.text.clone());
                Ok(Some(next.text))
            }
            Kind::Var => {
                let (name, t) = (op.name(), &next.text);
                Err(Error::syntax(
                    next.pos,
                    format!(
                        "the result of ({name} ...) is output, so it is a capture: @{t}, not ?{t}"
                    ),
                )
                .with_hint(format!("e.g. ({name} @c @{t})")))
            }
            _ => Ok(None),
        }
    }

    fn aggregate_filter(&mut self, op: Op) -> Result<Option<(Comparison, Operand)>, Error> {
        if self.peek().kind != Kind::Open {
            return Ok(None);
        }
        self.next();
        let form = self.next();
        let symbol = if form.kind == Kind::Direct {
            ">"
        } else {
            form.text.as_str()
        };
        let comparison = Comparison::from_symbol(symbol).ok_or_else(|| {
            Error::syntax(
                form.pos,
                format!(
                    "an aggregate filters with >, >=, < or <=, not {:?}",
                    form.text
                ),
            )
            .with_hint(format!("e.g. ({} @c @n (>= 3))", op.name()))
        })?;
        let symbol = symbol.to_owned();
        let limit = self.operand(&symbol)?;
        let close = self.next();
        if close.kind != Kind::Close {
            return Err(unexpected(&close, &format!(") closing ({symbol}")));
        }
        Ok(Some((comparison, limit)))
    }
}

impl Group {
    /// How often each variable is used by keys and aggregates.
    pub(super) fn uses(&self) -> BTreeMap<String, usize> {
        let mut uses = BTreeMap::new();
        let inputs = self
            .keys
            .iter()
            .map(|k| &k.input)
            .chain(self.aggregates.iter().map(|a| &a.input));
        for var in inputs.filter_map(Input::var) {
            *uses.entry(var.to_owned()).or_default() += 1;
        }
        uses
    }

    pub(super) fn is_output(&self, name: &str) -> bool {
        self.aggregates
            .iter()
            .any(|a| a.out.as_deref() == Some(name))
    }

    /// Checks that keys and aggregates read what the patterns bind and
    /// capture, and marks the captures that hold values.
    pub(super) fn check(&mut self, query: &Query, mentions: &[Mention]) -> Result<(), Error> {
        let context = Context {
            query,
            mentions,
            outputs: self
                .aggregates
                .iter()
                .filter_map(|a| a.out.clone())
                .collect(),
        };
        let repeated = repeated_captures(&query.members);
        for key in &mut self.keys {
            match &mut key.input {
                Input::Var(var) => context.bound(var, key.pos, "is a group key")?,
                Input::Capture(name, value) => {
                    *value = context.captured(name, key.pos, "so it cannot key it")?;
                    if repeated.contains(name.as_str()) {
                        return Err(Error::syntax(
                            key.pos,
                            format!("@{name} is captured under * or +, so it cannot key a group"),
                        )
                        .with_hint(format!("key the group on a capture that holds one node per match, or aggregate @{name}")));
                    }
                }
            }
        }
        for aggregate in &mut self.aggregates {
            let op = aggregate.op.name();
            let keyed = aggregate.op == Op::Sum || aggregate.key.is_some();
            match &mut aggregate.input {
                Input::Var(var) => context.bound(var, aggregate.pos, "is aggregated")?,
                Input::Capture(name, value) => {
                    let reads = format!("so ({op} ...) cannot read it");
                    *value = context.captured(name, aggregate.pos, &reads)?;
                    if *value && keyed {
                        return Err(Error::syntax(
                            aggregate.pos,
                            format!(
                                "@{name} captures a value, so ({op} ...) cannot read a key from it"
                            ),
                        )
                        .with_hint(format!(
                            "count it or take its min or max, as in (max @{name} @m)"
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

/// What a group's keys and aggregates are checked against.
struct Context<'a> {
    query: &'a Query,
    mentions: &'a [Mention],
    outputs: BTreeSet<String>,
}

impl Context<'_> {
    /// Fails unless a pattern binds `var`, which `role` reads.
    fn bound(&self, var: &str, pos: usize, role: &str) -> Result<(), Error> {
        let binds = self
            .mentions
            .iter()
            .any(|m| m.name == var && m.binds() && !m.negated);
        if binds {
            return Ok(());
        }
        Err(Error::new(
            ErrorKind::Unbound,
            pos,
            format!("?{var} {role} but no pattern binds it"),
        )
        .with_hint(format!(
            "bind it with :key ?{var} in a pattern of the query"
        )))
    }

    /// Whether the pattern capture `name` holds values; fails when no pattern
    /// captures it or an aggregate outputs it, which `reads` explains.
    fn captured(&self, name: &str, pos: usize, reads: &str) -> Result<bool, Error> {
        let captures = &self.query.captures;
        if !captures.iter().any(|c| c == name) {
            let candidates: Vec<&str> = captures.iter().map(String::as_str).collect();
            return Err(Error::new(
                ErrorKind::Unbound,
                pos,
                format!("@{name} is not captured by a pattern of the query"),
            )
            .with_hint(suggest(name, &candidates)));
        }
        if self.outputs.contains(name) {
            return Err(Error::syntax(
                pos,
                format!("@{name} is an aggregate of the group, {reads}"),
            )
            .with_hint("key the group on a ?variable, or aggregate a capture of the patterns"));
        }
        Ok(self.query.values.contains(name))
    }
}

/// The captures under `*` or `+`, which hold several nodes per match.
pub(super) fn repeated_captures(members: &[Pattern]) -> BTreeSet<String> {
    fn walk(pattern: &Pattern, under: bool, repeated: &mut BTreeSet<String>) {
        let under = under || pattern.repeats();
        if under {
            repeated.extend(pattern.capture.iter().cloned());
            for attr in &pattern.attrs {
                repeated.extend(attr.capture.iter().map(|(name, _)| name.clone()));
                repeated.extend(attr.score().map(str::to_owned));
            }
        }
        for child in children(pattern) {
            walk(child, under, repeated);
        }
    }
    let mut repeated = BTreeSet::new();
    for pattern in members {
        walk(pattern, false, &mut repeated);
    }
    repeated
}

/// The alternatives, chain links and relation targets of `pattern`.
fn children(pattern: &Pattern) -> impl Iterator<Item = &Pattern> {
    pattern
        .alts
        .iter()
        .chain(
            pattern
                .chains
                .iter()
                .flat_map(|chain| chain.links.iter().map(|link| &link.pattern)),
        )
        .chain(pattern.relations.iter().map(|r| r.target.as_ref()))
}
