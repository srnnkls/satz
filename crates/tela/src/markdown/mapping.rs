//! Paragraph and setext heading rules whose inline text maps onto the source
//! where lines strip only part of a tab.

use std::marker::PhantomData;
use std::ops::Range;

use markdown_it::MarkdownIt;
use markdown_it::document::NodeId;
use markdown_it::parser::block::{BlockRule, BlockState};
use markdown_it::parser::inline::InlineMapping;
use markdown_it::plugins::cmark::block::lheading::LHeadingScanner;
use markdown_it::plugins::cmark::block::paragraph::ParagraphScanner;

/// Replaces markdown-it's paragraph and setext heading rules with remapped
/// ones, in the same order.
pub(super) fn add(md: &mut MarkdownIt) {
    md.block.remove_rule::<LHeadingScanner>();
    md.block.remove_rule::<ParagraphScanner>();
    md.block
        .add_rule::<Remapped<ParagraphScanner>>()
        .after_all();
    md.block
        .add_rule::<Remapped<LHeadingScanner>>()
        .before::<Remapped<ParagraphScanner>>()
        .after_all();
}

/// A block rule whose node holds the inline text of its leading lines.
trait TextLines: BlockRule {
    /// How many of the `consumed` lines of a match are inline text.
    fn text_lines(consumed: usize) -> usize;
}

impl TextLines for ParagraphScanner {
    fn text_lines(consumed: usize) -> usize {
        consumed
    }
}

impl TextLines for LHeadingScanner {
    fn text_lines(consumed: usize) -> usize {
        consumed.saturating_sub(1)
    }
}

/// `R`, with its inline text rebuilt under a corrected mapping.
struct Remapped<R>(PhantomData<fn() -> R>);

impl<R: TextLines> BlockRule for Remapped<R> {
    const MARKERS: &'static [char] = R::MARKERS;
    const NAMES: &'static [&'static str] = R::NAMES;

    fn check(state: &mut BlockState<'_>) -> Option<()> {
        R::check(state)
    }

    fn run(state: &mut BlockState<'_>) -> Option<(Option<NodeId>, usize)> {
        let line = state.line;
        let (node, consumed) = R::run(state)?;
        if let Some(node) = node {
            remap(state, node, line..line + R::text_lines(consumed));
        }
        Some((node, consumed))
    }
}

/// Replaces the pending inline text of `node`, read from `lines`, when its
/// mapping needs correcting.
fn remap(state: &mut BlockState<'_>, node: NodeId, lines: Range<usize>) {
    let (content, mapping) = state.get_lines(lines.start, lines.end, state.blk_indent, false);
    let Some(mapping) = corrected(state.src, &content, &mapping) else {
        return;
    };
    for child in state.document.take_children(node) {
        state.document.discard_node(child);
    }
    let pending = state.pending_inline(content, mapping);
    state.document.push_child(node, pending);
}

/// The pinned fork's `get_lines` maps each line from before the spaces that
/// stand in for a partly stripped tab, so the text after them maps that many
/// bytes too far. Here those spaces map to the tab and the text to itself;
/// `None` when no line has such spaces.
fn corrected(src: &str, content: &str, mapping: &[(usize, usize)]) -> Option<InlineMapping> {
    let spaces = |text: &str, at: usize| {
        text.as_bytes()
            .get(at..)
            .unwrap_or_default()
            .iter()
            .take_while(|&&c| c == b' ')
            .count()
    };
    let mut padded = false;
    let mut out = Vec::with_capacity(mapping.len());
    for &(at, from) in mapping {
        let pad = spaces(content, at).saturating_sub(spaces(src, from));
        padded |= pad > 0;
        let tab = from.saturating_sub(1);
        out.extend((at..at + pad).map(|space| (space, tab)));
        out.push((at + pad, from));
    }
    padded.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpadded_lines_keep_their_mapping() {
        assert_eq!(corrected("- a\n  b", "a\nb", &[(0, 2), (2, 6)]), None);
    }

    #[test]
    fn spaces_for_a_tab_map_to_it() {
        assert_eq!(
            corrected("- a\n \t}", "a\n  }", &[(0, 2), (2, 6)]),
            Some(vec![(0, 2), (2, 5), (3, 5), (4, 6)])
        );
    }
}
