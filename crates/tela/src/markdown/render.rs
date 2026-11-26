//! Renders a document's directives as directive syntax, XML elements or
//! plain Markdown, and unwraps named directives into their content.

use std::fmt::Write as _;
use std::sync::{Arc, LazyLock};

use markdown_it::MarkdownIt;
use markdown_it::document::{Document as Syntax, NodeId};
use markdown_it::plugins::cmark::{block, inline};
use markdown_it::plugins::html::html_inline;
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

use super::attributes::{Attrs, Fault};
use super::directive::{self, Delimiters, Span};
use super::document::{MARKDOWN, count_lines, line_prefix, line_start, lossy};
use super::write::{self, unprefix};
use super::{Document, Item, Problem, RenderFormat};
use crate::tree::{Origin, Position};

/// The inline parser directive labels are read with: paragraphs of inline
/// Markdown, no other blocks.
static LABELS: LazyLock<MarkdownIt> = LazyLock::new(|| {
    let mut parser = MarkdownIt::empty();
    inline::newline::add(&mut parser);
    inline::escape::add(&mut parser);
    inline::backticks::add(&mut parser);
    inline::emphasis::add(&mut parser);
    inline::link::add(&mut parser);
    inline::image::add(&mut parser);
    inline::autolink::add(&mut parser);
    inline::entity::add(&mut parser);
    html_inline::add(&mut parser);
    block::paragraph::add(&mut parser);
    directive::add_inline(&mut parser);
    parser
});

/// How deep directive labels may nest.
const MAX_DEPTH: usize = 100;

/// Renders every directive of `doc` in `format`, replacing each changed
/// block with its rendered text. Blocks that fail keep their source.
pub(super) fn render(doc: &mut Document, format: RenderFormat) -> Result<(), Problem> {
    let mut problems = Vec::new();
    render_items(&mut doc.children, format, &mut problems);
    problems
        .into_iter()
        .min_by_key(|problem| problem.origin.position().map(|p| (p.line(), p.column())))
        .map_or(Ok(()), Err)
}

fn render_items(items: &mut [Item], format: RenderFormat, problems: &mut Vec<Problem>) {
    for i in 0..items.len() {
        match items[i].kind.as_str() {
            "code" | "html" | "rule" => continue,
            "directive" | "paragraph" | "heading" | "table" | "text" => {}
            _ => {
                render_items(&mut items[i].children, format, problems);
                continue;
            }
        }
        let text = markdown(&items[i]);
        match render_text(&text, format, 0) {
            Ok(out) if out != text => replace_text(items, i, out),
            Ok(_) => {}
            Err(faults) => {
                problems.extend(faults.iter().map(|fault| relocate(&items[i], &text, fault)));
            }
        }
    }
}

/// Replaces each block directive named `name` with its content, and returns
/// a document of those contents.
pub(super) fn unwrap(doc: &mut Document, name: &str) -> Document {
    let mut inners = Vec::new();
    unwrap_items(&mut doc.children, name, &mut inners);
    let mut head = Document::new(doc.file.clone());
    let text = inners.join("\n");
    if !text.is_empty() {
        let origin = Origin::named(
            &Arc::from(doc.file.as_str()),
            Position::new(0..0, 1..=1, 1).ok(),
        );
        let mut item = Item::new("text", origin);
        text.strip_suffix('\n')
            .unwrap_or(&text)
            .clone_into(&mut item.text);
        head.children.push(item);
    }
    head
}

fn unwrap_items(items: &mut [Item], name: &str, inners: &mut Vec<String>) {
    for i in 0..items.len() {
        let item = &items[i];
        let named = item.kind == "directive"
            && item.attrs.get("name").is_some_and(|n| n == name)
            && item
                .attrs
                .get("inline")
                .is_none_or(|inline| inline != "true");
        if !named {
            unwrap_items(&mut items[i].children, name, inners);
            continue;
        }
        let written = lossy(&write::item(item));
        let rest = written.split_once('\n').map_or("", |(_, rest)| rest);
        let inner = &rest[..rest.rfind('\n').map_or(0, |n| n + 1)];
        let body = inner.strip_suffix('\n').unwrap_or(inner);
        let text = lossy(&unprefix(body.as_bytes(), &item_prefix(item)));
        inners.push(inner.to_owned());
        replace_text(items, i, text);
    }
}

/// The Markdown of `item` as written, out of its content column.
fn markdown(item: &Item) -> String {
    lossy(&unprefix(&write::item(item), &item_prefix(item)))
}

fn item_prefix(item: &Item) -> String {
    item.snapshot
        .as_ref()
        .map(|p| line_prefix(p.source.bytes(), p.start))
        .unwrap_or_default()
}

fn replace_text(items: &mut [Item], i: usize, text: String) {
    let old = &items[i];
    let mut replacement = Item::new("text", old.origin.clone());
    replacement.text = text;
    replacement.expansion = old.expansion.clone();
    if let Some(replacement) = old.replace([replacement]).pop() {
        items[i] = replacement;
    }
}

/// The problem of `fault`, found in `text` written from `item`, located in
/// the item's source.
fn relocate(item: &Item, text: &str, fault: &Fault) -> Problem {
    let offset = fault.offset.min(text.len());
    let head = &text.as_bytes()[..offset];
    let line = count_lines(head);
    let column = offset - line_start(head, offset);
    let at = item.origin.position();
    let line = at.map_or(1, Position::line) + to_u32(line);
    let column = at.map_or(1, Position::column) + to_u32(column);
    let start = item.snapshot.as_ref().map_or_else(
        || at.map_or(0, |p| p.span().start),
        |p| offset_of(p.source.bytes(), line, column),
    );
    let message = match fault.id {
        Some(id) => format!("{id}: {}", fault.message),
        None => fault.message.clone(),
    };
    Problem {
        origin: item
            .origin
            .with_position(Position::new(start..start, line..=line, column).ok()),
        message,
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The offset of 1-based `line` and `column` in `text`, kept on that line.
fn offset_of(text: &[u8], line: u32, column: u32) -> usize {
    let skip = usize::try_from(line.saturating_sub(1)).unwrap_or(usize::MAX);
    let start = if skip == 0 {
        0
    } else {
        text.iter()
            .enumerate()
            .filter(|&(_, &b)| b == b'\n')
            .nth(skip - 1)
            .map_or(text.len(), |(at, _)| at + 1)
    };
    let end = text[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(text.len(), |n| start + n);
    let column = usize::try_from(column.saturating_sub(1)).unwrap_or(usize::MAX);
    start.saturating_add(column).min(end)
}

/// `input` with its directives rendered in `format`; other Markdown keeps
/// its bytes. Faults are at offsets of `input`.
fn render_text(input: &str, format: RenderFormat, depth: usize) -> Result<String, Vec<Fault>> {
    if depth > MAX_DEPTH {
        return Err(vec![Fault::new(0, "directive nesting exceeds 100 levels")]);
    }
    if !input.contains(':') {
        return Ok(input.to_owned());
    }
    let parser = if depth > 0 {
        &*LABELS
    } else {
        &MARKDOWN.parser
    };
    let syntax = parser.parse(input);
    let faults = directive::faults(&syntax);
    if !faults.is_empty() {
        return Err(faults);
    }
    let source = syntax.source();
    let mut edits = Vec::new();
    collect(&syntax, syntax.root(), format, depth, &mut edits)?;
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for ((start, stop), text) in edits {
        let kept = source
            .get(at..start)
            .filter(|_| stop <= source.len())
            .ok_or_else(|| vec![Fault::new(start, "overlapping directive source spans")])?;
        out.push_str(kept);
        out.push_str(&text);
        at = stop;
    }
    out.push_str(source.get(at..).unwrap_or_default());
    Ok(out)
}

fn collect(
    syntax: &Syntax,
    id: NodeId,
    format: RenderFormat,
    depth: usize,
    edits: &mut Vec<(Span, String)>,
) -> Result<(), Vec<Fault>> {
    for &child in syntax.children(id) {
        let found = directive::delimiters(syntax, child).zip(directive::signature(syntax, child));
        let Some((d, (name, attrs))) = found else {
            collect(syntax, child, format, depth, edits)?;
            continue;
        };
        if d.fence < 3 {
            edits.push(relabel(syntax.source(), name, attrs, &d, format, depth)?);
            continue;
        }
        let (open, close) = delimiters(name, attrs, &d, format, false);
        edits.push((d.open, open));
        collect(syntax, child, format, depth, edits)?;
        if let Some(span) = d.close {
            edits.push((span, close));
        }
    }
    Ok(())
}

/// The edit that renders a leaf or inline directive with its label.
fn relabel(
    source: &str,
    name: &str,
    attrs: &Attrs,
    d: &Delimiters,
    format: RenderFormat,
    depth: usize,
) -> Result<(Span, String), Vec<Fault>> {
    let (start, end) = d.label();
    let label = source.get(start..end).unwrap_or_default();
    let content = render_text(label, format, depth + 1).map_err(|faults| {
        faults
            .into_iter()
            .map(|f| f.shifted(start))
            .collect::<Vec<_>>()
    })?;
    let (open, close) = delimiters(name, attrs, d, format, true);
    let stop = d.close.map_or(end, |close| close.1);
    Ok(((d.open.0, stop), format!("{open}{content}{close}")))
}

/// The opening and closing text of a directive in `format`.
fn delimiters(
    name: &str,
    attrs: &Attrs,
    d: &Delimiters,
    format: RenderFormat,
    inline: bool,
) -> (String, String) {
    match format {
        RenderFormat::Markdown => {
            let mut label = name.to_owned();
            for (key, value) in attrs {
                let _ = write!(label, " {key}={}", go_quote(value));
            }
            let label = markdown_escape(&label);
            if inline {
                (format!("**{label}:** "), String::new())
            } else {
                (format!("**{label}**"), String::new())
            }
        }
        RenderFormat::Xml => {
            let mut open = format!("<{name}");
            for (key, value) in attrs {
                let _ = write!(open, " {key}=\"{}\"", xml_escape(value));
            }
            open.push('>');
            (open, format!("</{name}>"))
        }
        RenderFormat::Keep => {
            let list = attrs
                .iter()
                .map(|(key, value)| format!("{key}=\"{}\"", directive_escape(value)))
                .collect::<Vec<_>>()
                .join(" ");
            let list = if list.is_empty() {
                list
            } else {
                format!("{{{list}}}")
            };
            if inline {
                let open = format!("{}{name}", ":".repeat(d.fence.max(1)));
                if d.labeled {
                    (format!("{open}["), format!("]{list}"))
                } else {
                    (format!("{open}{list}"), String::new())
                }
            } else {
                let fence = ":".repeat(d.fence);
                (format!("{fence}{name}{list}"), fence)
            }
        }
    }
}

fn markdown_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '*' | '_' | '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

/// An attribute value escaped as Go's `html.EscapeString` does, with line
/// breaks and tabs as character references.
fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&#39;"),
            '"' => out.push_str("&#34;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            '\t' => out.push_str("&#x9;"),
            c => out.push(c),
        }
    }
    out
}

fn directive_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// `value` quoted as Go's `strconv.Quote` quotes it.
fn go_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{7}' => out.push_str("\\a"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            c if c < ' ' || c == '\u{7f}' => {
                let _ = write!(out, "\\x{:02x}", u32::from(c));
            }
            c if printable(c) => out.push(c),
            c if u32::from(c) < 0x10000 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => {
                let _ = write!(out, "\\U{:08x}", u32::from(c));
            }
        }
    }
    out.push('"');
    out
}

/// Go's `unicode.IsPrint`: letters, marks, numbers, punctuation, symbols
/// and the ASCII space.
fn printable(c: char) -> bool {
    c == ' '
        || matches!(
            c.general_category_group(),
            GeneralCategoryGroup::Letter
                | GeneralCategoryGroup::Mark
                | GeneralCategoryGroup::Number
                | GeneralCategoryGroup::Punctuation
                | GeneralCategoryGroup::Symbol
        )
}

#[cfg(test)]
mod tests;
