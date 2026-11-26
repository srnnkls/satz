//! Builds henia's element tree from a markdown-it document: sections around
//! headings, goldmark's block and inline extents, tables as rows and cells.

use std::collections::{BTreeMap, HashMap, HashSet};

use markdown_it::document::{Document, NodeId, Text, TextSpecial};
use markdown_it::plugins::cmark::block::blockquote::Blockquote;
use markdown_it::plugins::cmark::block::code::CodeBlock;
use markdown_it::plugins::cmark::block::fence::CodeFence;
use markdown_it::plugins::cmark::block::heading::ATXHeading;
use markdown_it::plugins::cmark::block::hr::ThematicBreak;
use markdown_it::plugins::cmark::block::lheading::SetextHeader;
use markdown_it::plugins::cmark::block::list::{BulletList, ListItem, OrderedList};
use markdown_it::plugins::cmark::block::paragraph::Paragraph;
use markdown_it::plugins::cmark::block::reference::Definition;
use markdown_it::plugins::cmark::inline::autolink::Autolink;
use markdown_it::plugins::cmark::inline::backticks::CodeInline;
use markdown_it::plugins::cmark::inline::image::Image;
use markdown_it::plugins::cmark::inline::link::Link;
use markdown_it::plugins::extra::tables::{Table, TableBody, TableCell, TableHead, TableRow};
use markdown_it::plugins::html::html_block::HtmlBlock;
use markdown_it::plugins::html::html_inline::HtmlInline;

use super::directive::{self, Container, Delimiters, InlineDirective, Leaf, Span, source_span};

/// One node of the tree under construction, positioned in the parsed text.
#[derive(Debug, Default)]
pub(super) struct Element {
    pub(super) kind: &'static str,
    pub(super) attrs: BTreeMap<String, String>,
    pub(super) text: String,
    pub(super) start: Option<usize>,
    pub(super) end: usize,
    pub(super) line: usize,
    pub(super) end_line: usize,
    pub(super) column: usize,
    pub(super) children: Vec<Element>,
    /// A directive's delimiters.
    pub(super) delimiters: Option<Delimiters>,
    /// An inline directive's label: the text between its brackets, or the
    /// label an inline rule claimed.
    pub(super) label: Option<String>,
    /// A link's or image's whole source, from its opening bracket.
    pub(super) link: Option<Span>,
    /// A block's span as markdown-it reads it, from its first marker.
    pub(super) source: Option<Span>,
}

impl Element {
    fn new(kind: &'static str) -> Self {
        Self {
            kind,
            ..Self::default()
        }
    }

    fn attr(mut self, key: &str, value: impl Into<String>) -> Self {
        self.attrs.insert(key.to_owned(), value.into());
        self
    }

    fn extend_attrs(&mut self, attrs: &[(String, String)]) {
        self.attrs.extend(attrs.iter().cloned());
    }

    fn span(&mut self, (start, end): Span) {
        self.start = Some(start);
        self.end = end;
    }

    fn level(&self) -> usize {
        self.attrs
            .get("level")
            .and_then(|level| level.parse().ok())
            .unwrap_or(0)
    }
}

enum Item {
    Block(NodeId),
    Tight(Vec<NodeId>),
}

/// Builds the element tree of `doc`, whose source is the parsed text;
/// headings open sections inside directives, and at the top when
/// `sectioned` is set.
pub(super) fn build(doc: &Document, sectioned: bool) -> Element {
    let mut builder = Builder::new(doc);
    let mut root = Element::new("file");
    root.span((0, builder.src.len()));
    let items = builder.items(doc.root());
    builder.blocks(&mut root, items, sectioned);
    builder.finish(&mut root);
    root
}

struct Builder<'a> {
    doc: &'a Document,
    body: &'a str,
    src: &'a [u8],
    newlines: Vec<usize>,
    ids: HashMap<NodeId, String>,
}

fn is_block(doc: &Document, id: NodeId) -> bool {
    let n = doc.node(id);
    n.is::<Paragraph>()
        || n.is::<ATXHeading>()
        || n.is::<SetextHeader>()
        || n.is::<CodeFence>()
        || n.is::<CodeBlock>()
        || n.is::<HtmlBlock>()
        || n.is::<ThematicBreak>()
        || n.is::<BulletList>()
        || n.is::<OrderedList>()
        || n.is::<ListItem>()
        || n.is::<Blockquote>()
        || n.is::<Table>()
        || n.is::<Container>()
        || n.is::<Leaf>()
        || n.is::<Definition>()
}

fn heading_level(doc: &Document, id: NodeId) -> Option<u8> {
    let n = doc.node(id);
    n.cast::<ATXHeading>()
        .map(|h| h.level)
        .or_else(|| n.cast::<SetextHeader>().map(|h| h.level))
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn cover(acc: &mut Option<Span>, (a, b): Span) {
    *acc = Some(acc.map_or((a, b), |(x, y)| (x.min(a), y.max(b))));
}

impl<'a> Builder<'a> {
    fn new(doc: &'a Document) -> Self {
        let body = doc.source();
        let src = body.as_bytes();
        let mut builder = Self {
            doc,
            body,
            src,
            newlines: body.match_indices('\n').map(|(at, _)| at).collect(),
            ids: HashMap::new(),
        };
        builder.ids = builder.heading_ids();
        builder
    }

    fn text(&self, (a, b): Span) -> &'a str {
        self.body.get(a..b).unwrap_or_default()
    }

    /// goldmark's automatic heading ids, generated for every heading in
    /// document order from its last line.
    fn heading_ids(&self) -> HashMap<NodeId, String> {
        let mut headings = Vec::new();
        self.doc.walk(self.doc.root(), |id| {
            if heading_level(self.doc, id).is_some() {
                headings.push(id);
            }
        });
        let mut seen = HashSet::new();
        headings
            .into_iter()
            .map(|id| {
                let last = self
                    .heading_lines(id)
                    .last()
                    .map_or("", |&line| self.text(line));
                (id, unique_id(&mut seen, slug(last)))
            })
            .collect()
    }

    fn items(&self, id: NodeId) -> Vec<Item> {
        let mut out = Vec::new();
        let mut run = Vec::new();
        for &child in self.doc.children(id) {
            if !is_block(self.doc, child) {
                run.push(child);
                continue;
            }
            if !run.is_empty() {
                out.push(Item::Tight(std::mem::take(&mut run)));
            }
            if !self.doc.node(child).is::<Definition>() {
                out.push(Item::Block(child));
            }
        }
        if !run.is_empty() {
            out.push(Item::Tight(run));
        }
        out
    }

    fn line_start(&self, at: usize) -> usize {
        self.src[..at]
            .iter()
            .rposition(|&c| c == b'\n')
            .map_or(0, |p| p + 1)
    }

    fn line_end(&self, at: usize) -> usize {
        self.src[at..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(self.src.len(), |p| at + p)
    }

    /// The offset after the line break at `at`, if one is there.
    fn after_eol(&self, mut at: usize) -> usize {
        if self.src.get(at) == Some(&b'\r') {
            at += 1;
        }
        if self.src.get(at) == Some(&b'\n') {
            at += 1;
        }
        at
    }

    fn trim_end(&self, start: usize, mut end: usize) -> usize {
        while end > start && is_space(self.src[end - 1]) {
            end -= 1;
        }
        end
    }

    fn trim_start(&self, mut start: usize, end: usize) -> usize {
        while start < end && is_space(self.src[start]) {
            start += 1;
        }
        start
    }

    /// goldmark's heading lines: the title of an ATX heading, the lines of a
    /// setext heading before its underline.
    fn heading_lines(&self, id: NodeId) -> Vec<Span> {
        let Some((s, e)) = source_span(self.doc, id) else {
            return Vec::new();
        };
        if self.doc.node(id).is::<ATXHeading>() {
            let line = &self.src[s..e];
            let hashes = line.iter().take_while(|&&c| c == b'#').count();
            if hashes == line.len() {
                return Vec::new();
            }
            let hs = s + hashes + line[hashes..].iter().take_while(|&&c| is_space(c)).count();
            let he = self.trim_end(hs, e);
            if he <= hs {
                return Vec::new();
            }
            let title = &self.src[hs..he];
            let mut j = title.len() - 1;
            while title[j] == b'#' && j > 0 {
                j -= 1;
            }
            if j == 0 && title[0] == b'#' {
                return Vec::new();
            }
            let mut stop = title.len();
            if j != title.len() - 1 && is_space(title[j]) {
                stop = j;
                while stop > 0 && is_space(title[stop - 1]) {
                    stop -= 1;
                }
            }
            return vec![(hs, hs + stop)];
        }
        let underline = self.line_start(self.trim_end(s, e));
        let mut lines = Vec::new();
        let mut at = s;
        while at < underline {
            let end = self.line_end(at);
            let start = at
                + self.src[at..end]
                    .iter()
                    .take_while(|&&c| c == b' ' || c == b'\t')
                    .count();
            lines.push((start, (end + 1).min(self.src.len())));
            at = end + 1;
        }
        if let Some(last) = lines.last_mut() {
            last.1 = self.trim_end(last.0, last.1);
        }
        lines
    }

    fn blocks(&mut self, parent: &mut Element, items: Vec<Item>, sectioned: bool) {
        let mut path: Vec<usize> = Vec::new();
        for item in items {
            if let (Item::Block(id), true) = (&item, sectioned)
                && let Some(level) = heading_level(self.doc, *id)
            {
                let level = usize::from(level);
                while !path.is_empty() && at_path(parent, &path).level() >= level {
                    path.pop();
                }
                let title: String = self
                    .heading_lines(*id)
                    .into_iter()
                    .map(|line| self.text(line))
                    .collect();
                let section = Element::new("section")
                    .attr("id", self.ids.get(id).cloned().unwrap_or_default())
                    .attr("title", title)
                    .attr("level", level.to_string());
                let host = at_path_mut(parent, &path);
                host.children.push(section);
                path.push(host.children.len() - 1);
            }
            if let Some(element) = self.block(item) {
                at_path_mut(parent, &path).children.push(element);
            }
        }
    }

    fn block(&mut self, item: Item) -> Option<Element> {
        let id = match item {
            Item::Tight(run) => return Some(self.tight(&run)),
            Item::Block(id) => id,
        };
        let n = self.doc.node(id);
        let span = source_span(self.doc, id);
        let (s, t) = span.unwrap_or_default();
        let mut e;
        if let Some(level) = heading_level(self.doc, id) {
            e = Element::new("heading").attr("level", level.to_string());
            self.child_inlines(&mut e, id);
            let lines = self.heading_lines(id);
            if let (Some(first), Some(last)) = (lines.first(), lines.last()) {
                e.span((first.0, last.1));
            }
            e.text = lines.into_iter().map(|line| self.text(line)).collect();
        } else if n.is::<Paragraph>() {
            e = Element::new("paragraph");
            self.child_inlines(&mut e, id);
            e.span((s, self.trim_end(s, t)));
        } else if let Some(fence) = n.cast::<CodeFence>() {
            let info = fence.info.trim();
            e = Element::new("code")
                .attr("lang", info.split(' ').next().unwrap_or_default())
                .attr("inline", "false");
            if !info.is_empty() {
                e = e.attr("info", info);
            }
            fence.as_str().clone_into(&mut e.text);
            let (open, end, _) = self.fence(id, fence);
            e.span((open, end));
        } else if let Some(code) = n.cast::<CodeBlock>() {
            e = Element::new("code").attr("inline", "false");
            let start = self.content_start(s, code.as_str());
            e.span((start, self.after_eol(self.trim_end(s, t))));
        } else if let Some(html) = n.cast::<HtmlBlock>() {
            e = Element::new("html");
            e.span((self.content_start(s, html.as_str()), self.after_eol(t)));
        } else if n.is::<ThematicBreak>() {
            e = Element::new("rule");
            e.span((s, self.trim_end(s, t)));
        } else if n.is::<BulletList>() || n.is::<OrderedList>() {
            e = Element::new("list").attr("ordered", n.is::<OrderedList>().to_string());
            self.nested(&mut e, id);
        } else if n.is::<ListItem>() {
            e = Element::new("item");
            self.nested(&mut e, id);
        } else if n.is::<Blockquote>() {
            e = Element::new("quote");
            self.nested(&mut e, id);
        } else if n.is::<Table>() {
            e = Element::new("table");
            self.rows(&mut e, id);
            if let (Some(first), Some(last)) = (e.children.first(), e.children.last()) {
                e.start = first.start;
                e.end = last.end;
            }
        } else if let Some(container) = n.cast::<Container>() {
            e = Element::new("directive").attr("name", container.name.clone());
            e.extend_attrs(&container.attrs);
            e.span((
                container.open.0,
                container.open.1.max(container.close.map_or(0, |c| c.1)),
            ));
            let items = self.items(id);
            self.blocks(&mut e, items, true);
        } else {
            let leaf = n.cast::<Leaf>()?;
            e = Element::new("directive").attr("name", leaf.name.clone());
            e.extend_attrs(&leaf.attrs);
            e.span((leaf.open.0, leaf.open.1.max(leaf.close.1)));
            self.child_inlines(&mut e, id);
        }
        e.delimiters = directive::delimiters(self.doc, id);
        e.source = span;
        Some(e)
    }

    /// A paragraph markdown-it left implicit, as in a tight list item.
    fn tight(&mut self, run: &[NodeId]) -> Element {
        let mut e = Element::new("paragraph");
        for &id in run {
            self.inlines(&mut e, id);
        }
        if let Some((s, t)) = self.run_extent(run) {
            e.span((s, self.trim_end(s, t)));
        }
        e
    }

    fn run_extent(&self, run: &[NodeId]) -> Option<Span> {
        let mut acc = None;
        for &id in run {
            if let Some(span) = source_span(self.doc, id) {
                cover(&mut acc, span);
            }
        }
        acc.map(|(s, t)| (self.trim_start(s, t), t))
    }

    fn nested(&mut self, e: &mut Element, id: NodeId) {
        let items = self.items(id);
        self.blocks(e, items, false);
        let mut acc = None;
        self.block_extent(id, &mut acc);
        if let Some(span) = acc {
            e.span(span);
        }
    }

    /// goldmark's extent of a container block: the union of its descendants'
    /// lines and text.
    fn block_extent(&self, id: NodeId, acc: &mut Option<Span>) {
        let mut tight = Vec::new();
        for &c in self.doc.children(id) {
            if !is_block(self.doc, c) {
                tight.push(c);
                continue;
            }
            let n = self.doc.node(c);
            let (s, t) = source_span(self.doc, c).unwrap_or_default();
            if n.is::<Paragraph>() {
                cover(acc, (s, self.trim_end(s, t)));
            } else if heading_level(self.doc, c).is_some() {
                let lines = self.heading_lines(c);
                if let (Some(first), Some(last)) = (lines.first(), lines.last()) {
                    cover(acc, (first.0, last.1));
                }
            } else if let Some(fence) = n.cast::<CodeFence>() {
                if let (_, _, Some(content)) = self.fence(c, fence) {
                    cover(acc, content);
                }
            } else if let Some(code) = n.cast::<CodeBlock>() {
                let start = self.content_start(s, code.as_str());
                cover(acc, (start, self.after_eol(self.trim_end(s, t))));
            } else if let Some(html) = n.cast::<HtmlBlock>() {
                let start = self.content_start(s, html.as_str());
                cover(acc, (start, self.after_eol(self.trim_end(s, t))));
            } else if let Some(container) = n.cast::<Container>() {
                cover(acc, (container.open.0, container.close.map_or(0, |c| c.1)));
                self.block_extent(c, acc);
            } else if let Some(leaf) = n.cast::<Leaf>() {
                cover(acc, (leaf.open.0, leaf.close.1));
            } else if n.is::<Table>() {
                let rows = self.table_rows(c);
                let columns = rows.first().map_or(0, |&r| self.doc.children(r).len());
                for (i, &row) in rows.iter().enumerate() {
                    if let Some((a, b)) = source_span(self.doc, row) {
                        for cell in self.row_cells(a, b, columns, i == 0).into_iter().flatten() {
                            cover(acc, cell);
                        }
                    }
                }
            } else if !n.is::<Definition>() {
                self.block_extent(c, acc);
            }
        }
        if let Some((s, t)) = self.run_extent(&tight) {
            cover(acc, (s, self.trim_end(s, t)));
        }
    }

    /// Where a code or HTML block's text starts: the blanks its content keeps
    /// before `first`, its first non-blank byte, without the spaces that
    /// stand in for a partly stripped tab.
    fn content_start(&self, first: usize, content: &str) -> usize {
        let blank = content
            .bytes()
            .take_while(|&c| c == b' ' || c == b'\t')
            .count();
        let kept = &content.as_bytes()[..blank];
        let pads = kept.iter().take_while(|&&c| c == b' ').count();
        (0..=pads)
            .find_map(|pad| {
                let start = first.checked_sub(blank - pad)?;
                (self.src.get(start..first) == Some(&kept[pad..])).then_some(start)
            })
            .unwrap_or(first)
    }

    /// goldmark's fence geometry: the block from its opening line, and the
    /// extent of its content lines.
    fn fence(&self, id: NodeId, fence: &CodeFence) -> (usize, usize, Option<Span>) {
        let (s, t) = source_span(self.doc, id).unwrap_or_default();
        let open = self.line_start(s);
        let open_end = self.line_end(open);
        let last_line = self.line_start(t.max(open));
        let closed = last_line > open && {
            let line = self
                .text((last_line, t))
                .trim_start_matches([' ', '>', '\t']);
            let marks = line.chars().take_while(|&c| c == fence.marker).count();
            marks >= fence.marker_len && line[marks..].trim().is_empty()
        };
        let content_end = if closed {
            last_line.saturating_sub(1)
        } else {
            self.line_end(t)
        };
        let has_lines = content_end > open_end;
        let last = if has_lines { content_end } else { open_end };
        let end = if last >= self.src.len() {
            self.src.len()
        } else {
            self.line_end(last + 1)
        };
        let content = has_lines.then(|| {
            let prefix = &self.src[open..s];
            let quotes = self.text((open, s)).matches('>').count();
            let indent =
                prefix.len() - prefix.iter().rposition(|&c| c == b'>').map_or(0, |p| p + 1);
            let mut at = open_end + 1;
            for _ in 0..quotes {
                at += self.src[at..].iter().take_while(|&&c| c == b' ').count();
                if self.src.get(at) == Some(&b'>') {
                    at += 1;
                    if self.src.get(at) == Some(&b' ') {
                        at += 1;
                    }
                }
            }
            let indent = if quotes > 0 {
                indent.saturating_sub(1)
            } else {
                indent
            };
            let spaces = self.src[at..].iter().take_while(|&&c| c == b' ').count();
            (
                at + spaces.min(indent),
                (content_end + 1).min(self.src.len()),
            )
        });
        (open, end, content)
    }

    fn child_inlines(&mut self, parent: &mut Element, id: NodeId) {
        for &child in self.doc.children(id) {
            self.inlines(parent, child);
        }
    }

    fn inlines(&mut self, parent: &mut Element, id: NodeId) {
        let n = self.doc.node(id);
        let mut e = if let Some(link) = n.cast::<Link>() {
            Element::new("link").attr("url", link.url.clone())
        } else if let Some(image) = n.cast::<Image>() {
            Element::new("image").attr("url", image.url.clone())
        } else if n.is::<Autolink>() {
            let mut e = Element::new("link");
            if let Some((s, t)) = source_span(self.doc, id) {
                let inner = (s + 1, t.saturating_sub(1).max(s + 1));
                e = e.attr("url", self.text(inner));
                e.span(inner);
                e.link = Some((s, t));
            }
            parent.children.push(e);
            return;
        } else if let Some(code) = n.cast::<CodeInline>() {
            let mut e = Element::new("code").attr("inline", "true");
            let inner = self.code_inner(id, code);
            let ticks = inner.map(|span| self.ticks(span));
            if let (Some(inner), Some(ticks)) = (inner, ticks) {
                e.span(ticks);
                self.text(inner).clone_into(&mut e.text);
                e = e.attr("ticks", (inner.0 - ticks.0).to_string());
            } else {
                e = e.attr("ticks", "0");
            }
            parent.children.push(e);
            return;
        } else if let Some(directive) = n.cast::<InlineDirective>() {
            let mut e = Element::new("directive")
                .attr("name", directive.name.clone())
                .attr("inline", "true");
            e.extend_attrs(&directive.attrs);
            if let Some(span) = source_span(self.doc, id) {
                e.span(span);
            }
            e.delimiters = directive::delimiters(self.doc, id);
            e.label = directive.claimed.clone().or_else(|| {
                e.delimiters
                    .map(|delimiters| self.text(delimiters.label()).to_owned())
            });
            self.child_inlines(&mut e, id);
            parent.children.push(e);
            return;
        } else {
            self.child_inlines(parent, id);
            return;
        };
        let mut acc = None;
        for &child in self.doc.children(id) {
            self.text_extent(child, &mut acc);
        }
        if let Some(span) = acc {
            e.span(span);
        }
        e.link = source_span(self.doc, id);
        self.child_inlines(&mut e, id);
        parent.children.push(e);
    }

    /// goldmark's inline extent: the union of text, code spans with their
    /// ticks and inline directives below a node.
    fn text_extent(&self, id: NodeId, acc: &mut Option<Span>) {
        let n = self.doc.node(id);
        if n.is::<Text>() || n.is::<TextSpecial>() || n.is::<InlineDirective>() {
            if let Some(span) = source_span(self.doc, id) {
                cover(acc, span);
            }
            return;
        }
        if let Some(code) = n.cast::<CodeInline>() {
            if let Some(inner) = self.code_inner(id, code) {
                cover(acc, self.ticks(inner));
            }
            return;
        }
        if n.is::<Autolink>() || n.is::<HtmlInline>() {
            return;
        }
        for &child in self.doc.children(id) {
            self.text_extent(child, acc);
        }
    }

    /// goldmark's code span content, one space trimmed from each side.
    fn code_inner(&self, id: NodeId, code: &CodeInline) -> Option<Span> {
        let (s, t) = source_span(self.doc, id)?;
        let (mut a, mut b) = (s + code.marker_len, t.saturating_sub(code.marker_len));
        if b <= a {
            return None;
        }
        let inner = &self.src[a..b];
        let padded = |c: Option<&u8>| matches!(c, Some(b' ' | b'\n'));
        if !inner.iter().all(|&c| is_space(c)) && padded(inner.first()) && padded(inner.last()) {
            a += 1;
            b -= 1;
        }
        Some((a, b))
    }

    fn ticks(&self, (mut s, mut e): Span) -> Span {
        while s > 0 && self.src[s - 1] == b'`' {
            s -= 1;
        }
        while self.src.get(e) == Some(&b'`') {
            e += 1;
        }
        (s, e)
    }

    fn table_rows(&self, id: NodeId) -> Vec<NodeId> {
        self.doc
            .children(id)
            .iter()
            .filter(|&&part| {
                let n = self.doc.node(part);
                n.is::<TableHead>() || n.is::<TableBody>()
            })
            .flat_map(|&part| self.doc.children(part))
            .copied()
            .filter(|&row| self.doc.node(row).is::<TableRow>())
            .collect()
    }

    /// A row per line, the header first, and a cell per column; a row spans
    /// its line from pipe to pipe, a cell its trimmed text.
    fn rows(&mut self, table: &mut Element, id: NodeId) {
        let rows = self.table_rows(id);
        let width = rows.first().map_or(0, |&r| self.doc.children(r).len());
        let mut columns: Vec<String> = Vec::new();
        let mut next = self.line_start(source_span(self.doc, id).map_or(0, |s| s.0));
        for (i, &row) in rows.iter().enumerate() {
            let mut r = Element::new("row")
                .attr("index", i.to_string())
                .attr("header", (i == 0).to_string());
            let (ls, le) = source_span(self.doc, row).unwrap_or((next, next));
            let cells: Vec<NodeId> = self
                .doc
                .children(row)
                .iter()
                .copied()
                .filter(|&c| self.doc.node(c).is::<TableCell>())
                .collect();
            for (j, extent) in self
                .row_cells(ls, le, width, i == 0)
                .into_iter()
                .enumerate()
            {
                let mut c = Element::new("cell").attr("index", j.to_string());
                if let Some(span) = extent {
                    c.span(span);
                    self.text(span).clone_into(&mut c.text);
                    r.start = r.start.or(Some(span.0));
                    r.end = span.1;
                }
                if i == 0 {
                    columns.push(c.text.clone());
                }
                if let Some(column) = columns.get(j) {
                    c = c.attr("column", column.clone());
                }
                if let Some(&cell) = cells.get(j) {
                    self.child_inlines(&mut c, cell);
                }
                r.children.push(c);
            }
            if let Some(start) = r.start {
                r.span(self.pipes(start, r.end));
            } else {
                r.span((next, self.line_end(next)));
            }
            for c in &mut r.children {
                if c.start.is_none() {
                    c.span((r.end, r.end));
                }
            }
            next = (self.line_end(r.end) + 1).min(self.src.len());
            if i == 0 {
                next = (self.line_end(next) + 1).min(self.src.len());
            }
            table.children.push(r);
        }
    }

    /// goldmark's `parseRow`: the trimmed extent of each cell of a row line.
    fn row_cells(&self, s: usize, t: usize, width: usize, header: bool) -> Vec<Option<Span>> {
        let a = self.trim_start(s, t);
        let line = &self.src[a..self.trim_end(a, t)];
        let mut pos = usize::from(line.first() == Some(&b'|'));
        let limit = line.len() - usize::from(line.len() > pos && line.last() == Some(&b'|'));
        let mut out = Vec::new();
        while pos < limit {
            if out.len() >= width && !header {
                return out;
            }
            let mut closure = pos;
            while closure < limit
                && !(line[closure] == b'|' && (closure == 0 || line[closure - 1] != b'\\'))
            {
                closure += 1;
            }
            let x = self.trim_start(a + pos, a + closure);
            out.push(Some((x, self.trim_end(x, a + closure))));
            pos = closure + 1;
        }
        out.resize(out.len().max(width), None);
        out
    }

    /// Widens a row's span over the blanks to its outer pipes.
    fn pipes(&self, mut start: usize, mut end: usize) -> Span {
        let before = self.src[..start]
            .iter()
            .rposition(|&c| c != b' ' && c != b'\t');
        if let Some(pipe) = before.filter(|&p| self.src[p] == b'|') {
            start = pipe;
        }
        let after = end
            + self.src[end..]
                .iter()
                .take_while(|&&c| c == b' ' || c == b'\t')
                .count();
        if self.src.get(after) == Some(&b'|') {
            end = after + 1;
        }
        (start, end)
    }

    /// henia's `finish`: fills unset extents from children, then columns,
    /// texts and lines.
    fn finish(&self, e: &mut Element) {
        let file = e.kind == "file";
        let section = e.kind == "section";
        for c in &mut e.children {
            self.finish(c);
            let start = c.start.unwrap_or_default();
            if e.start.is_none_or(|s| section && start < s) {
                e.start = Some(start);
            }
            if !file {
                e.end = e.end.max(c.end);
            }
        }
        let mut start = e.start.unwrap_or_else(|| {
            e.end = 0;
            0
        });
        e.end = e.end.max(start);
        e.column = start
            - self.src[..start]
                .iter()
                .rposition(|&c| c == b'\n')
                .map_or(0, |p| p + 1)
            + 1;
        if section || e.kind == "heading" {
            start = self.line_start(start);
        }
        e.start = Some(start);
        if e.text.is_empty() && !file {
            self.text((start, e.end))
                .trim_end_matches('\n')
                .clone_into(&mut e.text);
        }
        e.line = self.newlines.partition_point(|&x| x < start) + 1;
        e.end_line = self
            .newlines
            .partition_point(|&x| x < start.max(e.end.saturating_sub(1)))
            + 1;
    }
}

/// goldmark's id of a heading line: ASCII letters and digits lowercased,
/// blanks, `-` and `_` as `-`, everything else dropped.
fn slug(value: &str) -> String {
    let value = value.trim_matches(|c: char| c.is_ascii() && u8::try_from(c).is_ok_and(is_space));
    let id: String = value
        .chars()
        .filter(char::is_ascii)
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c == '-' || c == '_' || u8::try_from(c).is_ok_and(is_space) {
                Some('-')
            } else {
                None
            }
        })
        .collect();
    if id.is_empty() {
        "heading".to_owned()
    } else {
        id
    }
}

fn unique_id(seen: &mut HashSet<String>, id: String) -> String {
    if seen.insert(id.clone()) {
        return id;
    }
    let mut n = 1;
    loop {
        let candidate = format!("{id}-{n}");
        if seen.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

fn at_path<'e>(mut e: &'e Element, path: &[usize]) -> &'e Element {
    for &i in path {
        e = &e.children[i];
    }
    e
}

fn at_path_mut<'e>(mut e: &'e mut Element, path: &[usize]) -> &'e mut Element {
    for &i in path {
        e = &mut e.children[i];
    }
    e
}
