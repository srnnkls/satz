//! Rows from bindings: one per distinct match, or one per group, then
//! sorted and published.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use super::group::{Aggregate, Group, Input, Op};
use super::matcher::{Binding, Matcher};
use super::sort::SortBy;
use super::{Capture, Row, Value};

/// What a row holds for one capture before it is published.
pub(super) enum Cell {
    /// Nodes, by view position; `collected` when a group gathered them.
    Nodes { nodes: Vec<usize>, collected: bool },
    /// Values; `number` when they print as numbers.
    Values {
        values: Vec<String>,
        number: bool,
        collected: bool,
    },
}

pub(super) struct Out {
    cells: Vec<(String, Cell)>,
    vars: BTreeMap<String, String>,
}

impl Out {
    fn value(&self, name: &str) -> Option<&str> {
        self.cells.iter().find_map(|(cell, content)| match content {
            Cell::Values { values, .. } if cell == name => values.first().map(String::as_str),
            _ => None,
        })
    }
}

impl Matcher<'_, '_> {
    /// One row per distinct capture, sort and message value.
    pub(super) fn rows(&self, bindings: Vec<Binding<'_>>) -> Vec<Out> {
        let query = self.query;
        let mut seen = HashSet::new();
        let mut rows = Vec::new();
        for binding in bindings {
            let mut key = String::new();
            let mut cells = Vec::with_capacity(query.captures.len());
            for name in &query.captures {
                if query.values.contains(name) {
                    let value = binding.vars.get(&format!("@{name}"));
                    if let Some(value) = value {
                        let _ = write!(key, "{value:?}");
                    }
                    key.push(';');
                    cells.push((
                        name.clone(),
                        Cell::Values {
                            values: value.cloned().into_iter().collect(),
                            number: query.numbers.contains(name),
                            collected: false,
                        },
                    ));
                    continue;
                }
                let nodes = binding
                    .captures
                    .get(name.as_str())
                    .cloned()
                    .unwrap_or_default();
                for node in &nodes {
                    let _ = write!(key, "{node},");
                }
                key.push(';');
                cells.push((
                    name.clone(),
                    Cell::Nodes {
                        nodes,
                        collected: false,
                    },
                ));
            }
            for var in query.sorts.iter().filter_map(|s| s.var()) {
                match binding.vars.get(var) {
                    Some(value) => {
                        let _ = write!(key, "{value:?}");
                    }
                    None => key.push('-'),
                }
            }
            for name in &query.printed {
                let value = binding.vars.get(name).map_or("", String::as_str);
                let _ = write!(key, "{value:?};");
            }
            if seen.insert(key) {
                rows.push(Out {
                    cells,
                    vars: binding.vars,
                });
            }
        }
        rows
    }

    /// One row per group of bindings with equal keys; a group without keys
    /// is one row even over no bindings.
    pub(super) fn group_rows(&self, group: &Group, bindings: &[Binding<'_>]) -> Vec<Out> {
        let mut slots: HashMap<String, usize> = HashMap::new();
        let mut groups: Vec<Vec<&Binding<'_>>> = Vec::new();
        if group.keys.is_empty() {
            slots.insert(String::new(), 0);
            groups.push(Vec::new());
        }
        for binding in bindings {
            let key = group_key(group, binding);
            let next = groups.len();
            let slot = *slots.entry(key).or_insert(next);
            if slot == next {
                groups.push(Vec::new());
            }
            groups[slot].push(binding);
        }
        groups
            .iter()
            .filter_map(|members| self.group_row(group, members))
            .collect()
    }

    fn group_row(&self, group: &Group, members: &[&Binding<'_>]) -> Option<Out> {
        let query = self.query;
        let nodes = |name: &str| {
            let mut all: Vec<usize> = members
                .iter()
                .flat_map(|b| b.captures.get(name).into_iter().flatten().copied())
                .collect();
            all.sort_unstable();
            all.dedup();
            all
        };
        let values = |name: &str| {
            let mut all: Vec<String> = Vec::new();
            for value in members
                .iter()
                .filter_map(|b| b.vars.get(&format!("@{name}")))
            {
                if !all.contains(value) {
                    all.push(value.clone());
                }
            }
            all
        };
        let mut vars = BTreeMap::new();
        for var in group.keys.iter().filter_map(|k| k.input.var()) {
            if let Some(value) = members.first().and_then(|b| b.vars.get(var)) {
                vars.insert(var.to_owned(), value.clone());
            }
        }
        let mut results = BTreeMap::new();
        for aggregate in &group.aggregates {
            let value = self.evaluate(aggregate, members, &nodes);
            if let Some((comparison, limit)) = &aggregate.filter
                && !value.is_some_and(|v| comparison.holds(v, self.env.number(limit)))
            {
                return None;
            }
            if let (Some(out), Some(value)) = (&aggregate.out, value) {
                results.insert(out.as_str(), value.to_string());
            }
        }
        let cells = query
            .captures
            .iter()
            .map(|name| {
                let keyed = group
                    .keys
                    .iter()
                    .any(|k| k.input.capture() == Some(name.as_str()));
                let cell = if group.is_output(name) {
                    Cell::Values {
                        values: results.get(name.as_str()).cloned().into_iter().collect(),
                        number: true,
                        collected: false,
                    }
                } else if query.values.contains(name) {
                    Cell::Values {
                        values: values(name),
                        number: query.numbers.contains(name),
                        collected: !keyed,
                    }
                } else {
                    Cell::Nodes {
                        nodes: nodes(name),
                        collected: !keyed,
                    }
                };
                (name.clone(), cell)
            })
            .collect();
        Some(Out { cells, vars })
    }

    fn evaluate(
        &self,
        aggregate: &Aggregate,
        members: &[&Binding<'_>],
        nodes: &impl Fn(&str) -> Vec<usize>,
    ) -> Option<f64> {
        let mut operands = Vec::new();
        match &aggregate.input {
            Input::Capture(name, false) => {
                let elements = nodes(name);
                if aggregate.op == Op::Count {
                    return Some(count(elements.len()));
                }
                let key = aggregate.key.as_deref().unwrap_or_default();
                operands.extend(
                    elements
                        .iter()
                        .filter_map(|&e| self.value(e, key))
                        .filter_map(|v| finite(&v)),
                );
            }
            input => {
                let name = match input {
                    Input::Var(var) => var.clone(),
                    Input::Capture(name, _) => format!("@{name}"),
                };
                let mut seen = Vec::new();
                for value in members.iter().filter_map(|b| b.vars.get(&name)) {
                    if !seen.contains(&value) {
                        seen.push(value);
                        operands.extend(finite(value));
                    }
                }
                if aggregate.op == Op::Count {
                    return Some(count(seen.len()));
                }
            }
        }
        let first = *operands.first()?;
        Some(match aggregate.op {
            Op::Min => operands.iter().copied().fold(first, f64::min),
            Op::Max => operands.iter().copied().fold(first, f64::max),
            Op::Sum | Op::Count => operands.iter().sum(),
        })
    }

    /// Orders rows stably by the query's sort keys; rows lacking a key's
    /// value follow those that have it, and a key compares as numbers when
    /// every present value parses as one.
    pub(super) fn sort(&self, rows: Vec<Out>) -> Vec<Out> {
        let sorts = &self.query.sorts;
        if sorts.is_empty() {
            return rows;
        }
        let keyed: Vec<Vec<Option<String>>> = rows
            .iter()
            .map(|row| sorts.iter().map(|s| self.sort_value(&s.by, row)).collect())
            .collect();
        let numeric: Vec<bool> = (0..sorts.len())
            .map(|k| {
                keyed
                    .iter()
                    .filter_map(|values| values[k].as_deref())
                    .all(|v| v.parse::<f64>().is_ok())
            })
            .collect();
        let mut order: Vec<usize> = (0..rows.len()).collect();
        order.sort_by(|&a, &b| {
            for (k, sort) in sorts.iter().enumerate() {
                let ordering = match (&keyed[a][k], &keyed[b][k]) {
                    (Some(_), None) => return Ordering::Less,
                    (None, Some(_)) => return Ordering::Greater,
                    (None, None) => continue,
                    (Some(x), Some(y)) if numeric[k] => {
                        compare_floats(x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0))
                    }
                    (Some(x), Some(y)) => x.cmp(y),
                };
                let ordering = if sort.descending {
                    ordering.reverse()
                } else {
                    ordering
                };
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            Ordering::Equal
        });
        let mut rows: Vec<Option<Out>> = rows.into_iter().map(Some).collect();
        order.into_iter().filter_map(|i| rows[i].take()).collect()
    }

    fn sort_value(&self, by: &SortBy, row: &Out) -> Option<String> {
        match by {
            SortBy::Var(var) => row.vars.get(var).cloned(),
            SortBy::Capture(capture) => row.value(capture).map(str::to_owned),
            SortBy::Key(capture, key) => row.cells.iter().find_map(|(name, cell)| match cell {
                Cell::Nodes { nodes, .. } if name == capture => {
                    nodes.iter().min().and_then(|&first| self.value(first, key))
                }
                _ => None,
            }),
        }
    }

    /// The row as callers see it: unbound captures left out, nodes as hits.
    pub(super) fn publish(&self, row: Out) -> Row {
        let captures = row
            .cells
            .into_iter()
            .filter_map(|(name, cell)| {
                let capture = match cell {
                    Cell::Nodes { nodes, .. } if nodes.is_empty() => return None,
                    Cell::Values { values, .. } if values.is_empty() => return None,
                    Cell::Nodes { nodes, collected } => {
                        let hits = nodes.into_iter().map(|n| self.view.hit(n)).collect();
                        if collected {
                            Capture::Collected(hits)
                        } else {
                            Capture::Nodes(hits)
                        }
                    }
                    Cell::Values {
                        values,
                        number,
                        collected,
                    } => {
                        let mut values: Vec<Value> = values
                            .into_iter()
                            .map(|v| publish_value(v, number))
                            .collect();
                        if !collected && values.len() == 1 {
                            Capture::Value(values.swap_remove(0))
                        } else {
                            Capture::Values(values)
                        }
                    }
                };
                Some((name, capture))
            })
            .collect();
        let vars = row
            .vars
            .into_iter()
            .filter(|(name, _)| !name.starts_with('@'))
            .collect();
        Row { captures, vars }
    }
}

fn group_key(group: &Group, binding: &Binding<'_>) -> String {
    let mut key = String::new();
    for k in &group.keys {
        let var = match &k.input {
            Input::Var(var) => Some(var.clone()),
            Input::Capture(name, true) => Some(format!("@{name}")),
            Input::Capture(_, false) => None,
        };
        match (var, &k.input) {
            (Some(var), _) => match binding.vars.get(&var) {
                Some(value) => {
                    let _ = write!(key, "{value:?}");
                }
                None => key.push('-'),
            },
            (None, input) => {
                let mut nodes = input
                    .capture()
                    .and_then(|name| binding.captures.get(name))
                    .cloned()
                    .unwrap_or_default();
                nodes.sort_unstable();
                nodes.dedup();
                for node in nodes {
                    let _ = write!(key, "{node},");
                }
            }
        }
        key.push(';');
    }
    key
}

fn publish_value(value: String, number: bool) -> Value {
    match value.parse::<f64>() {
        Ok(n) if number && n.is_finite() => Value::Number(n),
        _ => Value::Text(value),
    }
}

fn finite(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// Go's `cmp.Compare` for floats: NaN sorts first and equals NaN.
fn compare_floats(x: f64, y: f64) -> Ordering {
    match (x.is_nan(), y.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
    }
}
