//! Writes a document back from its items: untouched items as their source
//! bytes, edited and constructed items in canonical form.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::ops::Range;
use std::sync::Arc;

use super::attributes::bare_length;
use super::directive::Delimiters;
use super::document::{Slot, Snapshot, atx, destination, line_prefix};
use super::{Document, Item};

/// The bytes of `doc` as written.
pub(super) fn document(doc: &Document) -> Vec<u8> {
    let mut writer = Writer::default();
    if let Some(root) = &doc.snapshot {
        writer.region(root, &doc.children);
    } else {
        writer.blocks(&doc.children, "\n\n");
        if !writer.out.is_empty() {
            writer.out.push(b'\n');
        }
    }
    writer.out
}

/// The bytes of `item` as written in place.
pub(super) fn item(item: &Item) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.item(item);
    writer.out
}

/// `text` with `prefix` removed from every line after the first; a line
/// without it loses the prefix's trailing blanks instead.
pub(super) fn unprefix(text: &[u8], prefix: &str) -> Vec<u8> {
    let blank = prefix.trim_end_matches([' ', '\t']);
    let mut lines = text.split(|&b| b == b'\n');
    let mut out = lines.next().unwrap_or_default().to_vec();
    for line in lines {
        out.push(b'\n');
        let line = line
            .strip_prefix(prefix.as_bytes())
            .or_else(|| line.strip_prefix(blank.as_bytes()))
            .unwrap_or(line);
        out.extend_from_slice(line);
    }
    out
}

/// What was written before a child of a region.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Previous {
    Nothing,
    Slot(usize),
    Other,
}

#[derive(Default)]
struct Writer {
    out: Vec<u8>,
}

impl Writer {
    fn write(&mut self, bytes: impl AsRef<[u8]>) {
        self.out.extend_from_slice(bytes.as_ref());
    }

    fn region(&mut self, p: &Snapshot, children: &[Item]) {
        if p.inline {
            self.inline(p, children, None);
            return;
        }
        let mut previous = Previous::Nothing;
        for (i, child) in children.iter().enumerate() {
            let own = owned(child, p);
            let at = position(child, &p.slots);
            let kept = at.filter(|&at| {
                follows(at, previous) && (own || first(children, at, &p.slots) == Some(i))
            });
            match kept {
                Some(at) => self.write(p.source.slice(p.slots[at].gap.clone())),
                None if i > 0 => self.write(&p.sep),
                None => self.write(&p.lead),
            }
            if own {
                self.item(child);
            } else if child.snapshot.is_none() && p.list {
                let marker = marker(&p.attrs, i);
                let rest = format!("{}{}", p.prefix, " ".repeat(marker.len()));
                self.moved(child, &marker, &rest);
            } else {
                self.moved(child, "", &p.prefix);
            }
            previous = at.map_or(Previous::Other, Previous::Slot);
        }
        let mut trail = p.source.slice(p.trail..p.hi);
        if children.is_empty()
            && !p.slots.is_empty()
            && p.lo > 0
            && p.source.bytes().get(p.lo - 1) == Some(&b'\n')
        {
            let next = trail.iter().position(|&b| b == b'\n').map_or(0, |n| n + 1);
            trail = &trail[next..];
        }
        if !children.is_empty() && p.slots.is_empty() && !trail.starts_with(b"\n") {
            self.out.push(b'\n');
        }
        self.write(trail);
    }

    /// Writes inline children between the gaps of those parsed, with
    /// `splice` replacing a span of the trail.
    fn inline(&mut self, p: &Snapshot, children: &[Item], splice: Option<(Range<usize>, &str)>) {
        let mut kept = vec![false; p.slots.len()];
        for child in children {
            if let Some(at) = position(child, &p.slots) {
                kept[at] = true;
            }
        }
        let mut next = 0;
        for child in children {
            match position(child, &p.slots) {
                Some(at) if at >= next => {
                    self.gaps(p, next..at + 1);
                    next = at + 1;
                }
                None if kept.get(next) == Some(&false) => {
                    self.gaps(p, next..next + 1);
                    next += 1;
                }
                _ => {}
            }
            self.item(child);
        }
        self.gaps(p, next..p.slots.len());
        match splice {
            Some((span, text)) => {
                self.write(p.source.slice(p.trail..span.start));
                self.write(text);
                self.write(p.source.slice(span.end..p.hi));
            }
            None => self.write(p.source.slice(p.trail..p.hi)),
        }
    }

    fn gaps(&mut self, p: &Snapshot, slots: Range<usize>) {
        for slot in p.slots.get(slots).unwrap_or_default() {
            self.write(p.source.slice(slot.gap.clone()));
        }
    }

    /// Writes an item that left its parsed place: its first line after
    /// `first`, the others re-prefixed from their old column to `rest`.
    fn moved(&mut self, item: &Item, first: &str, rest: &str) {
        let from = item
            .snapshot
            .as_ref()
            .map(|s| line_prefix(s.source.bytes(), s.start))
            .unwrap_or_default();
        let inner = self::item(item);
        if first.is_empty() && from == rest {
            self.write(inner);
            return;
        }
        self.prefix(&unprefix(&inner, &from), first, rest);
    }

    fn item(&mut self, item: &Item) {
        let Some(p) = &item.snapshot else {
            self.generated(item);
            return;
        };
        let retext = item.text != p.text;
        let reattr = item.attrs != p.attrs;
        let Some(delimiters) = &p.delimiters else {
            if retext
                && !reattr
                && let Some(underline) = setext(item, p)
            {
                self.write(&item.text);
                self.write(underline);
                return;
            }
            if reattr
                && !retext
                && let Some((span, destination)) = relinked(item, p)
            {
                self.write(p.source.slice(p.start..p.lo));
                self.inline(p, &item.children, Some((span, &destination)));
                return;
            }
            if (retext || reattr) && p.inline {
                self.generated(item);
                return;
            }
            self.write(p.source.slice(p.start..p.lo));
            self.region(p, &item.children);
            return;
        };
        let (head, tail) = fences(item, p, delimiters, retext || reattr);
        self.write(head);
        if retext {
            self.write(&item.text);
        } else {
            self.region(p, &item.children);
        }
        self.write(tail);
    }

    fn generated(&mut self, item: &Item) {
        let name = attr(item, "name");
        let attrs = attribute_text(&item.attrs);
        let inline = attr(item, "inline") == "true";
        match item.kind.as_str() {
            "directive" if inline => {
                self.write(format!(":{name}["));
                self.label(item);
                self.write(format!("]{attrs}"));
            }
            _ if container(item) => {
                let colons = ":".repeat(fence_length(item));
                self.write(format!("{colons}{name}{attrs}\n"));
                self.blocks(&item.children, "\n\n");
                if !item.children.is_empty() {
                    self.out.push(b'\n');
                }
                self.write(colons);
            }
            "directive" => {
                self.write(format!("::{name}"));
                if !item.text.is_empty() || !item.children.is_empty() {
                    self.write("[");
                    self.label(item);
                    self.write("]");
                }
                self.write(attrs);
            }
            "heading" => {
                let level = attr(item, "level").parse().unwrap_or(0usize).max(1);
                self.write(format!("{} {}", "#".repeat(level), item.text));
            }
            "code" if !inline => {
                let info = match attr(item, "info") {
                    "" => attr(item, "lang"),
                    info => info,
                };
                self.write(format!("```{info}\n{}", item.text));
                if !item.text.ends_with('\n') {
                    self.out.push(b'\n');
                }
                self.write("```");
            }
            "code" => self.write(code_span(&item.text)),
            kind @ ("link" | "image") => {
                if kind == "image" {
                    self.write("!");
                }
                self.write("[");
                self.label(item);
                self.write(format!("]({})", destination(attr(item, "url"))));
            }
            "list" => {
                for (i, child) in item.children.iter().enumerate() {
                    if i > 0 {
                        self.out.push(b'\n');
                    }
                    let marker = marker(&item.attrs, i);
                    let mut inner = Self::default();
                    inner.item(child);
                    self.prefix(&inner.out, &marker, &" ".repeat(marker.len()));
                }
            }
            "quote" => {
                let mut inner = Self::default();
                inner.blocks(&item.children, "\n\n");
                self.prefix(&inner.out, "> ", "> ");
            }
            _ if item.text.is_empty() => self.blocks(&item.children, "\n\n"),
            _ => self.write(&item.text),
        }
    }

    fn label(&mut self, item: &Item) {
        if !item.text.is_empty() || item.children.is_empty() {
            self.write(&item.text);
            return;
        }
        for child in &item.children {
            self.item(child);
        }
    }

    fn blocks(&mut self, items: &[Item], separator: &str) {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.write(separator);
            }
            self.item(item);
        }
    }

    /// Writes `text` with `first` before its first line and `rest` before
    /// the others, blank lines without trailing spaces.
    fn prefix(&mut self, text: &[u8], first: &str, rest: &str) {
        for (i, line) in text.split(|&b| b == b'\n').enumerate() {
            let mut lead = rest;
            if i == 0 {
                lead = first;
            } else {
                self.out.push(b'\n');
            }
            if line.is_empty() {
                lead = lead.trim_end_matches(' ');
            }
            self.write(lead);
            self.write(line);
        }
    }
}

fn attr<'a>(item: &'a Item, key: &str) -> &'a str {
    item.attrs.get(key).map_or("", String::as_str)
}

/// Whether `item` is the child parsed into a slot of `p`.
fn owned(item: &Item, p: &Snapshot) -> bool {
    item.snapshot
        .as_ref()
        .is_some_and(|own| p.slots.iter().any(|slot| Arc::ptr_eq(&slot.item, own)))
}

/// The slot of `item`, or of the first item it replaces that has one.
fn position(item: &Item, slots: &[Slot]) -> Option<usize> {
    item.snapshot
        .iter()
        .chain(&item.anchors)
        .find_map(|anchor| {
            slots
                .iter()
                .position(|slot| Arc::ptr_eq(&slot.item, anchor))
        })
}

/// Whether the slot `at` was parsed right after `previous`.
fn follows(at: usize, previous: Previous) -> bool {
    match at.checked_sub(1) {
        None => previous == Previous::Nothing,
        Some(before) => previous == Previous::Slot(before),
    }
}

/// The index of the first of `children` standing in slot `at`.
fn first(children: &[Item], at: usize, slots: &[Slot]) -> Option<usize> {
    children
        .iter()
        .position(|child| position(child, slots) == Some(at))
}

/// The underline of a setext heading, from its line break.
fn setext<'a>(item: &Item, p: &'a Snapshot) -> Option<&'a [u8]> {
    let span = p.source.slice(p.start..p.end);
    if item.kind != "heading" || atx(span) {
        return None;
    }
    let line = span.iter().rposition(|&b| b == b'\n')?;
    Some(&span[line..])
}

/// The destination splice of a link whose `url` alone changed.
fn relinked(item: &Item, p: &Snapshot) -> Option<(Range<usize>, String)> {
    let target = p.target.as_ref()?;
    let url = item.attrs.get("url")?;
    let others_kept = item.attrs.len() == p.attrs.len()
        && item
            .attrs
            .iter()
            .all(|(key, value)| key == "url" || p.attrs.get(key) == Some(value));
    let within = p.trail <= target.span.start && target.span.end <= p.hi;
    (others_kept && within).then(|| (target.span.clone(), target.write(url)))
}

/// The opening and closing delimiters of a parsed directive: as written, or
/// rebuilt from its name and attributes when `rewrite` is set. Container
/// fences take their canonical length.
fn fences(item: &Item, p: &Snapshot, d: &Delimiters, rewrite: bool) -> (Vec<u8>, Vec<u8>) {
    let source = &p.source;
    let name = attr(item, "name");
    if d.fence >= 3 {
        let colons = ":".repeat(fence_length(item));
        let mut head = colons.clone().into_bytes();
        if rewrite {
            head.extend_from_slice(name.as_bytes());
            head.extend_from_slice(attribute_text(&item.attrs).as_bytes());
            head.extend_from_slice(source.slice(d.open.1..p.lo));
        } else {
            head.extend_from_slice(source.slice(d.open.0 + d.fence..p.lo));
        }
        let tail = match d.close {
            None => source.slice(p.hi..p.end).to_vec(),
            Some(close) => {
                let mut tail = colons.into_bytes();
                let rest = source.slice(close.0..p.end);
                let colons = rest.iter().take_while(|&&b| b == b':').count();
                tail.extend_from_slice(&rest[colons..]);
                tail
            }
        };
        return (head, tail);
    }
    if !rewrite {
        return (
            source.slice(p.start..p.lo).to_vec(),
            source.slice(p.hi..p.end).to_vec(),
        );
    }
    let mut open = format!("{}{name}", ":".repeat(d.fence.max(1)));
    let mut close = attribute_text(&item.attrs);
    if d.labeled || !item.text.is_empty() {
        open.push('[');
        close.insert(0, ']');
    }
    (open.into_bytes(), close.into_bytes())
}

/// Whether `item` is a block directive written with a colon fence.
pub(super) fn container(item: &Item) -> bool {
    if item.kind != "directive" || attr(item, "inline") == "true" {
        return false;
    }
    match &item.snapshot {
        Some(p) => p.delimiters.is_some_and(|d| d.fence >= 3),
        None => !item.children.is_empty(),
    }
}

/// The canonical fence of a container: one colon longer than the longest
/// fence it contains, and at least three.
fn fence_length(item: &Item) -> usize {
    fn inner(items: &[Item]) -> usize {
        items
            .iter()
            .map(|item| {
                if container(item) {
                    fence_length(item)
                } else {
                    inner(&item.children)
                }
            })
            .max()
            .unwrap_or(0)
    }
    (inner(&item.children) + 1).max(3)
}

fn marker(list: &BTreeMap<String, String>, i: usize) -> String {
    if list.get("ordered").is_some_and(|ordered| ordered == "true") {
        format!("{}. ", i + 1)
    } else {
        "- ".to_owned()
    }
}

/// A code span around `text`, its fence longer than every backtick run in it.
fn code_span(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let ticks = "`".repeat(longest + 1);
    let pad = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{ticks}{pad}{text}{pad}{ticks}")
}

/// A directive attribute list: the id and classes as shorthands, then the
/// other attributes by name.
fn attribute_text(attrs: &BTreeMap<String, String>) -> String {
    let mut parts = Vec::new();
    if let Some(id) = attrs.get("id") {
        parts.push(shorthand('#', "id", id));
    }
    if let Some(class) = attrs.get("class") {
        parts.extend(class.split_whitespace().map(|c| shorthand('.', "class", c)));
    }
    for (key, value) in attrs {
        if !matches!(key.as_str(), "name" | "inline" | "id" | "class") {
            parts.push(format!("{key}={}", value_text(value)));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("{{{}}}", parts.join(" "))
    }
}

fn is_bare(value: &str) -> bool {
    !value.is_empty() && bare_length(value.as_bytes()) == value.len()
}

fn shorthand(marker: char, name: &str, value: &str) -> String {
    if is_bare(value) {
        format!("{marker}{value}")
    } else {
        format!("{name}={}", value_text(value))
    }
}

/// An attribute value: bare where it can be, else a JSON string as Go's
/// `encoding/json` writes it without HTML escaping.
fn value_text(value: &str) -> String {
    if is_bare(value) {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c < ' ' || c == '\u{2028}' || c == '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests;
