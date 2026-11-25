//! Directive syntax as markdown-it rules: `:::name{attrs}` containers closed
//! by a colon fence, `::name[label]{attrs}` leaves and `:name[label]{attrs}`
//! inline directives.

use markdown_it::MarkdownIt;
use markdown_it::document::{Document, NodeId, Root};
use markdown_it::parser::block::{BlockRule, BlockState};
use markdown_it::parser::inline::{InlineRule, InlineState};
use markdown_it::plugins::cmark::block::blockquote::Blockquote;
use markdown_it::plugins::cmark::block::fence::CodeFence;
use markdown_it::plugins::cmark::block::list::{BulletList, ListItem, OrderedList};
use markdown_it::plugins::html::html_block::HtmlBlock;

use super::attributes::{Attrs, Fault, name_length, skip_blanks, tokenize};

/// A byte range of the parsed text.
pub(super) type Span = (usize, usize);

/// The span markdown-it maps node `id` to, clamped to the parsed text.
pub(super) fn source_span(doc: &Document, id: NodeId) -> Option<Span> {
    let (start, end) = doc.node(id).srcmap()?.get_byte_offsets();
    let end = end.min(doc.source().len());
    Some((start.min(end), end))
}

/// Block-level faults at offsets of the parsed text.
#[derive(Debug, Default)]
struct BlockFaults(Vec<Fault>);

/// A `:::name` container; `close` is absent while it is unclosed.
#[derive(Debug)]
pub(super) struct Container {
    pub(super) name: String,
    pub(super) attrs: Attrs,
    pub(super) open: Span,
    pub(super) close: Option<Span>,
    fence: usize,
    end_line: usize,
}

/// A `::name[label]` leaf: `open` runs to the label, `close` from it.
#[derive(Debug)]
pub(super) struct Leaf {
    pub(super) name: String,
    pub(super) attrs: Attrs,
    pub(super) open: Span,
    pub(super) close: Span,
    labeled: bool,
}

/// A `:name[label]` inline directive; its label span and faults are
/// relative to its start.
#[derive(Debug)]
pub(super) struct InlineDirective {
    pub(super) name: String,
    pub(super) attrs: Attrs,
    label: Span,
    faults: Vec<Fault>,
}

/// Where a directive's delimiters sit in the parsed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Delimiters {
    /// The colons of the fence: none for an inline directive, two for a leaf.
    pub(super) fence: usize,
    /// The opening delimiter: up to the label, or the opening line.
    pub(super) open: Span,
    /// The closing delimiter: from the label, or the closing line; absent
    /// while a container is unclosed.
    pub(super) close: Option<Span>,
    /// Whether a leaf or inline directive has a `[label]`.
    pub(super) labeled: bool,
}

impl Delimiters {
    /// The label of a leaf or inline directive, between its delimiters.
    pub(super) fn label(&self) -> Span {
        (self.open.1, self.close.map_or(self.open.1, |close| close.0))
    }
}

/// The name and attributes of node `id`, if it is a directive.
pub(super) fn signature(doc: &Document, id: NodeId) -> Option<(&str, &Attrs)> {
    let node = doc.node(id);
    if let Some(container) = node.cast::<Container>() {
        return Some((&container.name, &container.attrs));
    }
    if let Some(leaf) = node.cast::<Leaf>() {
        return Some((&leaf.name, &leaf.attrs));
    }
    node.cast::<InlineDirective>()
        .map(|directive| (directive.name.as_str(), &directive.attrs))
}

/// The delimiters of node `id`, if it is a directive.
pub(super) fn delimiters(doc: &Document, id: NodeId) -> Option<Delimiters> {
    let node = doc.node(id);
    if let Some(container) = node.cast::<Container>() {
        return Some(Delimiters {
            fence: container.fence,
            open: container.open,
            close: container.close,
            labeled: false,
        });
    }
    if let Some(leaf) = node.cast::<Leaf>() {
        return Some(Delimiters {
            fence: 2,
            open: leaf.open,
            close: Some(leaf.close),
            labeled: leaf.labeled,
        });
    }
    let directive = node.cast::<InlineDirective>()?;
    let (start, end) = source_span(doc, id)?;
    Some(Delimiters {
        fence: 0,
        open: (start, start + directive.label.0),
        close: Some((start + directive.label.1, end)),
        labeled: true,
    })
}

/// A failed inline directive: it consumes its `:` and carries its faults,
/// relative to its start.
#[derive(Debug)]
pub(super) struct InlineFailure {
    faults: Vec<Fault>,
}

pub(super) fn add(md: &mut MarkdownIt) {
    md.block
        .add_rule::<DirectiveBlock>()
        .after_named("blockquote");
    add_inline(md);
}

/// Adds inline directives alone, as directive labels are read.
pub(super) fn add_inline(md: &mut MarkdownIt) {
    md.inline.add_rule::<DirectiveInline>();
}

/// The faults of a parsed document: the block faults and those of inline
/// directives outside every inline label, in source order, else the first
/// fault inside a label, as rendering the labels finds them.
pub(super) fn faults(doc: &Document) -> Vec<Fault> {
    let mut top = doc
        .value::<Root>(doc.root())
        .and_then(|root| root.ext.get::<BlockFaults>())
        .map(|faults| faults.0.clone())
        .unwrap_or_default();
    let mut labels = Vec::new();
    level_faults(doc, doc.root(), &mut top, &mut labels);
    top.sort_by_key(|fault| fault.offset);
    if !top.is_empty() {
        return top;
    }
    labels
        .into_iter()
        .find_map(|label| first_label_fault(doc, label))
        .into_iter()
        .collect()
}

/// Collects the faults of `id`'s subtree that belong to its label level, and
/// the inline directives whose labels open the next level.
fn level_faults(doc: &Document, id: NodeId, out: &mut Vec<Fault>, labels: &mut Vec<NodeId>) {
    for &child in doc.children(id) {
        let node = doc.node(child);
        let base = source_span(doc, child).map_or(0, |span| span.0);
        if let Some(failure) = node.cast::<InlineFailure>() {
            out.extend(failure.faults.iter().cloned().map(|f| f.shifted(base)));
        } else if let Some(directive) = node.cast::<InlineDirective>() {
            out.extend(directive.faults.iter().cloned().map(|f| f.shifted(base)));
            labels.push(child);
        } else {
            level_faults(doc, child, out, labels);
        }
    }
}

fn first_label_fault(doc: &Document, directive: NodeId) -> Option<Fault> {
    let mut faults = Vec::new();
    let mut labels = Vec::new();
    level_faults(doc, directive, &mut faults, &mut labels);
    faults
        .into_iter()
        .min_by_key(|fault| fault.offset)
        .or_else(|| {
            labels
                .into_iter()
                .find_map(|label| first_label_fault(doc, label))
        })
}

/// goldmark's `FindClosure` with code spans and nesting: the offset of the
/// `closer` that balances an `opener` before `from`, on one line.
pub(super) fn find_closure(line: &[u8], from: usize, opener: u8, closer: u8) -> Option<usize> {
    let mut depth = 1;
    let mut code = 0;
    let mut i = from;
    while let Some(&byte) = line.get(i) {
        if byte == b'`' {
            let run = line[i..].iter().take_while(|&&tick| tick == b'`').count();
            if code == 0 {
                code = run;
            } else if run == code {
                code = 0;
            }
            i += run;
            continue;
        }
        if code == 0 {
            if byte == b'\\' && line.get(i + 1).is_some_and(u8::is_ascii_punctuation) {
                i += 2;
                continue;
            }
            if byte == closer {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            } else if byte == opener {
                depth += 1;
            }
        }
        i += 1;
    }
    None
}

fn record(state: &mut BlockState<'_>, base: usize, faults: impl IntoIterator<Item = Fault>) {
    let list = &mut state.root_ext.get_or_insert_default::<BlockFaults>().0;
    for fault in faults {
        let fault = fault.shifted(base);
        if !list.contains(&fault) {
            list.push(fault);
        }
    }
}

fn fault_count(state: &mut BlockState<'_>) -> usize {
    state
        .root_ext
        .get_or_insert_default::<BlockFaults>()
        .0
        .len()
}

/// Drops an attempted node and the faults recorded since `saved`.
fn discard(state: &mut BlockState<'_>, node: NodeId, saved: usize) {
    state.document.discard_node(node);
    state
        .root_ext
        .get_or_insert_default::<BlockFaults>()
        .0
        .truncate(saved);
}

struct Opening {
    name: String,
    attrs: Attrs,
    fence: usize,
    faults: Vec<Fault>,
}

/// Reads a container opening line, from its first non-space byte.
fn opening(line: &[u8]) -> Option<Opening> {
    let fence = line.iter().take_while(|&&c| c == b':').count();
    if fence < 3 {
        return None;
    }
    let i = skip_blanks(line, fence);
    let n = name_length(&line[i..]);
    if n == 0 {
        return None;
    }
    let name = String::from_utf8_lossy(&line[i..i + n]).into_owned();
    let i = skip_blanks(line, i + n);
    let rest = line[i..].trim_ascii_end();
    let (attrs, faults) = if rest.is_empty() {
        (Attrs::new(), Vec::new())
    } else {
        match tokenize(rest) {
            Ok(mut tokens) => {
                if tokens.consumed < rest.len() {
                    tokens.faults.push(Fault::new(
                        tokens.consumed,
                        "unexpected text after directive attributes",
                    ));
                }
                (tokens.attrs, tokens.faults)
            }
            Err(faults) => (Attrs::new(), faults),
        }
    };
    let faults = faults.into_iter().map(|f| f.shifted(i)).collect();
    Some(Opening {
        name,
        attrs,
        fence,
        faults,
    })
}

struct LeafLine {
    name: String,
    attrs: Attrs,
    label: Span,
    labeled: bool,
    faults: Vec<Fault>,
}

/// Reads a leaf line, from its first non-space byte; a line that is no leaf
/// returns the faults it reports.
fn leaf_line(line: &[u8]) -> Result<LeafLine, Vec<Fault>> {
    let mut i = 2;
    let n = name_length(&line[i..]);
    if n == 0 {
        return Err(Vec::new());
    }
    let name = String::from_utf8_lossy(&line[i..i + n]).into_owned();
    i += n;
    let mut label = (i, i);
    let mut labeled = false;
    if line.get(i) == Some(&b'[') {
        let close = find_closure(line, i + 1, b'[', b']').ok_or_else(Vec::new)?;
        labeled = true;
        label = (i + 1, close);
        i = close + 1;
    }
    let mut attrs = Attrs::new();
    let mut faults = Vec::new();
    let attr_start = i;
    if line.get(i) == Some(&b'{') {
        match tokenize(&line[i..]) {
            Ok(tokens) => {
                attrs = tokens.attrs;
                faults = tokens.faults;
                i += tokens.consumed;
            }
            Err(found) if !labeled => {
                return Err(found.into_iter().map(|f| f.shifted(i)).collect());
            }
            Err(_) => return Err(Vec::new()),
        }
    }
    if !line[i..].iter().all(u8::is_ascii_whitespace) {
        return Err(Vec::new());
    }
    Ok(LeafLine {
        name,
        attrs,
        label,
        labeled,
        faults: faults.into_iter().map(|f| f.shifted(attr_start)).collect(),
    })
}

fn is_closer(line: &str, fence: usize) -> bool {
    let line = line.trim();
    line.len() >= fence && line.bytes().all(|c| c == b':')
}

struct DirectiveBlock;

impl DirectiveBlock {
    fn current(state: &BlockState<'_>) -> (usize, Vec<u8>) {
        let base = state.line_offsets[state.line].first_nonspace;
        (base, state.get_line(state.line).as_bytes().to_vec())
    }

    fn leaf(state: &mut BlockState<'_>) -> Option<(Option<NodeId>, usize)> {
        let (base, line) = Self::current(state);
        let leaf = match leaf_line(&line) {
            Ok(leaf) => leaf,
            Err(faults) => {
                record(state, base, faults);
                return None;
            }
        };
        record(state, base, leaf.faults);
        let node = state.document.create_node(Leaf {
            name: leaf.name,
            attrs: leaf.attrs,
            open: (base, base + leaf.label.0),
            close: (base + leaf.label.1, base + line.len()),
            labeled: leaf.labeled,
        });
        if leaf.label.1 > leaf.label.0 {
            let content = String::from_utf8_lossy(&line[leaf.label.0..leaf.label.1]).into_owned();
            let pending = state.pending_inline(content, vec![(0, base + leaf.label.0)]);
            state.document.push_child(node, pending);
        }
        Some((Some(node), 1))
    }

    fn container(state: &mut BlockState<'_>, opening: Opening) -> (Option<NodeId>, usize) {
        let start_line = state.line;
        let offsets = &state.line_offsets[start_line];
        let open = (offsets.first_nonspace, offsets.line_end);
        record(state, open.0, opening.faults);
        let attempt = |state: &mut BlockState<'_>, max: usize| -> (NodeId, usize) {
            let node = state.document.create_node(Container {
                name: opening.name.clone(),
                attrs: opening.attrs.clone(),
                open,
                close: None,
                fence: opening.fence,
                end_line: 0,
            });
            let outer = std::mem::replace(&mut state.node, node);
            let outer_max = std::mem::replace(&mut state.line_max, max);
            state.line = start_line + 1;
            state.tokenize_nested();
            let end = state.line;
            state.line = start_line;
            state.line_max = outer_max;
            state.node = outer;
            (node, end)
        };
        let saved = fault_count(state);
        let (probe, boundary) = attempt(state, state.line_max);
        discard(state, probe, saved);
        let candidates = (start_line + 1..boundary).filter(|&l| {
            !state.is_empty(l)
                && (0..4).contains(&state.line_indent(l))
                && is_closer(state.get_line(l), opening.fence)
        });
        for candidate in candidates.collect::<Vec<_>>() {
            let saved = fault_count(state);
            let (node, _) = attempt(state, candidate);
            if literal_open(state, node, candidate) {
                discard(state, node, saved);
                continue;
            }
            let offsets = &state.line_offsets[candidate];
            let close = (offsets.first_nonspace, offsets.line_end);
            if let Some(container) = state.document.value_mut::<Container>(node) {
                container.close = Some(close);
                container.end_line = candidate + 1;
            }
            return (Some(node), candidate + 1 - start_line);
        }
        let (node, end) = attempt(state, state.line_max);
        let end = end.max(start_line + 1);
        if let Some(container) = state.document.value_mut::<Container>(node) {
            container.end_line = end;
        }
        let message = format!("directive {} is not closed", opening.name);
        record(
            state,
            open.0,
            [Fault::with_id(0, "unclosed-directive", message)],
        );
        (Some(node), end - start_line)
    }
}

/// Whether the last open block of `node`'s subtree is a literal block, or an
/// unclosed nested container, still open at line `stop`, so that the line
/// belongs to it rather than closing `node`.
fn literal_open(state: &BlockState<'_>, node: NodeId, stop: usize) -> bool {
    let before = state.line_offsets[stop - 1].line_end;
    let mut id = node;
    loop {
        let Some(&last) = state.document.children(id).last() else {
            return false;
        };
        let n = state.document.node(last);
        let span = source_span(state.document, last);
        if let Some(container) = n.cast::<Container>() {
            if container.close.is_some() {
                return false;
            }
            if container.end_line == stop {
                return true;
            }
        } else if let Some(fence) = n.cast::<CodeFence>() {
            return span.is_some_and(|span| span.1 == before && fence_open(state.src, span, fence));
        } else if n.is::<HtmlBlock>() {
            return span.is_some_and(|span| {
                span.1 == before && state.src.get(span.0..span.1).is_some_and(html_open)
            });
        } else if !(n.is::<Blockquote>()
            || n.is::<BulletList>()
            || n.is::<OrderedList>()
            || n.is::<ListItem>())
        {
            return false;
        }
        id = last;
    }
}

/// Whether a fence ending at `span` lacks its closing line.
fn fence_open(src: &str, (start, end): Span, fence: &CodeFence) -> bool {
    let Some(head) = src.get(..end) else {
        return true;
    };
    let last = head.rfind('\n').map_or(0, |p| p + 1).max(start);
    if last == start {
        return true;
    }
    let Some(line) = head.get(last..) else {
        return true;
    };
    let line = line.trim_start_matches([' ', '>']).trim_start();
    let marks = line.chars().take_while(|&c| c == fence.marker).count();
    !(marks >= fence.marker_len && line[marks..].trim().is_empty())
}

/// Whether an HTML block lacks the end condition its start condition needs.
fn html_open(text: &str) -> bool {
    let first = text.lines().next().unwrap_or_default().to_ascii_lowercase();
    let last = text.lines().last().unwrap_or_default().to_ascii_lowercase();
    let closers: &[&str] = if first.starts_with("<!--") {
        &["-->"]
    } else if first.starts_with("<?") {
        &["?>"]
    } else if first.starts_with("<![cdata[") {
        &["]]>"]
    } else if first.starts_with("<!") {
        &[">"]
    } else if ["<script", "<pre", "<style", "<textarea"]
        .iter()
        .any(|tag| first.starts_with(tag))
    {
        &["</script>", "</pre>", "</style>", "</textarea>"]
    } else {
        return true;
    };
    !closers.iter().any(|closer| last.contains(closer))
}

impl BlockRule for DirectiveBlock {
    const MARKERS: &'static [char] = &[':'];
    const NAMES: &'static [&'static str] = &["directive"];

    fn check(state: &mut BlockState<'_>) -> Option<()> {
        if !(0..4).contains(&state.line_indent(state.line)) {
            return None;
        }
        let (base, line) = Self::current(state);
        if line.iter().take_while(|&&c| c == b':').count() == 2 {
            return match leaf_line(&line) {
                Ok(_) => Some(()),
                Err(faults) => {
                    record(state, base, faults);
                    None
                }
            };
        }
        opening(&line).map(|_| ())
    }

    fn run(state: &mut BlockState<'_>) -> Option<(Option<NodeId>, usize)> {
        if state.line_indent(state.line) >= 4 {
            return None;
        }
        let (_, line) = Self::current(state);
        if line.iter().take_while(|&&c| c == b':').count() == 2 {
            return Self::leaf(state);
        }
        let opening = opening(&line)?;
        Some(Self::container(state, opening))
    }
}

struct DirectiveInline;

impl InlineRule for DirectiveInline {
    const MARKER: char = ':';
    const NAMES: &'static [&'static str] = &["directive_inline"];

    fn run(state: &mut InlineState<'_>) -> Option<(Option<NodeId>, usize)> {
        let rest = state.remaining();
        let line = rest.as_bytes()[..rest.find('\n').map_or(rest.len(), |p| p + 1)].to_vec();
        let length = name_length(&line[1..]);
        if length == 0 || line.get(length + 1) != Some(&b'[') {
            return None;
        }
        let start = length + 2;
        let Some(close) = find_closure(&line, start, b'[', b']') else {
            let node = state.document.create_node(InlineFailure {
                faults: vec![Fault::new(0, "unclosed inline directive")],
            });
            return Some((Some(node), 1));
        };
        let end = close + 1;
        let name = String::from_utf8_lossy(&line[1..=length]).into_owned();
        let spaces = skip_blanks(&line, end);
        let mut attrs = Attrs::new();
        let mut faults = Vec::new();
        let mut stop = end;
        if line.get(spaces) == Some(&b'{') {
            match tokenize(&line[spaces..]) {
                Ok(tokens) => {
                    faults = tokens
                        .faults
                        .into_iter()
                        .map(|f| f.shifted(spaces))
                        .collect();
                    attrs = tokens.attrs;
                    stop = spaces + tokens.consumed;
                }
                Err(found) => {
                    let faults = found.into_iter().map(|f| f.shifted(spaces)).collect();
                    let node = state.document.create_node(InlineFailure { faults });
                    return Some((Some(node), 1));
                }
            }
        }
        let children = state.parse_subrange(start..close).unwrap_or_default();
        let node = state.document.create_node(InlineDirective {
            name,
            attrs,
            label: (start, close),
            faults,
        });
        for child in children {
            state.document.push_child(node, child);
        }
        Some((Some(node), stop))
    }
}
