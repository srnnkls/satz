use std::fmt;
use std::ops::{Range, RangeInclusive};
use std::sync::Arc;

/// A validated span of text: byte offsets plus 1-based lines and column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Position {
    span: Range<usize>,
    lines: RangeInclusive<u32>,
    column: u32,
}

/// Why a [`Position`] was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PositionError {
    /// The span ends before it starts.
    #[error("span {start}..{end} ends before it starts")]
    Span {
        /// The start offset.
        start: usize,
        /// The end offset.
        end: usize,
    },
    /// A line or the column is 0, or the last line precedes the first.
    #[error("lines {first}..={last} at column {column} are not 1-based and ordered")]
    Lines {
        /// The first line.
        first: u32,
        /// The last line.
        last: u32,
        /// The column.
        column: u32,
    },
}

impl Position {
    /// A position over the byte `span`, on `lines`, starting at `column`.
    ///
    /// # Errors
    /// Returns [`PositionError::Span`] when the span ends before it starts,
    /// and [`PositionError::Lines`] when a line or the column is 0 or the
    /// last line precedes the first.
    pub fn new(
        span: Range<usize>,
        lines: RangeInclusive<u32>,
        column: u32,
    ) -> Result<Self, PositionError> {
        if span.end < span.start {
            return Err(PositionError::Span {
                start: span.start,
                end: span.end,
            });
        }
        let (first, last) = (*lines.start(), *lines.end());
        if first == 0 || column == 0 || last < first {
            return Err(PositionError::Lines {
                first,
                last,
                column,
            });
        }
        Ok(Self {
            span,
            lines,
            column,
        })
    }

    /// The byte span.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }

    /// The first line, 1-based.
    #[must_use]
    pub fn line(&self) -> u32 {
        *self.lines.start()
    }

    /// The last line, 1-based.
    #[must_use]
    pub fn end_line(&self) -> u32 {
        *self.lines.end()
    }

    /// The column of the start, 1-based.
    #[must_use]
    pub fn column(&self) -> u32 {
        self.column
    }

    /// The position moved down by `lines` lines, as when a caller strips
    /// frontmatter of that many lines; span and column stay.
    #[must_use]
    pub fn shifted(&self, lines: u32) -> Self {
        Self {
            span: self.span.clone(),
            lines: self.line().saturating_add(lines)..=self.end_line().saturating_add(lines),
            column: self.column,
        }
    }

    /// The position moved down `lines` lines and on `bytes` bytes, as a
    /// body's position becomes its file's past a header of that size.
    #[must_use]
    pub fn moved(&self, lines: u32, bytes: usize) -> Self {
        let mut moved = self.shifted(lines);
        moved.span = self.span.start + bytes..self.span.end + bytes;
        moved
    }
}

/// Where a node comes from: its file and its position in that file's source.
///
/// Every position satz reports reads the origin; a node's own
/// [`Node::position`](super::Node::position) stays relative to the parsed text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Origin {
    file: Arc<str>,
    position: Option<Position>,
}

/// An origin that names no file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("origin names no file")]
pub struct EmptyFile;

impl Origin {
    /// An origin in `file`, with no position for nodes that span the whole
    /// file.
    ///
    /// # Errors
    /// Returns [`EmptyFile`] when `file` is empty.
    pub fn new(file: impl Into<Arc<str>>, position: Option<Position>) -> Result<Self, EmptyFile> {
        let file = file.into();
        if file.is_empty() {
            return Err(EmptyFile);
        }
        Ok(Self { file, position })
    }

    /// An origin in `file`, or in `-` when `file` is empty, as for text a
    /// frontend parses without a name.
    pub(crate) fn named(file: &Arc<str>, position: Option<Position>) -> Self {
        let file = if file.is_empty() {
            Arc::from("-")
        } else {
            Arc::clone(file)
        };
        Self { file, position }
    }

    /// The file, as the caller named it.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// An origin in the same file at `position`; the file name is shared,
    /// not copied.
    #[must_use]
    pub fn with_position(&self, position: Option<Position>) -> Self {
        Self {
            file: Arc::clone(&self.file),
            position,
        }
    }

    /// The position in the source, absent for a whole-file node.
    #[must_use]
    pub fn position(&self) -> Option<&Position> {
        self.position.as_ref()
    }

    /// The first source line, absent for a whole-file node.
    #[must_use]
    pub fn line(&self) -> Option<u32> {
        self.position.as_ref().map(Position::line)
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.position {
            Some(at) => write!(f, "{}:{}:{}", self.file, at.line(), at.column()),
            None => f.write_str(&self.file),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_rejects_reversed_spans() {
        assert_eq!(
            Position::new(Range { start: 5, end: 4 }, 1..=1, 1),
            Err(PositionError::Span { start: 5, end: 4 })
        );
    }

    #[test]
    fn position_rejects_zero_and_reversed_lines() {
        assert!(Position::new(0..1, 0..=1, 1).is_err());
        assert!(Position::new(0..1, 1..=1, 0).is_err());
        assert!(Position::new(0..1, RangeInclusive::new(3, 2), 1).is_err());
    }

    #[test]
    fn shifted_moves_lines_only() {
        let at = Position::new(40..52, 10..=11, 4).unwrap().shifted(4);
        assert_eq!(
            (at.span(), at.line(), at.end_line(), at.column()),
            (40..52, 14, 15, 4)
        );
    }

    #[test]
    fn origin_requires_a_file_and_prints_its_location() {
        assert_eq!(Origin::new("", None), Err(EmptyFile));
        let origin =
            Origin::new("docs/a.md", Some(Position::new(0..3, 14..=14, 2).unwrap())).unwrap();
        assert_eq!(origin.line(), Some(14));
        assert_eq!(origin.to_string(), "docs/a.md:14:2");
        assert_eq!(
            Origin::new("docs/a.md", None).unwrap().to_string(),
            "docs/a.md"
        );
    }
}
