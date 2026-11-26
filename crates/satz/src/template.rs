//! Templates: text with `{@capture}`, `{@capture.key}`, `{?var}` and
//! `{$param}` placeholders, filled from a query row and passed through
//! filters such as `{?t|lower}` or `{?l|times:#}`.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::query::{self, Capture, Hit, Row, Value};
use crate::tree::{NodeRef, Position, Tree};

/// What a placeholder reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sigil {
    /// `{@capture}`, or `{@capture.key}`.
    Capture,
    /// `{?var}`.
    Var,
    /// `{$param}`.
    Param,
}

/// A `{@capture.key}`, `{?var}` or `{$param}` placeholder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Placeholder<'a> {
    pub(crate) sigil: Sigil,
    /// The capture, variable or param name.
    pub(crate) name: &'a str,
    /// The key after the dot, such as `line` in `{@x.line}`.
    pub(crate) key: Option<&'a str>,
    /// The filters after `|`, each with its argument after `:`.
    pub(crate) filters: Vec<(&'a str, Option<&'a str>)>,
    /// The byte span of the whole placeholder, braces included.
    pub(crate) span: Range<usize>,
}

/// The filters a placeholder may apply, in the order they run.
pub(crate) const FILTERS: [&str; 5] = ["lower", "upper", "capitalize", "trim", "times"];

/// `value` through `filters`; an unknown filter or a `times` count that is
/// not a whole number leaves the value as it was.
fn filter(value: String, filters: &[(&str, Option<&str>)]) -> String {
    filters
        .iter()
        .fold(value, |value, &(name, arg)| match name {
            "lower" => value.to_lowercase(),
            "upper" => value.to_uppercase(),
            "capitalize" => {
                let mut chars = value.chars();
                chars.next().map_or_else(String::new, |first| {
                    first
                        .to_uppercase()
                        .chain(chars.flat_map(char::to_lowercase))
                        .collect()
                })
            }
            "trim" => value.trim().to_owned(),
            "times" => match value.trim().parse::<usize>() {
                Ok(n) => arg.unwrap_or_default().repeat(n),
                Err(_) => value,
            },
            _ => value,
        })
}

/// The placeholders of `template`, in order.
pub(crate) fn placeholders(template: &str) -> Vec<Placeholder<'_>> {
    let bytes = template.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(offset) = template[i..].find('{') {
        let start = i + offset;
        match placeholder(template, start) {
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
    let mut filters = Vec::new();
    while bytes.get(end) == Some(&b'|') {
        let stop = end + 1 + message[end + 1..].find(['|', '}'])?;
        let (name, arg) = match message[end + 1..stop].split_once(':') {
            Some((name, arg)) => (name, Some(arg)),
            None => (&message[end + 1..stop], None),
        };
        filters.push((name, arg));
        end = stop;
    }
    (bytes.get(end) == Some(&b'}')).then(|| Placeholder {
        sigil,
        name: &message[name_start..name_end],
        key,
        filters,
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

/// `message` with its placeholders filled from `row` and `params`.
pub(crate) fn render(
    message: &str,
    row: &Row,
    tree: &Tree,
    params: &BTreeMap<String, Value>,
) -> String {
    let mut rendered = String::with_capacity(message.len());
    let mut last = 0;
    for placeholder in placeholders(message) {
        rendered.push_str(&message[last..placeholder.span.start]);
        let value = fill(&placeholder, row, tree, params);
        rendered.push_str(&filter(value, &placeholder.filters));
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

pub(crate) fn hit<'r>(row: &'r Row, name: &str) -> Option<&'r Hit> {
    match row.captures.get(name)? {
        Capture::Nodes(hits) | Capture::Collected(hits) => hits.first(),
        Capture::Value(_) | Capture::Values(_) => None,
    }
}

pub(crate) fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => Some(*n),
        Value::Text(text) => text.parse().ok(),
    }
}
