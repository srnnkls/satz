//! Static checks a query passes before it runs: every variable bound where it
//! is used, captures that print, and patterns that join.

use std::collections::{BTreeMap, BTreeSet};

use super::group::{Group, repeated_captures};
use super::pattern::{Check, Pattern};
use super::{Query, ReadError, ReadErrorKind};

/// A variable's occurrence in a pattern.
#[derive(Debug, Clone)]
pub(super) struct Mention {
    pub(super) name: String,
    pub(super) pos: usize,
    /// The negation scope, an index into the walk's scopes.
    pub(super) scope: usize,
    pub(super) role: Role,
    pub(super) optional: bool,
    pub(super) output: bool,
    /// The query member, `None` inside a top-level `(not …)`.
    pub(super) member: Option<usize>,
    pub(super) negated: bool,
    /// Variables bound before an optional pattern is joined.
    pub(super) prior: BTreeSet<String>,
}

/// Whether a mention binds its variable or only compares with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Bind,
    Compare,
}

impl Mention {
    pub(super) fn binds(&self) -> bool {
        self.role == Role::Bind
    }
}

/// Negation scopes as a forest: each scope's parent.
#[derive(Default)]
struct Scopes(Vec<Option<usize>>);

impl Scopes {
    fn open(&mut self, parent: Option<usize>) -> usize {
        self.0.push(parent);
        self.0.len() - 1
    }

    fn within(&self, mut scope: usize, outer: usize) -> bool {
        loop {
            if scope == outer {
                return true;
            }
            match self.0[scope] {
                Some(parent) => scope = parent,
                None => return false,
            }
        }
    }
}

#[derive(Default)]
struct Walk {
    mentions: Vec<Mention>,
    scopes: Scopes,
    problem: Option<ReadError>,
}

#[derive(Clone, Copy)]
struct Place<'a> {
    scope: usize,
    member: Option<usize>,
    negated: bool,
    optional: bool,
    prior: &'a BTreeSet<String>,
}

impl Walk {
    fn fail(&mut self, error: ReadError) {
        self.problem.get_or_insert(error);
    }

    /// Records the variables a pattern's attributes mention, and the value
    /// captures a negation hides.
    fn attrs(&mut self, pattern: &Pattern, place: Place<'_>) {
        for attr in &pattern.attrs {
            if place.negated
                && let Some((name, pos)) = attr.output()
            {
                self.fail(
                    ReadError::syntax(
                        pos,
                        format!("@{name} is inside (not ...), so it never prints"),
                    )
                    .with_hint("capture a value outside the negation"),
                );
            }
            let mention = |name: &str, role: Role, output: bool| Mention {
                name: name.to_owned(),
                pos: attr.pos,
                scope: place.scope,
                role,
                optional: place.optional,
                output,
                member: place.member,
                negated: place.negated,
                prior: place.prior.clone(),
            };
            if let Some(var) = attr.var() {
                let role = if matches!(attr.check, Check::Bind(_)) {
                    Role::Bind
                } else {
                    Role::Compare
                };
                self.mentions
                    .push(mention(var, role, attr.capture.is_some()));
            }
            if let Check::Groups { groups, .. } = &attr.check {
                for var in groups.iter().filter_map(|g| g.var.as_deref()) {
                    self.mentions.push(mention(var, Role::Bind, false));
                }
            }
        }
    }

    /// Fails when the alternatives of `pattern` bind different variables.
    fn alternatives(&mut self, pattern: &Pattern) {
        if let [first, rest @ ..] = pattern.alts.as_slice() {
            let first_bound = bound(first);
            for alt in rest {
                let other = bound(alt);
                if other != first_bound {
                    self.fail(
                        ReadError::syntax(
                            alt.pos,
                            format!(
                                "alternatives bind different variables: {} and {}",
                                names(&first_bound),
                                names(&other)
                            ),
                        )
                        .with_hint("bind the same ?variables in every alternative of [ ]"),
                    );
                }
            }
        }
    }

    fn walk(&mut self, pattern: &Pattern, place: Place<'_>) {
        let place = Place {
            optional: place.optional || pattern.optional(),
            ..place
        };
        if place.negated
            && let Some(capture) = &pattern.capture
        {
            self.fail(
                ReadError::syntax(
                    pattern.pos,
                    format!("@{capture} is inside (not ...), so it never prints"),
                )
                .with_hint("capture a node outside the negation"),
            );
        }
        self.attrs(pattern, place);
        self.alternatives(pattern);
        for alt in &pattern.alts {
            self.walk(alt, place);
        }
        for (i, chain) in pattern.chains.iter().enumerate() {
            for link in &chain.links {
                let prior = enter(&link.pattern, place.prior, || before(pattern, i, 0));
                self.walk(
                    &link.pattern,
                    Place {
                        prior: &prior,
                        ..place
                    },
                );
            }
        }
        for (i, relation) in pattern.relations.iter().enumerate() {
            let prior = enter(&relation.target, place.prior, || {
                before(pattern, pattern.chains.len(), i)
            });
            self.walk(
                &relation.target,
                Place {
                    prior: &prior,
                    ..place
                },
            );
        }
        for not in &pattern.nots {
            let target = not.target();
            let prior = enter(target, place.prior, BTreeSet::new);
            let scope = self.scopes.open(Some(place.scope));
            self.walk(
                target,
                Place {
                    scope,
                    negated: true,
                    prior: &prior,
                    ..place
                },
            );
        }
    }
}

/// The variables bound before `child` is joined: `avail` for an optional
/// child, else those of its parent.
fn enter(
    child: &Pattern,
    prior: &BTreeSet<String>,
    avail: impl FnOnce() -> BTreeSet<String>,
) -> BTreeSet<String> {
    if child.optional() {
        avail()
    } else {
        prior.clone()
    }
}

/// The variables a pattern binds before its chain `chains` and relation
/// `relations`.
fn before(pattern: &Pattern, chains: usize, relations: usize) -> BTreeSet<String> {
    let mut vars = BTreeSet::new();
    for attr in &pattern.attrs {
        vars.extend(attr.bindings().map(str::to_owned));
        vars.extend(attr.score().map(str::to_owned));
    }
    for chain in &pattern.chains[..chains] {
        for link in &chain.links {
            vars.extend(required(&link.pattern));
        }
    }
    for relation in &pattern.relations[..relations] {
        vars.extend(required(&relation.target));
    }
    vars
}

/// The variables a pattern binds whenever it matches.
fn required(pattern: &Pattern) -> BTreeSet<String> {
    if pattern.optional() {
        return BTreeSet::new();
    }
    let mut vars = before(pattern, pattern.chains.len(), pattern.relations.len());
    for alt in &pattern.alts {
        vars.extend(required(alt));
    }
    vars
}

/// Every variable a pattern binds outside negations.
fn bound(pattern: &Pattern) -> BTreeSet<String> {
    let mut vars: BTreeSet<String> = pattern
        .attrs
        .iter()
        .flat_map(|a| a.bindings().map(str::to_owned))
        .collect();
    let nested = pattern
        .alts
        .iter()
        .chain(
            pattern
                .chains
                .iter()
                .flat_map(|c| c.links.iter().map(|l| &l.pattern)),
        )
        .chain(pattern.relations.iter().map(|r| r.target.as_ref()));
    for child in nested {
        vars.extend(bound(child));
    }
    vars
}

fn names(vars: &BTreeSet<String>) -> String {
    if vars.is_empty() {
        return "none".to_owned();
    }
    vars.iter()
        .map(|v| format!("?{v}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Query {
    pub(super) fn check(&mut self) -> Result<(), ReadError> {
        if self.members.iter().all(Pattern::optional) {
            return Err(ReadError::syntax(
                self.members.first().map_or(0, |p| p.pos),
                "a query needs a pattern that is not optional",
            )
            .with_hint("drop the ? from one pattern, or add a required pattern that the optional ones join"));
        }
        let mut walk = Walk::default();
        let root = walk.scopes.open(None);
        for (i, pattern) in self.members.iter().enumerate() {
            let mut prior = BTreeSet::new();
            if pattern.optional() {
                for (j, other) in self.members.iter().enumerate() {
                    if j != i && !other.optional() {
                        prior.extend(required(other));
                    }
                }
            }
            let place = Place {
                scope: root,
                member: Some(i),
                negated: false,
                optional: false,
                prior: &prior,
            };
            walk.walk(pattern, place);
        }
        let empty = BTreeSet::new();
        let mut absent_scopes = Vec::with_capacity(self.absent.len());
        for pattern in &self.absent {
            let scope = walk.scopes.open(Some(root));
            absent_scopes.push(scope);
            let place = Place {
                scope,
                member: None,
                negated: true,
                optional: false,
                prior: &empty,
            };
            walk.walk(pattern, place);
        }
        if let Some(problem) = walk.problem {
            return Err(problem);
        }
        let Walk {
            mentions, scopes, ..
        } = walk;
        let repeated = repeated_captures(&self.members);
        if let Some(name) = self
            .captures
            .iter()
            .find(|name| self.values.contains(*name) && repeated.contains(*name))
        {
            return Err(ReadError::syntax(
                self.members[0].pos,
                format!("@{name} captures a value under * or +, which holds one value per match"),
            )
            .with_hint("capture the value outside the repeated pattern"));
        }
        if let Some(mut group) = self.group.take() {
            let checked = group.check(self, &mentions);
            self.group = Some(group);
            checked?;
        }
        self.check_sorts(&mentions)?;
        let mut uses = self.group.as_ref().map(Group::uses).unwrap_or_default();
        self.output_uses(&mut uses);
        check_variables(&mentions, &scopes, &uses)?;
        self.check_absent(&mentions, &scopes, root, &absent_scopes)?;
        self.check_connected(&mentions, &absent_scopes)
    }
}

fn check_variables(
    mentions: &[Mention],
    scopes: &Scopes,
    uses: &BTreeMap<String, usize>,
) -> Result<(), ReadError> {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for m in mentions {
        *count.entry(m.name.as_str()).or_default() += 1;
    }
    for m in mentions {
        let outside = uses.get(&m.name).copied().unwrap_or_default();
        if count[m.name.as_str()] + outside == 1 && !m.output {
            let n = &m.name;
            return Err(ReadError::new(
                ReadErrorKind::Unbound,
                m.pos,
                format!("?{n} appears only once, so it matches anything"),
            )
            .with_hint(format!("use ?{n} again to join on it, or drop it")));
        }
    }
    for m in mentions {
        if m.binds() && m.optional && count[m.name.as_str()] > 1 && !m.prior.contains(&m.name) {
            let n = &m.name;
            return Err(ReadError::new(
                    ReadErrorKind::Unbound,
                    m.pos,
                    format!("?{n} is bound in an optional pattern, so it may be unbound where it is used again"),
                )
                .with_hint(format!("bind ?{n} first on the pattern that holds the optional one, or in a required pattern written before it")));
        }
    }
    for m in mentions.iter().filter(|m| !m.binds()) {
        let bound = mentions.iter().any(|o| {
            o.name == m.name && o.binds() && !o.optional && scopes.within(m.scope, o.scope)
        });
        if !bound {
            let n = &m.name;
            return Err(ReadError::new(
                ReadErrorKind::Unbound,
                m.pos,
                format!("?{n} is only compared, never bound"),
            )
            .with_hint(format!(
                "bind it with :key ?{n} in a pattern that encloses the comparison"
            )));
        }
    }
    Ok(())
}

impl Query {
    fn check_absent(
        &self,
        mentions: &[Mention],
        scopes: &Scopes,
        root: usize,
        absent_scopes: &[usize],
    ) -> Result<(), ReadError> {
        for (pattern, &scope) in self.absent.iter().zip(absent_scopes) {
            let shares = |required: bool| {
                mentions.iter().any(|m| {
                    scopes.within(m.scope, scope)
                        && mentions.iter().any(|o| {
                            o.name == m.name
                                && o.binds()
                                && o.scope == root
                                && (!required || !o.optional)
                        })
                })
            };
            if !shares(false) {
                return Err(ReadError::syntax(
                    pattern.pos,
                    "(not ...) shares no variable with the patterns of the query, so it drops every row or none",
                )
                .with_hint("use a ?variable bound by another pattern"));
            }
            if !shares(true) {
                return Err(ReadError::syntax(
                    pattern.pos,
                    "(not ...) shares only variables bound by optional patterns, so it cannot decide",
                )
                .with_hint("use a ?variable bound by a pattern that is not optional"));
            }
        }
        Ok(())
    }

    fn check_connected(
        &self,
        mentions: &[Mention],
        absent_scopes: &[usize],
    ) -> Result<(), ReadError> {
        let mut component: Vec<usize> = (0..self.members.len()).collect();
        let joined = |a: &Mention| (!a.negated).then_some(a.member).flatten();
        for a in mentions {
            for b in mentions {
                if a.name == b.name
                    && let (Some(x), Some(y)) = (joined(a), joined(b))
                {
                    let root = find(&component, y);
                    let at = find(&component, x);
                    component[at] = root;
                }
            }
        }
        for &scope in absent_scopes {
            let mut via: Option<usize> = None;
            for a in mentions.iter().filter(|a| a.scope == scope && !a.optional) {
                for b in mentions.iter().filter(|b| b.name == a.name) {
                    let Some(member) = joined(b) else { continue };
                    if let Some(via) = via {
                        let root = find(&component, via);
                        let at = find(&component, member);
                        component[at] = root;
                    }
                    via = Some(member);
                }
            }
        }
        for (i, pattern) in self.members.iter().enumerate().skip(1) {
            if find(&component, i) != find(&component, 0) {
                return Err(ReadError::syntax(
                    pattern.pos,
                    "this pattern shares no variable with the first, so every combination would match: a cartesian product between disconnected patterns",
                )
                .with_hint("share a ?variable between the patterns, or list alternatives in [ ]"));
            }
        }
        Ok(())
    }
}

/// The representative of `i`'s component.
fn find(component: &[usize], mut i: usize) -> usize {
    while component[i] != i {
        i = component[i];
    }
    i
}
