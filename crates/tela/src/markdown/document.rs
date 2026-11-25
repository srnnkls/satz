//! Reads a Markdown body into editable items that remember where they came
//! from, so that writing reproduces untouched source byte for byte.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::{Arc, LazyLock};

use markdown_it::parser::inline::helpers::full_link::parse_link_destination;

use super::attributes::Fault;
use super::builder::{self, Element};
use super::directive::{self, Delimiters, find_closure};
use super::parse::Markdown;
use super::{Document, Expansion, Item, Parsed, Problem};
use crate::tree::{Origin, Position};

/// The block parser every document is read with.
pub(super) static MARKDOWN: LazyLock<Markdown> = LazyLock::new(Markdown::new);

/// The text a document was parsed from.
#[derive(Debug)]
pub(super) struct Source {
    pub(super) file: Arc<str>,
    pub(super) text: String,
}

impl Source {
    pub(super) fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// The source bytes of `range`, empty where it leaves the text.
    pub(super) fn slice(&self, range: Range<usize>) -> &[u8] {
        self.bytes().get(range).unwrap_or_default()
    }
}

/// An item as it was parsed. Its children sit in `lo..hi` of the source,
/// each after its gap; `trail` starts the source after the last child.
/// `sep` is written before a child whose predecessor changed, `lead` before
/// a child that moved to the front, and `prefix` is the content column's
/// line prefix of blanks and quote markers.
///
/// Items are identified by their snapshot: an item and its clones share
/// one, and a replacement records the snapshots it stands in for.
#[derive(Debug)]
pub(super) struct Snapshot {
    pub(super) source: Arc<Source>,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) lo: usize,
    pub(super) hi: usize,
    pub(super) trail: usize,
    pub(super) slots: Vec<Slot>,
    pub(super) sep: String,
    pub(super) lead: String,
    pub(super) prefix: String,
    pub(super) attrs: BTreeMap<String, String>,
    pub(super) text: String,
    pub(super) inline: bool,
    pub(super) list: bool,
    pub(super) delimiters: Option<Delimiters>,
    pub(super) target: Option<Target>,
}

/// A child as parsed, and the source between it and its predecessor.
#[derive(Debug)]
pub(super) struct Slot {
    pub(super) item: Arc<Snapshot>,
    pub(super) gap: Range<usize>,
}

/// Where a link's or image's destination is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Target {
    pub(super) span: Range<usize>,
    pub(super) form: Form,
}

/// How a link names its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Form {
    /// `[label](dest)`
    Bare,
    /// `[label](<dest>)`
    Angled,
    /// `[label][ref]`, `[label][]` or `[label]`: the span runs from the
    /// label's closing bracket.
    Reference,
    /// `<dest>`
    Autolink,
}

impl Target {
    /// The text that replaces [`Target::span`] to point at `url`.
    pub(super) fn write(&self, url: &str) -> String {
        match self.form {
            Form::Autolink => url.to_owned(),
            Form::Angled => angled(url),
            Form::Bare => destination(url),
            Form::Reference => format!("]({})", destination(url)),
        }
    }
}

/// `url` as an inline link destination: bare where it reads back the same,
/// else in angle brackets.
pub(super) fn destination(url: &str) -> String {
    let mut depth = 0usize;
    let bare = !url.is_empty()
        && url.chars().all(|c| match c {
            '(' => {
                depth += 1;
                true
            }
            ')' => depth.checked_sub(1).map(|d| depth = d).is_some(),
            '<' | '>' | '\\' | '&' | ' ' => false,
            c => !c.is_ascii_control(),
        })
        && depth == 0;
    if bare {
        url.to_owned()
    } else {
        format!("<{}>", angled(url))
    }
}

/// `url` escaped for the inside of angle brackets.
fn angled(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for c in url.chars() {
        if matches!(c, '<' | '>' | '\\' | '&') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

impl Snapshot {
    fn new(source: Arc<Source>, span: Range<usize>, attrs: BTreeMap<String, String>) -> Self {
        Self {
            source,
            start: span.start,
            end: span.end,
            lo: span.start,
            hi: span.end,
            trail: span.start,
            slots: Vec::new(),
            sep: String::new(),
            lead: String::new(),
            prefix: String::new(),
            attrs,
            text: String::new(),
            inline: false,
            list: false,
            delimiters: None,
            target: None,
        }
    }

    /// Records `children` as parsed: their gaps, the trail, and the
    /// separators a rewrite falls back on.
    fn adopt(&mut self, children: &[Item]) {
        let text = self.source.bytes();
        let mut at = self.lo;
        let mut sep = None;
        for child in children {
            let Some(snapshot) = &child.snapshot else {
                continue;
            };
            let start = at.max(snapshot.start);
            let gap = text.get(at..start).unwrap_or_default();
            if sep.is_none() && !self.slots.is_empty() && separating(gap) {
                sep = Some(lossy(gap));
            }
            self.slots.push(Slot {
                item: Arc::clone(snapshot),
                gap: at..start,
            });
            at = at.max(snapshot.end);
        }
        self.trail = at.min(self.hi);
        if let Some(first) = self.slots.first() {
            self.prefix = line_prefix(text, first.item.start);
        } else if self.delimiters.is_some() {
            self.prefix = line_prefix(text, self.hi);
        }
        match self.slots.first() {
            None => self.lead.clone_from(&self.prefix),
            Some(first) => {
                let gap = text.get(first.gap.clone()).unwrap_or_default();
                if trim(gap, b" \t\r\n>").is_empty() {
                    self.lead = lossy(gap);
                }
            }
        }
        if let Some(sep) = sep {
            self.sep = sep;
        } else if !self.inline {
            self.sep = format!("\n{}", self.prefix);
            if !self.list {
                self.sep = format!(
                    "\n{}{}",
                    self.prefix.trim_end_matches([' ', '\t']),
                    self.sep
                );
            }
        }
    }
}

/// Parses `text` of `file` into an editable document.
pub(super) fn parse(file: &str, text: &str) -> Parsed<Document> {
    let markdown = MARKDOWN.parser.parse(text);
    let source = Arc::new(Source {
        file: Arc::from(file),
        text: markdown.source().to_owned(),
    });
    let problems = directive::faults(&markdown)
        .into_iter()
        .map(|fault| problem(&source, &fault))
        .collect();
    let children: Vec<Item> = builder::build(&markdown, false)
        .children
        .into_iter()
        .map(|element| convert(&source, element, 0))
        .collect();
    let mut root = Snapshot::new(Arc::clone(&source), 0..source.text.len(), BTreeMap::new());
    root.adopt(&children);
    Parsed {
        value: Document {
            file: file.to_owned(),
            children,
            snapshot: Some(Arc::new(root)),
        },
        problems,
    }
}

fn problem(source: &Source, fault: &Fault) -> Problem {
    let message = match fault.id {
        Some(id) => format!("{id}: {}", fault.message),
        None => fault.message.clone(),
    };
    let at = fault.offset.min(source.text.len());
    Problem {
        origin: locate(source, at..at),
        message,
    }
}

fn convert(source: &Arc<Source>, mut element: Element, indent: usize) -> Item {
    let text = source.bytes();
    let first = element.start.unwrap_or_default();
    let inner = content(text, element.kind, first, indent);
    let children: Vec<Item> = std::mem::take(&mut element.children)
        .into_iter()
        .map(|child| convert(source, child, inner))
        .collect();
    let mut end = element.end.max(first);
    if let Some(delimiters) = &element.delimiters {
        end = end.max(delimiters.open.1);
    }
    if let Some(last) = children.last().and_then(|c| c.snapshot.as_ref()) {
        end = end.max(last.end);
    }
    let (start, end, lo) = extent(text, &element, &children, first..end, indent);
    let origin = locate(source, start..end.max(start));
    let span = origin.position().map_or(start..start, Position::span);
    let mut snapshot = Snapshot::new(Arc::clone(source), span.clone(), element.attrs.clone());
    snapshot.lo = span.start.max(lo);
    let mut item_text = String::new();
    match (element.kind, element.delimiters) {
        ("file" | "section" | "list" | "item" | "quote", _) => {
            snapshot.list = element.kind == "list";
        }
        ("directive", Some(delimiters)) if delimiters.fence >= 3 => {
            let mut lo = delimiters.open.1;
            if let Some(next) = text
                .get(lo..)
                .and_then(|rest| rest.iter().position(|&b| b == b'\n'))
            {
                lo += next + 1;
            }
            let hi = delimiters.close.map_or(snapshot.hi, |close| close.0);
            snapshot.lo = lo.min(snapshot.end);
            snapshot.hi = hi.max(snapshot.lo);
            snapshot.delimiters = Some(delimiters);
        }
        ("directive", Some(delimiters)) => {
            let (lo, hi) = delimiters.label();
            snapshot.inline = true;
            snapshot.lo = lo;
            snapshot.hi = hi;
            snapshot.delimiters = Some(delimiters);
            item_text = lossy(source.slice(lo..hi));
        }
        ("directive", None) => snapshot.inline = true,
        _ => {
            snapshot.inline = true;
            item_text = element.text;
        }
    }
    if let Some(link) = element.link {
        let (lo, target) = target(&source.text, link);
        snapshot.start = link.0;
        snapshot.end = link.1;
        snapshot.lo = lo;
        snapshot.hi = link.1;
        snapshot.target = target;
    }
    snapshot.adopt(&children);
    snapshot.text.clone_from(&item_text);
    Item {
        kind: element.kind.to_owned(),
        attrs: element.attrs,
        text: item_text,
        children,
        origin,
        expansion: Expansion::default(),
        snapshot: Some(Arc::new(snapshot)),
        anchors: Vec::new(),
    }
}

/// An element's source extent with its markers, closing lines and
/// indentation, and where its content starts.
fn extent(
    text: &[u8],
    element: &Element,
    children: &[Item],
    span: Range<usize>,
    indent: usize,
) -> (usize, usize, usize) {
    let Range { mut start, mut end } = span;
    let mut lo = start;
    match element.kind {
        "list" | "item" | "quote" => {
            if let Some(first) = children.first().and_then(|c| c.snapshot.as_ref()) {
                start = start.min(first.start);
            }
            lo = start;
            start = owner(text, start, element.kind);
        }
        "heading" | "section" => {
            start = after_indent(text, start, indent);
            if element.kind == "heading" {
                let rest = text.get(start..).unwrap_or_default();
                end = if atx(rest) {
                    end.max(start + line_length(rest))
                } else {
                    underline(text, end)
                };
            }
        }
        "table" => {
            start = start.min(after_indent(text, line_start(text, start), indent));
            end += line_length(text.get(end..).unwrap_or_default());
        }
        "code" => {
            if element.attrs.contains_key("lang") {
                start = after_indent(text, start, indent);
            } else if element
                .attrs
                .get("inline")
                .is_none_or(|inline| inline != "true")
            {
                start = indented(text, start);
            }
        }
        _ => {}
    }
    (start, end, lo)
}

/// Where a link's label starts, and where its destination is written.
fn target(text: &str, (start, end): (usize, usize)) -> (usize, Option<Target>) {
    let bytes = text.as_bytes();
    if bytes.get(start) == Some(&b'<') {
        let span = start + 1..end.saturating_sub(1).max(start + 1);
        let target = Target {
            span: span.clone(),
            form: Form::Autolink,
        };
        return (span.start, Some(target));
    }
    let label = start
        + if bytes.get(start) == Some(&b'!') {
            2
        } else {
            1
        };
    let within = bytes.get(..end).unwrap_or_default();
    let Some(close) = find_closure(within, label, b'[', b']') else {
        return (label.min(end), None);
    };
    if within.get(close + 1) != Some(&b'(') {
        let target = Target {
            span: close..end,
            form: Form::Reference,
        };
        return (label, Some(target));
    }
    let open = close + 2;
    let from = open
        + within
            .get(open..)
            .unwrap_or_default()
            .iter()
            .take_while(|b| b.is_ascii_whitespace())
            .count();
    let stop = parse_link_destination(text, from, end).map_or(from, |found| found.pos);
    let target = if within.get(from) == Some(&b'<') {
        Target {
            span: from + 1..stop.saturating_sub(1).max(from + 1),
            form: Form::Angled,
        }
    } else {
        Target {
            span: from..stop,
            form: Form::Bare,
        }
    };
    (label, Some(target))
}

/// The origin of `span` in `source`, without trailing line breaks.
pub(super) fn locate(source: &Source, span: Range<usize>) -> Origin {
    let text = source.bytes();
    let start = span.start.min(text.len());
    let end = start + trim_end(text.get(start..span.end).unwrap_or_default(), b"\r\n").len();
    let line = count_lines(text.get(..start).unwrap_or_default()) + 1;
    let end_line = line + count_lines(text.get(start..end).unwrap_or_default());
    let column = start - line_start(text, start) + 1;
    let position = Position::new(start..end, to_u32(line)..=to_u32(end_line), to_u32(column)).ok();
    Origin::named(&source.file, position)
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The line breaks in `text`.
pub(super) fn count_lines(text: &[u8]) -> usize {
    text.split(|&b| b == b'\n').count() - 1
}

/// The offset of the line holding `at`.
pub(super) fn line_start(text: &[u8], at: usize) -> usize {
    text.get(..at)
        .and_then(|head| head.iter().rposition(|&b| b == b'\n'))
        .map_or(0, |p| p + 1)
}

/// The opening marker of the list item or block quote whose content starts
/// at `start`.
fn owner(text: &[u8], start: usize, kind: &str) -> usize {
    let mut i = start;
    while i > 0
        && (matches!(text[i - 1], b' ' | b'\t')
            || kind == "item" && matches!(text[i - 1], b'\n' | b'\r'))
    {
        i -= 1;
    }
    if i == 0 {
        return start;
    }
    match text[i - 1] {
        b'>' if kind == "quote" => i - 1,
        b'-' | b'+' | b'*' if kind == "item" => i - 1,
        b'.' | b')' if kind == "item" => {
            let digits = text[..i - 1]
                .iter()
                .rev()
                .take_while(|b| b.is_ascii_digit())
                .count();
            if digits > 0 { i - 1 - digits } else { start }
        }
        _ => start,
    }
}

fn after_indent(text: &[u8], start: usize, indent: usize) -> usize {
    let line = line_start(text, start);
    let mut i = line;
    while i < text.len() && i - line < indent && text[i] != b'\n' {
        i += 1;
    }
    start.max(i)
}

/// The content column of a list item or block quote starting at `start`.
fn content(text: &[u8], kind: &str, start: usize, indent: usize) -> usize {
    if kind != "quote" && kind != "item" {
        return indent;
    }
    let line = line_start(text, start);
    let mut i = after_indent(text, line, indent);
    while matches!(text.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    if kind == "quote" {
        if text.get(i) == Some(&b'>') {
            i += 1;
            if text.get(i) == Some(&b' ') {
                i += 1;
            }
        }
        return i - line;
    }
    let marker = marker_length(text, i);
    if marker == 0 {
        return indent;
    }
    i += marker;
    let mut spaces = text[i..].iter().take_while(|&&b| b == b' ').count();
    if spaces == 0 || spaces > 4 || matches!(text.get(i + spaces), None | Some(b'\n')) {
        spaces = 1;
    }
    i + spaces - line
}

fn marker_length(text: &[u8], at: usize) -> usize {
    let end = at
        + text
            .get(at..)
            .unwrap_or_default()
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
    let Some(&mark) = text.get(end) else {
        return 0;
    };
    let ordered = end > at && matches!(mark, b'.' | b')');
    let bullet = end == at && matches!(mark, b'-' | b'+' | b'*');
    if (ordered || bullet) && matches!(text.get(end + 1), None | Some(b' ' | b'\t' | b'\n')) {
        end + 1 - at
    } else {
        0
    }
}

fn line_length(text: &[u8]) -> usize {
    text.iter().position(|&b| b == b'\n').unwrap_or(text.len())
}

/// Whether `line` opens an ATX heading.
pub(super) fn atx(line: &[u8]) -> bool {
    let i = line.iter().take(3).take_while(|&&b| b == b' ').count();
    line.get(i) == Some(&b'#')
}

/// The end of the underline of a setext heading whose title ends at `end`.
fn underline(text: &[u8], end: usize) -> usize {
    let end = trim_end(&text[..end.min(text.len())], b"\r\n").len();
    let Some(line) = text[end..].iter().position(|&b| b == b'\n') else {
        return end;
    };
    let next = end + line + 1;
    text[next..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(text.len(), |stop| next + stop)
}

/// The start of the indentation of an indented code block.
fn indented(text: &[u8], mut start: usize) -> usize {
    let mut width = 0;
    while width < 4 && start > 0 {
        match text[start - 1] {
            b' ' => width += 1,
            b'\t' => width += 4,
            _ => return start,
        }
        start -= 1;
    }
    start
}

fn separating(gap: &[u8]) -> bool {
    gap.contains(&b'\n') && trim(gap, b" \t\r\n>").is_empty()
}

/// The line before `start` with everything but blanks and quote markers
/// blanked out.
pub(super) fn line_prefix(text: &[u8], start: usize) -> String {
    let start = start.min(text.len());
    text[line_start(text, start)..start]
        .iter()
        .map(|&b| {
            if matches!(b, b' ' | b'\t' | b'>') {
                char::from(b)
            } else {
                ' '
            }
        })
        .collect()
}

fn trim<'a>(bytes: &'a [u8], set: &[u8]) -> &'a [u8] {
    let start = bytes
        .iter()
        .position(|b| !set.contains(b))
        .unwrap_or(bytes.len());
    trim_end(&bytes[start..], set)
}

pub(super) fn trim_end<'a>(bytes: &'a [u8], set: &[u8]) -> &'a [u8] {
    let end = bytes
        .iter()
        .rposition(|b| !set.contains(b))
        .map_or(0, |p| p + 1);
    &bytes[..end]
}

pub(super) fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests;
