//! Edits: the changes a query's `(replace …)`, `(delete …)`,
//! `(insert-before …)` and `(insert-after …)` forms make to the files
//! holding the nodes they capture, applied to a file's text or previewed as
//! a unified diff.

use std::fmt::Write as _;
use std::ops::Range;

/// What an edit does at a captured node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Action {
    /// Replaces the node's text.
    Replace,
    /// Removes the node; a node on lines of its own takes its line break
    /// and one blank line that would double another.
    Delete,
    /// Inserts text where the node starts.
    InsertBefore,
    /// Inserts text where the node ends.
    InsertAfter,
}

impl Action {
    pub(crate) fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "replace" => Some(Self::Replace),
            "delete" => Some(Self::Delete),
            "insert-before" => Some(Self::InsertBefore),
            "insert-after" => Some(Self::InsertAfter),
            _ => None,
        }
    }
}

/// One change to a file: `text` in place of the byte `span`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    file: String,
    span: Range<usize>,
    text: String,
    action: Action,
}

impl Change {
    /// The change `action` makes at `span` of `file`; an insert's span is
    /// the empty span where its text goes.
    #[must_use]
    pub fn new(
        file: impl Into<String>,
        span: Range<usize>,
        text: impl Into<String>,
        action: Action,
    ) -> Self {
        Self {
            file: file.into(),
            span,
            text: text.into(),
            action,
        }
    }

    /// The file, as its origin names it.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The byte span replaced.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }

    /// The text put in its place.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// What the change does.
    #[must_use]
    pub fn action(&self) -> Action {
        self.action
    }
}

/// Why changes could not be applied to a text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EditError {
    /// Two changes touch the same bytes.
    #[error("{file}: edits at bytes {first:?} and {second:?} overlap")]
    Overlap {
        /// The file.
        file: String,
        /// The earlier change's span.
        first: Range<usize>,
        /// The later change's span.
        second: Range<usize>,
    },
    /// A change's span lies outside the text or splits a character.
    #[error("{file}: an edit at bytes {span:?} lies outside the text")]
    OutOfRange {
        /// The file.
        file: String,
        /// The span.
        span: Range<usize>,
    },
}

/// `text` with `changes` applied; repeated changes apply once, and inserts
/// at one place keep their order.
///
/// # Errors
/// Returns [`EditError::OutOfRange`] for a span outside `text`, and
/// [`EditError::Overlap`] when two changes touch the same bytes.
pub fn apply(text: &str, changes: &[Change]) -> Result<String, EditError> {
    let changes = resolve(text, changes)?;
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for change in &changes {
        out.push_str(&text[last..change.span.start]);
        out.push_str(&change.text);
        last = change.span.end;
    }
    out.push_str(&text[last..]);
    Ok(out)
}

/// The changes to `text` as a unified diff without context lines, under
/// `---` and `+++` headers naming `file`.
///
/// # Errors
/// Returns the [`EditError`] [`apply`] would.
pub fn diff(file: &str, text: &str, changes: &[Change]) -> Result<String, EditError> {
    let changes = resolve(text, changes)?;
    let mut hunks: Vec<(Range<usize>, Vec<&Change>)> = Vec::new();
    for change in &changes {
        let lines = line_span(text, &change.span);
        match hunks.last_mut() {
            Some((span, members)) if lines.start <= span.end => {
                span.end = span.end.max(lines.end);
                members.push(change);
            }
            _ => hunks.push((lines, vec![change])),
        }
    }
    let mut out = format!("--- {file}\n+++ {file}\n");
    let mut shift: isize = 0;
    for (span, members) in hunks {
        let mut new = String::new();
        let mut last = span.start;
        for change in members {
            new.push_str(&text[last..change.span.start]);
            new.push_str(&change.text);
            last = change.span.end;
        }
        new.push_str(&text[last..span.end]);
        let old = &text[span.start..span.end];
        let first = text[..span.start].matches('\n').count() + 1;
        let (old_count, new_count) = (count_lines(old), count_lines(&new));
        let old_first = if old_count == 0 { first - 1 } else { first };
        let new_first = old_first.saturating_add_signed(shift);
        let new_first = if new_count == 0 && old_count > 0 {
            new_first - 1
        } else if new_count > 0 && old_count == 0 {
            new_first + 1
        } else {
            new_first
        };
        let _ = writeln!(
            out,
            "@@ -{old_first},{old_count} +{new_first},{new_count} @@"
        );
        for line in old.lines() {
            let _ = writeln!(out, "-{line}");
        }
        for line in new.lines() {
            let _ = writeln!(out, "+{line}");
        }
        shift += isize::try_from(new_count).unwrap_or(isize::MAX)
            - isize::try_from(old_count).unwrap_or(isize::MAX);
    }
    Ok(out)
}

/// The changes deduplicated, ordered, checked against `text`, with each
/// deletion widened to its lines.
fn resolve(text: &str, changes: &[Change]) -> Result<Vec<Change>, EditError> {
    let mut resolved: Vec<Change> = Vec::with_capacity(changes.len());
    for change in changes {
        let span = &change.span;
        if span.end > text.len()
            || !text.is_char_boundary(span.start)
            || !text.is_char_boundary(span.end)
        {
            return Err(EditError::OutOfRange {
                file: change.file.clone(),
                span: span.clone(),
            });
        }
        let mut change = change.clone();
        if change.action == Action::Delete {
            change.span = widen(text, change.span);
        }
        if !resolved.contains(&change) {
            resolved.push(change);
        }
    }
    resolved.sort_by_key(|change| (change.span.start, change.span.end));
    for pair in resolved.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if b.span.start < a.span.end {
            return Err(EditError::Overlap {
                file: b.file.clone(),
                first: a.span.clone(),
                second: b.span.clone(),
            });
        }
    }
    Ok(resolved)
}

/// A deleted `span` on lines of its own, with its line break and a blank
/// line that would double the one before it or end the text.
fn widen(text: &str, span: Range<usize>) -> Range<usize> {
    let bytes = text.as_bytes();
    let starts_line = span.start == 0 || bytes[span.start - 1] == b'\n';
    let mut end = span.end;
    if end > span.start && bytes[end - 1] == b'\n' {
        end -= 1;
    }
    let ends_line = end == bytes.len() || bytes[end] == b'\n';
    if !starts_line || !ends_line {
        return span;
    }
    end = (end + 1).min(bytes.len());
    let mut start = span.start;
    let blank_before = start == 0 || text[..start].ends_with("\n\n");
    if blank_before && bytes.get(end) == Some(&b'\n') {
        end += 1;
    } else if blank_before && end == bytes.len() && start > 0 {
        start -= 1;
    }
    start..end
}

/// The whole lines `span` touches, through the line break of the last.
fn line_span(text: &str, span: &Range<usize>) -> Range<usize> {
    let start = text[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let from = if span.end > span.start && text.as_bytes()[span.end - 1] == b'\n' {
        span.end - 1
    } else {
        span.end
    };
    let end = text[from..].find('\n').map_or(text.len(), |i| from + i + 1);
    start..end
}

fn count_lines(text: &str) -> usize {
    text.lines().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(span: Range<usize>, text: &str, action: Action) -> Change {
        Change::new("a.md", span, text, action)
    }

    #[test]
    fn replacements_and_inserts_apply_in_order() {
        let text = "# A\n\nold\n";
        let changes = [
            change(5..8, "new", Action::Replace),
            change(5..5, "> ", Action::InsertBefore),
            change(8..8, "!", Action::InsertAfter),
        ];
        assert_eq!(apply(text, &changes).unwrap(), "# A\n\n> new!\n");
    }

    #[test]
    fn a_deleted_block_takes_its_line_and_a_doubled_blank_line() {
        let text = "# A\n\none\n\ntwo\n";
        let changes = [change(5..8, "", Action::Delete)];
        assert_eq!(apply(text, &changes).unwrap(), "# A\n\ntwo\n");
        let last = [change(10..13, "", Action::Delete)];
        assert_eq!(apply(text, &last).unwrap(), "# A\n\none\n");
        let inline = [change(6..7, "", Action::Delete)];
        assert_eq!(apply(text, &inline).unwrap(), "# A\n\noe\n\ntwo\n");
    }

    #[test]
    fn overlapping_changes_are_rejected_and_repeats_apply_once() {
        let text = "abcdef";
        let overlap = [
            change(0..3, "x", Action::Replace),
            change(2..4, "y", Action::Replace),
        ];
        assert!(matches!(
            apply(text, &overlap),
            Err(EditError::Overlap { .. })
        ));
        let repeated = [
            change(0..1, "A", Action::Replace),
            change(0..1, "A", Action::Replace),
        ];
        assert_eq!(apply(text, &repeated).unwrap(), "Abcdef");
    }

    #[test]
    fn a_diff_shows_each_changed_line_once() {
        let text = "# A\n\none\n\ntwo\n";
        let changes = [
            change(5..8, "ONE", Action::Replace),
            change(10..13, "", Action::Delete),
        ];
        assert_eq!(
            diff("a.md", text, &changes).unwrap(),
            "--- a.md\n+++ a.md\n@@ -3,3 +3,1 @@\n-one\n-\n-two\n+ONE\n"
        );
    }
}
