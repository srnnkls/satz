//! The matcher: a query's patterns against a run's [`View`].
//!
//! Document order, memos and relation caches live here, per run; the tree
//! is only read.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;
use std::time::UNIX_EPOCH;

use super::pattern::{
    Chain, Check, Measure, Negation, Pattern, Quant, Relate, RelationKind, Similarity, Submatch,
    Test,
};
use super::view::View;
use super::{Attribute, Environment, Query, RunError};
use crate::similarity::{Embedding, EmbeddingError, Shingles, lowercase, words};

/// How many joined bindings a run may produce.
const MAX_BINDINGS: usize = 100_000;

/// Variables bound by one match, the nodes it captured, and the constraints
/// still waiting for their variables.
#[derive(Debug, Clone, Default)]
pub(super) struct Binding<'q> {
    pub(super) captures: BTreeMap<&'q str, Vec<usize>>,
    pub(super) vars: BTreeMap<String, String>,
    unequal: Vec<Constraint<'q>>,
}

impl<'q> Binding<'q> {
    fn bound(vars: BTreeMap<String, String>) -> Self {
        Self {
            vars,
            ..Self::default()
        }
    }

    fn capture(mut self, name: Option<&'q str>, at: usize) -> Self {
        if let Some(name) = name {
            self.captures.entry(name).or_default().push(at);
        }
        self
    }

    /// The values of `names`, or `None` when one is unbound.
    fn key(&self, names: &[String]) -> Option<Vec<&str>> {
        names
            .iter()
            .map(|name| self.vars.get(name).map(String::as_str))
            .collect()
    }
}

#[derive(Debug, Clone)]
struct Constraint<'q> {
    name: &'q str,
    value: String,
    relate: &'q Relate,
}

/// Bindings of one node to one pattern, shared between memo and callers.
type Matches<'q> = Rc<[Binding<'q>]>;

pub(super) struct Matcher<'q, 't> {
    pub(super) query: &'q Query,
    pub(super) env: &'q Environment,
    pub(super) view: View<'t>,
    kinds: HashMap<&'t str, Vec<usize>>,
    memo: HashMap<(usize, usize), Matches<'q>>,
    reached: HashMap<(RelationKind, usize), Rc<[usize]>>,
    linkers: Option<HashMap<usize, Vec<usize>>>,
    incoming: Option<HashMap<usize, Vec<usize>>>,
    shingles: HashMap<String, Rc<Shingles>>,
    vectors: HashMap<String, Option<Rc<Embedding>>>,
    pub(super) failure: Option<RunError>,
    produced: usize,
}

impl<'q, 't> Matcher<'q, 't> {
    pub(super) fn new(query: &'q Query, env: &'q Environment, view: View<'t>) -> Self {
        let mut kinds: HashMap<&'t str, Vec<usize>> = HashMap::new();
        for at in 0..view.entries.len() {
            kinds.entry(view.kind(at)).or_default().push(at);
        }
        Self {
            query,
            env,
            view,
            kinds,
            memo: HashMap::new(),
            reached: HashMap::new(),
            linkers: None,
            incoming: None,
            shingles: HashMap::new(),
            vectors: HashMap::new(),
            failure: None,
            produced: 0,
        }
    }

    fn produce(&mut self) -> Result<(), RunError> {
        self.produced += 1;
        if self.produced > MAX_BINDINGS {
            return Err(RunError::TooMany);
        }
        Ok(())
    }

    /// Every binding of `pattern` anywhere in the view, in document order;
    /// an `implicit` capture names each matched node `""`.
    pub(super) fn everywhere(
        &mut self,
        pattern: &'q Pattern,
        implicit: bool,
    ) -> Result<Vec<Binding<'q>>, RunError> {
        let candidates: Vec<usize> = match pattern.kinds() {
            None => (0..self.view.entries.len()).collect(),
            Some(kinds) => {
                let mut candidates: Vec<usize> = kinds
                    .iter()
                    .flat_map(|kind| self.kinds.get(kind).into_iter().flatten().copied())
                    .collect();
                if kinds.len() > 1 {
                    candidates.sort_unstable();
                }
                candidates
            }
        };
        let mut out = Vec::new();
        for at in candidates {
            for binding in self.at(pattern, at)?.iter() {
                let mut binding = binding.clone();
                if implicit {
                    binding.captures = BTreeMap::from([("", vec![at])]);
                }
                out.push(binding);
            }
        }
        Ok(out)
    }

    /// Joins the query's members, required ones first, then drops the rows
    /// its absent patterns match.
    pub(super) fn join(&mut self) -> Result<Vec<Binding<'q>>, RunError> {
        let query = self.query;
        let mut rows = vec![Binding::default()];
        for optional in [false, true] {
            for member in query.members.iter().filter(|m| m.optional() == optional) {
                rows = self.join_member(rows, member, optional)?;
            }
        }
        for absent in &query.absent {
            let bindings = self.everywhere(absent, false)?;
            let index = Index::new(&bindings, shared(&rows, &bindings));
            let mut kept = Vec::with_capacity(rows.len());
            for row in rows {
                let found = index
                    .candidates(&row)
                    .into_iter()
                    .any(|b| self.unify(&row, &bindings[b]).is_some());
                if !found {
                    kept.push(row);
                }
            }
            rows = kept;
        }
        Ok(rows)
    }

    fn join_member(
        &mut self,
        rows: Vec<Binding<'q>>,
        member: &'q Pattern,
        optional: bool,
    ) -> Result<Vec<Binding<'q>>, RunError> {
        let bindings = self.everywhere(member, false)?;
        let index = Index::new(&bindings, shared(&rows, &bindings));
        let near = if index.keys.is_empty() {
            self.near_index(&rows, &bindings)
        } else {
            None
        };
        let mut next = Vec::new();
        for row in rows {
            let candidates = match &near {
                Some(near) => {
                    let text = row.vars.get(near.name).cloned().unwrap_or_default();
                    let shingles = self.shingle(&text);
                    near.candidates(&shingles)
                }
                None => index.candidates(&row),
            };
            let mut matched = false;
            for b in candidates {
                let Some(joined) = self.unify(&row, &bindings[b]) else {
                    continue;
                };
                self.produce()?;
                matched = true;
                next.push(joined);
            }
            if optional && !matched {
                next.push(row);
            }
        }
        Ok(next)
    }

    /// An index of `bindings` by the shingles of the text a `near` or
    /// `overlap` constraint compares with a variable `rows` bind.
    fn near_index(
        &mut self,
        rows: &[Binding<'q>],
        bindings: &[Binding<'q>],
    ) -> Option<NearIndex<'q>> {
        let (row, first) = (rows.first()?, bindings.first()?);
        let name = first
            .unequal
            .iter()
            .rev()
            .find(|c| row.vars.contains_key(c.name) && c.relate.shingled())?
            .name;
        let mut near = NearIndex {
            name,
            postings: HashMap::new(),
            loose: Vec::new(),
        };
        for (i, binding) in bindings.iter().enumerate() {
            let constraint = binding
                .unequal
                .iter()
                .find(|c| c.name == name && c.relate.shingled());
            match constraint {
                Some(c) => {
                    let value = c.value.clone();
                    for shingle in self.shingle(&value).iter() {
                        near.postings.entry(shingle.to_owned()).or_default().push(i);
                    }
                }
                None => near.loose.push(i),
            }
        }
        Some(near)
    }

    pub(super) fn matches(&mut self, pattern: &'q Pattern, at: usize) -> Result<bool, RunError> {
        Ok(!self.at(pattern, at)?.is_empty())
    }

    pub(super) fn at(&mut self, pattern: &'q Pattern, at: usize) -> Result<Matches<'q>, RunError> {
        if let Some(cached) = self.memo.get(&(pattern.id, at)) {
            return Ok(Rc::clone(cached));
        }
        let matched: Matches<'q> = self.matched(pattern, at)?.into();
        self.memo.insert((pattern.id, at), Rc::clone(&matched));
        Ok(matched)
    }

    fn matched(&mut self, pattern: &'q Pattern, at: usize) -> Result<Vec<Binding<'q>>, RunError> {
        let capture = pattern.capture.as_deref();
        if !pattern.alts.is_empty() {
            let mut out = Vec::new();
            for alt in &pattern.alts {
                for binding in self.at(alt, at)?.iter() {
                    out.push(binding.clone().capture(capture, at));
                }
            }
            return Ok(out);
        }
        let entry = &self.view.entries[at];
        if !entry.matchable || pattern.kind.as_ref().is_some_and(|k| *k != entry.node.kind) {
            return Ok(Vec::new());
        }
        let Some(seed) = self.seed(pattern, at) else {
            return Ok(Vec::new());
        };
        let mut results = vec![(seed, None)];
        for chain in &pattern.chains {
            let matched = self.chain(chain, at)?;
            let optional = chain.optional();
            let mut next = Vec::new();
            for (binding, last) in &results {
                let kept = next.len();
                for m in &matched {
                    if matches!((m.first, *last), (Some(first), Some(last)) if first <= last) {
                        continue;
                    }
                    self.produce()?;
                    if let Some(joined) = self.unify(binding, &m.binding) {
                        next.push((joined, (*last).max(m.last)));
                    }
                }
                if optional && next.len() == kept {
                    next.push((binding.clone(), *last));
                }
            }
            results = next;
        }
        for relation in &pattern.relations {
            let mut targets = Vec::new();
            for related in self.related(relation.kind, at).iter() {
                targets.extend(self.at(&relation.target, *related)?.iter().cloned());
            }
            let optional = relation.target.optional();
            let mut next = Vec::new();
            for (binding, last) in &results {
                let kept = next.len();
                for target in &targets {
                    self.produce()?;
                    if let Some(joined) = self.unify(binding, target) {
                        next.push((joined, *last));
                    }
                }
                if optional && next.len() == kept {
                    next.push((binding.clone(), *last));
                }
            }
            results = next;
        }
        for not in &pattern.nots {
            let (candidates, target): (Vec<usize>, &Pattern) = match not {
                Negation::Pattern(target) => (self.descendants(at, false), target),
                Negation::Relation(relation) => {
                    (self.related(relation.kind, at).to_vec(), &relation.target)
                }
            };
            let mut found = Vec::new();
            for candidate in candidates {
                found.extend(self.at(target, candidate)?.iter().cloned());
            }
            let mut kept = Vec::with_capacity(results.len());
            for (binding, last) in results {
                if !found.iter().any(|f| self.unify(&binding, f).is_some()) {
                    kept.push((binding, last));
                }
            }
            results = kept;
        }
        Ok(results
            .into_iter()
            .map(|(binding, _)| binding.capture(capture, at))
            .collect())
    }

    /// The binding of a pattern's attributes at node `at`, or `None` when one
    /// fails.
    fn seed(&mut self, pattern: &'q Pattern, at: usize) -> Option<Binding<'q>> {
        let mut seed = Binding::default();
        for attr in &pattern.attrs {
            let mut bound = match &attr.check {
                Check::Groups { regex, groups } => {
                    let text = if attr.key == "matches" {
                        Some(self.view.node(at).text.clone())
                    } else {
                        self.value(at, &attr.key)
                    }?;
                    Binding::bound(submatches(regex, groups, &text)?)
                }
                Check::Test { test, negate } => {
                    if self.test(&attr.key, test, at) == *negate {
                        return None;
                    }
                    Binding::default()
                }
                Check::Bind(var) => {
                    let value = self.value(at, &attr.key)?;
                    Binding::bound(BTreeMap::from([(var.clone(), value)]))
                }
                Check::Constrain { var, relate } => {
                    let value = self.value(at, &attr.key)?;
                    Binding {
                        unequal: vec![Constraint {
                            name: var,
                            value,
                            relate,
                        }],
                        ..Binding::default()
                    }
                }
            };
            if let Some((name, _)) = &attr.capture {
                let value = self.value(at, &attr.key).unwrap_or_default();
                bound.vars.insert(format!("@{name}"), value);
            }
            seed = self.unify(&seed, &bound)?;
        }
        Some(seed)
    }

    fn descendants(&self, at: usize, direct: bool) -> Vec<usize> {
        let entry = &self.view.entries[at];
        if direct {
            entry.children.clone()
        } else {
            (at + 1..entry.end).collect()
        }
    }

    fn chain(&mut self, chain: &'q Chain, at: usize) -> Result<Vec<ChainMatch<'q>>, RunError> {
        let pivot = chain.pivot().unwrap_or(0);
        let direct = chain.links[0].direct;
        let mut out = Vec::new();
        for d in self.descendants(at, direct && pivot == 0) {
            if let Some(consumed) = self.consume(chain, pivot, d, at)? {
                out.extend(self.bind_links(chain, &consumed)?);
            }
        }
        Ok(out)
    }

    /// The siblings each link of `chain` takes when its pivot link matches
    /// node `d` under node `at`, or `None` when the chain does not fit there.
    fn consume(
        &mut self,
        chain: &'q Chain,
        pivot: usize,
        d: usize,
        at: usize,
    ) -> Result<Option<Vec<Vec<usize>>>, RunError> {
        let anchor = &chain.links[pivot].pattern;
        let entry = &self.view.entries[d];
        let (Some(parent), index) = (entry.parent, entry.index) else {
            return Ok(None);
        };
        if !self.matches(anchor, d)? {
            return Ok(None);
        }
        let siblings = self.view.entries[parent].children.clone();
        let plus = anchor.quant == Some(Quant::Plus);
        if plus && index > 0 && self.matches(anchor, siblings[index - 1])? {
            return Ok(None);
        }
        if chain.links[0].direct && parent != at {
            return Ok(None);
        }
        let mut consumed = vec![Vec::new(); chain.links.len()];
        consumed[pivot].push(d);
        let mut right = index + 1;
        if plus {
            while right < siblings.len() && self.matches(anchor, siblings[right])? {
                consumed[pivot].push(siblings[right]);
                right += 1;
            }
        }
        for (j, link) in chain.links.iter().enumerate().skip(pivot + 1) {
            let Some(taken) = self.take(&link.pattern, &siblings, Some(right), true)? else {
                return Ok(None);
            };
            consumed[j] = taken.nodes;
            right = taken.next.unwrap_or(siblings.len());
        }
        let mut left = index.checked_sub(1);
        for (j, link) in chain.links.iter().enumerate().take(pivot).rev() {
            let Some(taken) = self.take(&link.pattern, &siblings, left, false)? else {
                return Ok(None);
            };
            consumed[j] = taken.nodes;
            left = taken.next;
        }
        let fits =
            !(chain.first_child && left.is_some() || chain.last_child && right != siblings.len());
        Ok(fits.then_some(consumed))
    }

    /// The bindings of the nodes each link consumed, joined in order; a
    /// repeated link binds its first match only.
    fn bind_links(
        &mut self,
        chain: &'q Chain,
        consumed: &[Vec<usize>],
    ) -> Result<Vec<ChainMatch<'q>>, RunError> {
        let mut matches = vec![ChainMatch {
            binding: Binding::default(),
            first: None,
            last: None,
        }];
        for (link, nodes) in chain.links.iter().zip(consumed) {
            for &n in nodes {
                let found = self.at(&link.pattern, n)?;
                let found = if link.pattern.repeats() {
                    &found[..found.len().min(1)]
                } else {
                    &found[..]
                };
                let mut next = Vec::new();
                for m in &matches {
                    for b in found {
                        self.produce()?;
                        let Some(binding) = self.unify(&m.binding, b) else {
                            continue;
                        };
                        next.push(ChainMatch {
                            binding,
                            first: Some(m.first.map_or(n, |first| first.min(n))),
                            last: Some(m.last.map_or(n, |last| last.max(n))),
                        });
                    }
                }
                matches = next;
            }
        }
        Ok(matches)
    }

    /// The siblings from `start` that `pattern` takes, stepping right or
    /// left; `None` when a required pattern takes none.
    fn take(
        &mut self,
        pattern: &'q Pattern,
        siblings: &[usize],
        start: Option<usize>,
        rightward: bool,
    ) -> Result<Option<Taken>, RunError> {
        let mut nodes = Vec::new();
        let mut i = start;
        while let Some(at) = i.filter(|&at| at < siblings.len()) {
            if !self.matches(pattern, siblings[at])? {
                break;
            }
            nodes.push(siblings[at]);
            i = if rightward {
                Some(at + 1)
            } else {
                at.checked_sub(1)
            };
            if matches!(pattern.quant, None | Some(Quant::Optional)) {
                break;
            }
        }
        if nodes.is_empty() && matches!(pattern.quant, None | Some(Quant::Plus)) {
            return Ok(None);
        }
        if !rightward {
            nodes.reverse();
        }
        Ok(Some(Taken { nodes, next: i }))
    }

    pub(super) fn unify(&mut self, b: &Binding<'q>, other: &Binding<'q>) -> Option<Binding<'q>> {
        if other
            .vars
            .iter()
            .any(|(name, value)| b.vars.get(name).is_some_and(|v| v != value))
        {
            return None;
        }
        let mut vars = b.vars.clone();
        vars.extend(other.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        let unequal: Vec<Constraint<'q>> =
            b.unequal.iter().chain(&other.unequal).cloned().collect();
        let mut scores = Vec::new();
        for c in &unequal {
            let Some(value) = vars.get(c.name) else {
                continue;
            };
            let score = self.satisfies(c, value)?;
            if let Some(name) = c.relate.score() {
                scores.push((format!("@{name}"), score.to_string()));
            }
        }
        vars.extend(scores);
        let mut captures = b.captures.clone();
        for (name, nodes) in &other.captures {
            captures.entry(name).or_default().extend(nodes);
        }
        Some(Binding {
            captures,
            vars,
            unequal,
        })
    }

    /// Whether `bound` satisfies the constraint, with its similarity score.
    fn satisfies(&mut self, c: &Constraint<'q>, bound: &str) -> Option<f64> {
        let value = c.value.as_str();
        let holds = match c.relate {
            Relate::Not => value != bound,
            Relate::After => value > bound,
            Relate::Before => value < bound,
            Relate::Contains => lowercase(value).contains(&lowercase(bound)),
            Relate::Covers => bound == value || dotted_prefix(bound, value),
            Relate::Within => value == bound || dotted_prefix(value, bound),
            Relate::Similarity(similarity) => return self.similar(similarity, value, bound),
        };
        holds.then_some(0.0)
    }

    fn similar(&mut self, similarity: &Similarity, a: &str, b: &str) -> Option<f64> {
        let threshold = self.env.number(&similarity.threshold);
        if threshold <= 0.0 {
            return None;
        }
        let score = match similarity.measure {
            Measure::Near => self.shingle(a).jaccard(&self.shingle(b)),
            Measure::Overlap => self.shingle(a).containment(&self.shingle(b)),
            Measure::Similar => self.vector(a)?.cosine(&*self.vector(b)?),
        };
        (score >= threshold).then_some(score)
    }

    fn shingle(&mut self, text: &str) -> Rc<Shingles> {
        if let Some(cached) = self.shingles.get(text) {
            return Rc::clone(cached);
        }
        let shingles = Rc::new(Shingles::of(text, Shingles::DEFAULT_SIZE));
        self.shingles.insert(text.to_owned(), Rc::clone(&shingles));
        shingles
    }

    /// The embedding of `text`; `None` for the zero vector and on failure,
    /// which the run then reports.
    fn vector(&mut self, text: &str) -> Option<Rc<Embedding>> {
        if let Some(cached) = self.vectors.get(text) {
            return cached.clone();
        }
        let embedded = match self.env.embed.as_ref().map(|model| model.embed(text)) {
            Some(Ok(raw)) => match Embedding::new(&raw) {
                Ok(vector) => Some(Rc::new(vector)),
                Err(EmbeddingError::Zero) => None,
                Err(error) => {
                    self.failure.get_or_insert(RunError::Embedding(error));
                    None
                }
            },
            Some(Err(error)) => {
                self.failure.get_or_insert(RunError::Embed(error));
                None
            }
            None => {
                self.failure.get_or_insert(RunError::NoModel);
                None
            }
        };
        self.vectors.insert(text.to_owned(), embedded.clone());
        embedded
    }

    /// The nodes `kind` relates node `at` to, in document order.
    fn related(&mut self, kind: RelationKind, at: usize) -> Rc<[usize]> {
        match kind {
            RelationKind::To => return self.dereference(at).into_iter().collect(),
            RelationKind::From => {
                return self
                    .referrers()
                    .get(&at)
                    .map_or_else(|| Rc::from([]), |links| links.as_slice().into());
            }
            RelationKind::Reaches | RelationKind::Inbound => {}
        }
        let is_unit = self
            .query
            .unit
            .as_ref()
            .is_some_and(|unit| unit.kind == self.view.kind(at));
        if !is_unit {
            return Rc::from([]);
        }
        if let Some(cached) = self.reached.get(&(kind, at)) {
            return Rc::clone(cached);
        }
        let mut seen = BTreeSet::new();
        let mut queue = std::collections::VecDeque::from([at]);
        while let Some(current) = queue.pop_front() {
            let step = if kind == RelationKind::Inbound {
                self.sources(current)
            } else {
                self.unit_targets(current)
            };
            for target in step {
                if seen.insert(target) {
                    queue.push_back(target);
                }
            }
        }
        let reached: Rc<[usize]> = seen.into_iter().collect();
        self.reached.insert((kind, at), Rc::clone(&reached));
        reached
    }

    /// The unit node a link's `target` resolves to through the environment's
    /// targets, narrowed to the unit's file of the link's `path` and that
    /// file's section of the link's `anchor`.
    fn dereference(&self, at: usize) -> Option<usize> {
        let link = self.view.node(at);
        if link.kind != "link" {
            return None;
        }
        let target = self.target(at)?;
        let attr = |key: &str| link.attrs.get(key).map_or("", String::as_str);
        let (path, anchor) = (attr("path"), attr("anchor"));
        if path.is_empty() && anchor.is_empty() {
            return Some(target);
        }
        let file = self.view.entries[target]
            .children
            .iter()
            .copied()
            .find(|&f| {
                let node = self.view.node(f);
                node.kind == "file" && node.attrs.get("path").map_or("", String::as_str) == path
            })?;
        if anchor.is_empty() {
            return Some(file);
        }
        (file..self.view.entries[file].end).find(|&s| {
            let node = self.view.node(s);
            node.kind == "section" && node.attrs.get("id").is_some_and(|id| id == anchor)
        })
    }

    /// The node a link's `target` attribute resolves to.
    fn target(&self, at: usize) -> Option<usize> {
        let target = self
            .view
            .node(at)
            .attrs
            .get("target")
            .filter(|t| !t.is_empty())?;
        let id = self.env.targets.get(target)?;
        self.view.by_id.get(id).copied()
    }

    fn referrers(&mut self) -> &HashMap<usize, Vec<usize>> {
        if self.incoming.is_none() {
            let mut incoming: HashMap<usize, Vec<usize>> = HashMap::new();
            for at in 0..self.view.entries.len() {
                if let Some(target) = self.dereference(at) {
                    incoming.entry(target).or_default().push(at);
                }
            }
            self.incoming = Some(incoming);
        }
        self.incoming.get_or_insert_default()
    }

    /// The units the links inside unit `at` resolve to, other than itself.
    fn unit_targets(&self, at: usize) -> Vec<usize> {
        (at..self.view.entries[at].end)
            .filter(|&n| self.view.kind(n) == "link")
            .filter_map(|n| self.target(n))
            .filter(|&t| t != at)
            .collect()
    }

    /// The units with a link resolving to unit `at`.
    fn sources(&mut self, at: usize) -> Vec<usize> {
        if self.linkers.is_none() {
            let units = self
                .query
                .unit
                .as_ref()
                .and_then(|unit| self.kinds.get(unit.kind.as_str()))
                .cloned()
                .unwrap_or_default();
            let mut linkers: HashMap<usize, Vec<usize>> = HashMap::new();
            for source in units {
                for target in self.unit_targets(source) {
                    let sources = linkers.entry(target).or_default();
                    if !sources.contains(&source) {
                        sources.push(source);
                    }
                }
            }
            self.linkers = Some(linkers);
        }
        self.linkers
            .as_ref()
            .and_then(|linkers| linkers.get(&at))
            .cloned()
            .unwrap_or_default()
    }

    /// The value of `key` at node `at`: a computed text key or an attribute.
    pub(super) fn value(&self, at: usize, key: &str) -> Option<String> {
        let node = self.view.node(at);
        Some(match key {
            "text" => node.text.trim().to_owned(),
            "words" => self
                .measure(at, |n| words(&[n.text.as_str(), &n.body].concat()).len())
                .to_string(),
            "lines" => self.measure(at, lines).to_string(),
            "chars" => self
                .measure(at, |n| n.text.chars().count() + n.body.chars().count())
                .to_string(),
            "norm" => lowercase(&node.text.split_whitespace().collect::<Vec<_>>().join(" ")),
            "node" => format!("{at:09}"),
            "position" => {
                self.view
                    .ancestors(at)
                    .find(|&a| self.view.kind(a) == "file")?;
                let start = node.origin.position().map_or(0, |p| p.span().start);
                format!("{}\0{start:09}", node.origin.file())
            }
            _ => return node.attrs.get(key).cloned(),
        })
    }

    /// `measure` of node `at`, or its sum over a unit's files.
    fn measure(&self, at: usize, measure: impl Fn(&crate::tree::Node) -> usize) -> usize {
        let is_unit = self
            .query
            .unit
            .as_ref()
            .is_some_and(|unit| unit.kind == self.view.kind(at));
        if !is_unit {
            return measure(self.view.node(at));
        }
        self.view.entries[at]
            .children
            .iter()
            .filter(|&&c| self.view.kind(c) == "file")
            .map(|&c| measure(self.view.node(c)))
            .sum()
    }

    fn typed(&self, at: usize, key: &str) -> Option<&'q Attribute> {
        let id = self.view.entries[at].id?;
        self.env.typed.get(&id)?.get(key)
    }

    fn test(&self, key: &str, test: &Test, at: usize) -> bool {
        let node = self.view.node(at);
        match key {
            "contains" => {
                let needle = match test {
                    Test::Equals(literal) => literal.clone(),
                    Test::Param(name) => self.param(name),
                    _ => String::new(),
                };
                return lowercase(&node.text).contains(&lowercase(&needle));
            }
            "matches" => {
                return matches!(test, Test::Regex(regex) if regex.is_match(&node.text));
            }
            _ => {}
        }
        let Some(value) = self.value(at, key) else {
            return false;
        };
        let typed = self.typed(at, key);
        let amount = match typed {
            Some(Attribute::Amount(n)) => Some(*n),
            _ => None,
        };
        match test {
            Test::Older(limit) => older(&value, self.env.number(limit), self.env),
            Test::Compare(comparison, limit) => amount
                .or_else(|| value.parse().ok())
                .is_some_and(|n| comparison.holds(n, self.env.number(limit))),
            Test::Param(name) => equals(typed, &value, &self.param(name)),
            Test::Regex(regex) => match typed {
                Some(Attribute::Members(items)) => items.iter().any(|item| regex.is_match(item)),
                _ => regex.is_match(&value),
            },
            Test::Range(lo, hi) => {
                let integer = value.parse::<i64>().is_ok();
                let n = amount.or_else(|| value.parse().ok().filter(|_| integer));
                n.is_some_and(|n| *lo <= n && n <= *hi)
            }
            Test::Equals(want) => {
                self.qualified(at, key, &value, want) || equals(typed, &value, want)
            }
        }
    }

    /// Whether `want` names the unit node `at` as `qualifier:id`.
    fn qualified(&self, at: usize, key: &str, value: &str, want: &str) -> bool {
        let Some(unit) = &self.query.unit else {
            return false;
        };
        let node = self.view.node(at);
        if node.kind != unit.kind || key != unit.id_key {
            return false;
        }
        unit.qualifier_key
            .as_ref()
            .and_then(|q| node.attrs.get(q))
            .filter(|q| !q.is_empty())
            .and_then(|q| want.strip_prefix(q.as_str()))
            .and_then(|rest| rest.strip_prefix(':'))
            == Some(value)
    }

    fn param(&self, name: &str) -> String {
        self.env
            .params
            .get(name)
            .map(ToString::to_string)
            .unwrap_or_default()
    }
}

/// The siblings a chain link takes, and the position after them, `None`
/// past the left end.
struct Taken {
    nodes: Vec<usize>,
    next: Option<usize>,
}

#[derive(Debug, Clone)]
struct ChainMatch<'q> {
    binding: Binding<'q>,
    first: Option<usize>,
    last: Option<usize>,
}

/// Bindings bucketed by the values of the variables they share with the
/// rows they join.
struct Index {
    keys: Vec<String>,
    slots: HashMap<Vec<String>, usize>,
    buckets: Vec<Vec<usize>>,
    loose: Vec<usize>,
}

impl Index {
    fn new(bindings: &[Binding<'_>], keys: Vec<String>) -> Self {
        let mut index = Self {
            keys,
            slots: HashMap::new(),
            buckets: Vec::new(),
            loose: Vec::new(),
        };
        for (i, binding) in bindings.iter().enumerate() {
            let key = binding.key(&index.keys).filter(|_| !index.keys.is_empty());
            match key {
                Some(key) => {
                    let key: Vec<String> = key.into_iter().map(str::to_owned).collect();
                    let next = index.buckets.len();
                    let slot = *index.slots.entry(key).or_insert(next);
                    if slot == next {
                        index.buckets.push(Vec::new());
                    }
                    index.buckets[slot].push(i);
                }
                None => index.loose.push(i),
            }
        }
        index
    }

    /// The bindings that may join `row`, by index.
    fn candidates(&self, row: &Binding<'_>) -> Vec<usize> {
        let bucket = row
            .key(&self.keys)
            .filter(|_| !self.keys.is_empty())
            .map(|key| {
                let key: Vec<String> = key.into_iter().map(str::to_owned).collect();
                self.slots
                    .get(&key)
                    .map_or(&[][..], |&slot| &self.buckets[slot])
            });
        match bucket {
            Some(bucket) => bucket.iter().chain(&self.loose).copied().collect(),
            None => self
                .buckets
                .iter()
                .flatten()
                .chain(&self.loose)
                .copied()
                .collect(),
        }
    }
}

struct NearIndex<'q> {
    name: &'q str,
    postings: HashMap<String, Vec<usize>>,
    loose: Vec<usize>,
}

impl NearIndex<'_> {
    /// The bindings without the constraint, then those sharing a shingle
    /// with the row's text, by index.
    fn candidates(&self, shingles: &Shingles) -> Vec<usize> {
        let near: BTreeSet<usize> = shingles
            .iter()
            .filter_map(|shingle| self.postings.get(shingle))
            .flatten()
            .copied()
            .collect();
        self.loose.iter().copied().chain(near).collect()
    }
}

/// The variables both sides bind, judged by their first bindings.
fn shared(left: &[Binding<'_>], right: &[Binding<'_>]) -> Vec<String> {
    let (Some(l), Some(r)) = (left.first(), right.first()) else {
        return Vec::new();
    };
    l.vars
        .keys()
        .filter(|name| r.vars.contains_key(*name))
        .cloned()
        .collect()
}

fn dotted_prefix(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('.'))
}

fn submatches(
    regex: &regex::Regex,
    groups: &[Submatch],
    text: &str,
) -> Option<BTreeMap<String, String>> {
    let found = regex.captures(text)?;
    let mut vars = BTreeMap::new();
    for group in groups {
        let value = found.name(&group.name).map_or("", |m| m.as_str());
        match &group.var {
            None if value != group.value => return None,
            None => {}
            Some(var) => {
                if vars.get(var).is_some_and(|bound| bound != value) {
                    return None;
                }
                vars.insert(var.clone(), value.to_owned());
            }
        }
    }
    Some(vars)
}

/// A node's line count: a file's lines of body, or its span of lines.
pub(crate) fn lines(node: &crate::tree::Node) -> usize {
    if node.kind == "file" {
        let body = node.body.strip_suffix('\n').unwrap_or(&node.body);
        return if node.body.is_empty() {
            0
        } else {
            body.matches('\n').count() + 1
        };
    }
    node.origin
        .position()
        .or(node.position.as_ref())
        .map_or(1, |p| {
            usize::try_from(p.end_line() - p.line() + 1).unwrap_or(usize::MAX)
        })
}

fn equals(typed: Option<&Attribute>, raw: &str, want: &str) -> bool {
    match typed {
        Some(Attribute::Flag(flag)) => parse_bool(want) == Some(*flag),
        Some(Attribute::Amount(n)) => want
            .parse::<f64>()
            .is_ok_and(|w| w.partial_cmp(n) == Some(std::cmp::Ordering::Equal)),
        Some(Attribute::Members(items)) => items.iter().any(|item| item == want),
        None => raw == want,
    }
}

/// Go's `strconv.ParseBool`.
fn parse_bool(text: &str) -> Option<bool> {
    match text {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Some(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Some(false),
        _ => None,
    }
}

/// Whether the `YYYY-MM-DD` date `value` lies more than `days` days, in
/// whole hours, before the environment's clock.
fn older(value: &str, days: f64, env: &Environment) -> bool {
    let Some(date) = date_seconds(value) else {
        return false;
    };
    let now = match env.now.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_secs_f64(),
        Err(before) => -before.duration().as_secs_f64(),
    };
    now - date > (days * 24.0).trunc() * 3600.0
}

/// The Unix time of midnight UTC on the `YYYY-MM-DD` date `value`, as Go's
/// `time.Parse("2006-01-02", value)` reads it.
fn date_seconds(value: &str) -> Option<f64> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let field = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = &value[range];
        digits
            .bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| digits.parse().ok())
            .flatten()
    };
    let (year, month, day) = (field(0..4)?, field(5..7)?, field(8..10)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=days_in_month).contains(&day) {
        return None;
    }
    let days = i32::try_from(days_from_civil(year, month, day)).ok()?;
    Some(f64::from(days) * 86_400.0)
}

/// Days since 1970-01-01 of a proleptic Gregorian date, after Howard
/// Hinnant's `days_from_civil`.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
