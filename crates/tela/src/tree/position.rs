use std::sync::Arc;

/// A validated span of text: byte offsets plus 1-based line and column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    start: usize,
    end: usize,
    line: u32,
    end_line: u32,
    column: u32,
}

/// Why a [`Position`] or [`Origin`] was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PositionError {
    /// The end offset lies before the start offset.
    #[error("end offset {end} precedes start offset {start}")]
    Offsets {
        /// The start offset.
        start: usize,
        /// The end offset.
        end: usize,
    },
    /// A line or column is 0, or the end line precedes the start line.
    #[error("lines {line}..{end_line} at column {column} are not 1-based and ordered")]
    Lines {
        /// The first line.
        line: u32,
        /// The last line.
        end_line: u32,
        /// The column.
        column: u32,
    },
    /// An origin names no file.
    #[error("origin names no file")]
    EmptyFile,
}

impl Position {
    /// Builds a position from byte offsets, 1-based lines and a 1-based column.
    ///
    /// # Errors
    /// Returns [`PositionError::Offsets`] when `end < start`, and
    /// [`PositionError::Lines`] when a line or the column is 0 or
    /// `end_line < line`.
    pub fn new(
        start: usize,
        end: usize,
        line: u32,
        end_line: u32,
        column: u32,
    ) -> Result<Self, PositionError> {
        if end < start {
            return Err(PositionError::Offsets { start, end });
        }
        if line == 0 || column == 0 || end_line < line {
            return Err(PositionError::Lines {
                line,
                end_line,
                column,
            });
        }
        Ok(Self {
            start,
            end,
            line,
            end_line,
            column,
        })
    }

    /// The start byte offset.
    #[must_use]
    pub fn start(self) -> usize {
        self.start
    }

    /// The end byte offset, exclusive.
    #[must_use]
    pub fn end(self) -> usize {
        self.end
    }

    /// The first line, 1-based.
    #[must_use]
    pub fn line(self) -> u32 {
        self.line
    }

    /// The last line, 1-based.
    #[must_use]
    pub fn end_line(self) -> u32 {
        self.end_line
    }

    /// The column of the start, 1-based.
    #[must_use]
    pub fn column(self) -> u32 {
        self.column
    }

    /// The position moved down by `lines` lines, as when a caller strips
    /// frontmatter of that many lines; offsets and column stay.
    #[must_use]
    pub fn shifted(self, lines: u32) -> Self {
        Self {
            line: self.line.saturating_add(lines),
            end_line: self.end_line.saturating_add(lines),
            ..self
        }
    }
}

/// Where a node comes from: its file and its position in that file's source.
///
/// Every position tela reports reads the origin; a node's own
/// [`Node::position`](super::Node::position) stays relative to the parsed text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Origin {
    file: Arc<str>,
    position: Option<Position>,
}

impl Origin {
    /// Builds an origin for `file`, with no position for nodes that span the
    /// whole file.
    ///
    /// # Errors
    /// Returns [`PositionError::EmptyFile`] when `file` is empty.
    pub fn new(
        file: impl Into<Arc<str>>,
        position: Option<Position>,
    ) -> Result<Self, PositionError> {
        let file = file.into();
        if file.is_empty() {
            return Err(PositionError::EmptyFile);
        }
        Ok(Self { file, position })
    }

    /// The file, as the caller named it.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The shared file name, cheap to clone onto sibling nodes.
    #[must_use]
    pub fn file_name(&self) -> &Arc<str> {
        &self.file
    }

    /// The position in the source, absent for a whole-file node.
    #[must_use]
    pub fn position(&self) -> Option<Position> {
        self.position
    }

    /// The first source line, absent for a whole-file node.
    #[must_use]
    pub fn line(&self) -> Option<u32> {
        self.position.map(Position::line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_rejects_reversed_offsets() {
        assert_eq!(
            Position::new(5, 4, 1, 1, 1),
            Err(PositionError::Offsets { start: 5, end: 4 })
        );
    }

    #[test]
    fn position_rejects_zero_and_reversed_lines() {
        assert!(Position::new(0, 1, 0, 1, 1).is_err());
        assert!(Position::new(0, 1, 1, 1, 0).is_err());
        assert!(Position::new(0, 1, 3, 2, 1).is_err());
    }

    #[test]
    fn shifted_moves_lines_only() {
        let at = Position::new(40, 52, 10, 11, 4).unwrap().shifted(4);
        assert_eq!(
            (at.start(), at.end(), at.line(), at.end_line(), at.column()),
            (40, 52, 14, 15, 4)
        );
    }

    #[test]
    fn origin_requires_a_file() {
        assert_eq!(Origin::new("", None), Err(PositionError::EmptyFile));
        let origin =
            Origin::new("docs/a.md", Some(Position::new(0, 3, 14, 14, 1).unwrap())).unwrap();
        assert_eq!((origin.file(), origin.line()), ("docs/a.md", Some(14)));
    }
}
