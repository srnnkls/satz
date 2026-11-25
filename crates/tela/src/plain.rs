//! The plain-text frontend: paragraphs separated by blank lines.

use std::sync::Arc;

use crate::frontend::Frontend;
use crate::tree::{Node, Origin, Position, Tree};

/// Reads any text as paragraphs, each a run of non-blank lines. It claims no
/// extension; a registry names it as the fallback for files no other
/// frontend claims.
#[derive(Debug, Clone, Copy, Default)]
pub struct Plain;

impl Frontend for Plain {
    fn name(&self) -> &'static str {
        "plain"
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }

    fn parse(&self, file: &str, text: &str) -> Tree {
        let file: Arc<str> = Arc::from(file);
        let mut tree = Tree::new(Node::new("file", Origin::named(&file, None)));
        let root = tree.root().id();
        for span in Paragraphs::new(text) {
            let position = position(text, span);
            let mut node = Node::new("paragraph", Origin::named(&file, position.clone()));
            text[position.as_ref().map_or(0..0, Position::span)]
                .trim_end_matches('\n')
                .clone_into(&mut node.text);
            node.position = position;
            if tree.append(root, node).is_err() {
                break;
            }
        }
        tree
    }
}

/// The paragraph spans of a text: from a paragraph's first byte to the start
/// of the blank line after it, or to the end of the text.
struct Paragraphs<'a> {
    text: &'a str,
    offset: usize,
}

impl<'a> Paragraphs<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, offset: 0 }
    }
}

impl Iterator for Paragraphs<'_> {
    type Item = std::ops::Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut start = None;
        while self.offset <= self.text.len() {
            let offset = self.offset;
            let end = self.text[offset..]
                .find('\n')
                .map_or(self.text.len(), |n| offset + n);
            self.offset = end + 1;
            let blank = self.text[offset..end].trim().is_empty();
            match start {
                None if !blank => start = Some(offset),
                Some(start) if blank => return Some(start..offset),
                _ => {}
            }
        }
        start.map(|start| start..self.text.len())
    }
}

fn position(text: &str, span: std::ops::Range<usize>) -> Option<Position> {
    let lines = |at: usize| u32::try_from(text[..at].matches('\n').count() + 1).unwrap_or(u32::MAX);
    let line_start = text[..span.start].rfind('\n').map_or(0, |n| n + 1);
    let column = u32::try_from(span.start - line_start + 1).unwrap_or(u32::MAX);
    let lines = lines(span.start)..=lines(span.start.max(span.end.saturating_sub(1)));
    Position::new(span, lines, column).ok()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::frontend::Frontends;
    use crate::markdown::Markdown;

    fn paragraphs(text: &str) -> Vec<String> {
        Plain
            .parse("notes.txt", text)
            .root()
            .children()
            .map(|p| {
                let node = p.node();
                let at = node.origin.position().unwrap();
                format!("{:?} L{}-{}", node.text, at.line(), at.end_line())
            })
            .collect()
    }

    #[test]
    fn blank_lines_separate_paragraphs() {
        assert_eq!(
            paragraphs("a\nb\n\n\nc\n"),
            [r#""a\nb" L1-2"#, r#""c" L5-5"#]
        );
    }

    #[test]
    fn markup_is_plain_text() {
        assert_eq!(
            paragraphs("# no heading\n  \n[no](link)"),
            [r##""# no heading" L1-1"##, r#""[no](link)" L3-3"#]
        );
        assert_eq!(paragraphs(""), Vec::<String>::new());
        assert_eq!(paragraphs("\n \n"), Vec::<String>::new());
    }

    #[test]
    fn plain_is_the_fallback_beside_markdown() {
        let frontends = Frontends::new(
            vec![Arc::new(Markdown::new()), Arc::new(Plain)],
            Some("plain"),
        )
        .unwrap();
        let name = |path: &str| frontends.for_named(Path::new(path)).map(Frontend::name);
        assert_eq!(name("docs/a.md"), Some("markdown"));
        assert_eq!(name("notes.txt"), Some("plain"));
        assert_eq!(
            frontends
                .by_path(Path::new("notes.txt"))
                .map(Frontend::name),
            None
        );
    }
}
