//! Helpers shared by the document editor's tests.

use super::{Document, Item};
use crate::tree::Origin;

/// A constructed item of `kind` with `text`.
pub(super) fn item(kind: &str, text: &str) -> Item {
    let mut item = Item::new(kind, Origin::new("new.md", None).expect("a file name"));
    text.clone_into(&mut item.text);
    item
}

/// Parses `text` as `file`, failing on any problem.
pub(super) fn parse(file: &str, text: &str) -> Document {
    let parsed = Document::parse(file, text);
    assert_eq!(parsed.problems, [], "{text:?}");
    parsed.value
}

/// The index path of the `nth` item of `kind`, in document order.
pub(super) fn path(items: &[Item], kind: &str, nth: usize) -> Option<Vec<usize>> {
    fn visit(items: &[Item], kind: &str, nth: &mut usize, path: &mut Vec<usize>) -> bool {
        for (i, item) in items.iter().enumerate() {
            path.push(i);
            if item.kind == kind {
                if *nth == 0 {
                    return true;
                }
                *nth -= 1;
            }
            if visit(&item.children, kind, nth, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut nth = nth;
    let mut path = Vec::new();
    visit(items, kind, &mut nth, &mut path).then_some(path)
}

/// The item at `path`.
pub(super) fn at<'a>(items: &'a [Item], path: &[usize]) -> &'a Item {
    let (first, rest) = path.split_first().expect("a non-empty path");
    rest.iter()
        .fold(&items[*first], |item, &i| &item.children[i])
}

/// The item at `path`, mutably.
pub(super) fn at_mut<'a>(items: &'a mut [Item], path: &[usize]) -> &'a mut Item {
    let (first, rest) = path.split_first().expect("a non-empty path");
    rest.iter()
        .fold(&mut items[*first], |item, &i| &mut item.children[i])
}

/// The `nth` item of `kind`.
pub(super) fn find<'a>(doc: &'a Document, kind: &str, nth: usize) -> &'a Item {
    let path = path(&doc.children, kind, nth).expect("the item");
    at(&doc.children, &path)
}

/// The `nth` item of `kind`, mutably.
pub(super) fn find_mut<'a>(doc: &'a mut Document, kind: &str, nth: usize) -> &'a mut Item {
    let path = path(&doc.children, kind, nth).expect("the item");
    at_mut(&mut doc.children, &path)
}

/// The children at `path`, the document's own for an empty path.
pub(super) fn children_mut<'a>(doc: &'a mut Document, path: &[usize]) -> &'a mut Vec<Item> {
    path.iter()
        .fold(&mut doc.children, |items, &i| &mut items[i].children)
}
