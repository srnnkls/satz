//! The node tree every frontend builds and every query reads.
//!
//! A [`Tree`] is a flat arena: nodes live in one vector and refer to each
//! other by [`NodeId`]. `&mut Tree` builds and grafts; `&Tree` is read through
//! [`NodeRef`] handles, so one tree can be queried from several threads.

mod position;

use std::collections::BTreeMap;
use std::fmt;
use std::iter::FusedIterator;
use std::num::NonZeroU32;

pub use position::{EmptyFile, Origin, Position, PositionError};

/// The node kind of malformed input a frontend met.
pub const PROBLEM: &str = "problem";

/// The data of one node; its place in the tree is the arena's.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Node {
    /// The node type, such as `heading`, `paragraph` or a caller's `skill`.
    pub kind: String,
    /// Keyed attributes, such as `level` or `url`.
    pub attrs: BTreeMap<String, String>,
    /// The node's text, which text keys, `:contains` and `:matches` read.
    pub text: String,
    /// A whole-file node's text, frontmatter excluded, which only the size
    /// keys `:lines`, `:words` and `:chars` measure; empty on other nodes.
    pub body: String,
    /// The position in the parsed text, absent for a whole-file node.
    pub position: Option<Position>,
    /// The file and source position every reported position reads.
    pub origin: Origin,
}

impl Node {
    /// A node of `kind` from `origin`, with no attributes, text, body or
    /// position.
    #[must_use]
    pub fn new(kind: impl Into<String>, origin: Origin) -> Self {
        Self {
            kind: kind.into(),
            attrs: BTreeMap::new(),
            text: String::new(),
            body: String::new(),
            position: None,
            origin,
        }
    }

    /// A `problem` node reporting `message` at `origin`.
    #[must_use]
    pub fn problem(message: impl Into<String>, origin: Origin) -> Self {
        let message = message.into();
        let mut node = Self::new(PROBLEM, origin);
        node.attrs.insert("message".to_owned(), message.clone());
        node.text = message;
        node
    }
}

/// Identifies a node within the tree that issued it.
///
/// Ids are tree-local: an id from another tree is not detected unless it is
/// out of this tree's range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(NonZeroU32);

impl NodeId {
    fn from_slot(slot: usize) -> Option<Self> {
        let raw = u32::try_from(slot.checked_add(1)?).ok()?;
        NonZeroU32::new(raw).map(Self)
    }

    fn slot(self) -> usize {
        usize::try_from(self.0.get() - 1).unwrap_or(usize::MAX)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Why a tree could not be changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TreeError {
    /// The id is out of this tree's range.
    #[error("node {0} is out of this tree's range")]
    UnknownNode(NodeId),
    /// The tree already holds `u32::MAX` nodes.
    #[error("tree is full")]
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Slot {
    node: Node,
    parent: Option<NodeId>,
    index: usize,
    children: Vec<NodeId>,
}

/// An arena of nodes under one root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    slots: Vec<Slot>,
}

impl Tree {
    /// A tree holding only `root`.
    #[must_use]
    pub fn new(root: Node) -> Self {
        Self {
            slots: vec![Slot {
                node: root,
                parent: None,
                index: 0,
                children: Vec::new(),
            }],
        }
    }

    /// The root node.
    #[must_use]
    pub fn root(&self) -> NodeRef<'_> {
        NodeRef {
            tree: self,
            id: NodeId(NonZeroU32::MIN),
        }
    }

    /// The node `id` names, if it is in this tree's range.
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<NodeRef<'_>> {
        (id.slot() < self.slots.len()).then_some(NodeRef { tree: self, id })
    }

    /// Mutable access to the data of node `id`.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.slots.get_mut(id.slot()).map(|slot| &mut slot.node)
    }

    /// The number of nodes, the root included.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.slots.len()
    }

    /// The `problem` nodes, in document order.
    #[must_use]
    pub fn problems(&self) -> Problems<'_> {
        Problems(self.root().descendants())
    }

    /// Appends `node` as the last child of `parent`.
    ///
    /// # Errors
    /// Returns [`TreeError::UnknownNode`] when `parent` is out of range and
    /// [`TreeError::Full`] when the tree holds `u32::MAX` nodes.
    pub fn append(&mut self, parent: NodeId, node: Node) -> Result<NodeId, TreeError> {
        let index = self.slot(parent)?.children.len();
        let id = NodeId::from_slot(self.slots.len()).ok_or(TreeError::Full)?;
        self.slots.push(Slot {
            node,
            parent: Some(parent),
            index,
            children: Vec::new(),
        });
        self.slot_mut(parent)?.children.push(id);
        Ok(id)
    }

    /// Moves `subtree` under `parent` as its last child and returns the id of
    /// the subtree's former root. The subtree's ids are remapped; ids it issued
    /// before do not name its nodes here.
    ///
    /// # Errors
    /// Returns [`TreeError::UnknownNode`] when `parent` is out of range and
    /// [`TreeError::Full`] when the result would exceed `u32::MAX` nodes.
    pub fn graft(&mut self, parent: NodeId, subtree: Tree) -> Result<NodeId, TreeError> {
        let index = self.slot(parent)?.children.len();
        let offset = self.slots.len();
        let root = NodeId::from_slot(offset).ok_or(TreeError::Full)?;
        NodeId::from_slot(offset + subtree.slots.len() - 1).ok_or(TreeError::Full)?;
        let remap = |id: NodeId| NodeId::from_slot(id.slot() + offset).unwrap_or(id);
        self.slots
            .extend(subtree.slots.into_iter().map(|slot| Slot {
                parent: Some(slot.parent.map_or(parent, remap)),
                index: if slot.parent.is_none() {
                    index
                } else {
                    slot.index
                },
                children: slot.children.into_iter().map(remap).collect(),
                node: slot.node,
            }));
        self.slot_mut(parent)?.children.push(root);
        Ok(root)
    }

    fn slot(&self, id: NodeId) -> Result<&Slot, TreeError> {
        self.slots.get(id.slot()).ok_or(TreeError::UnknownNode(id))
    }

    fn slot_mut(&mut self, id: NodeId) -> Result<&mut Slot, TreeError> {
        self.slots
            .get_mut(id.slot())
            .ok_or(TreeError::UnknownNode(id))
    }
}

/// A read handle on one node of a borrowed tree.
#[derive(Clone, Copy)]
pub struct NodeRef<'a> {
    tree: &'a Tree,
    id: NodeId,
}

impl<'a> NodeRef<'a> {
    fn slot(self) -> &'a Slot {
        &self.tree.slots[self.id.slot()]
    }

    fn at(self, id: NodeId) -> Self {
        Self {
            tree: self.tree,
            id,
        }
    }

    /// The node's id.
    #[must_use]
    pub fn id(self) -> NodeId {
        self.id
    }

    /// The node's data.
    #[must_use]
    pub fn node(self) -> &'a Node {
        &self.slot().node
    }

    /// The node's index among its parent's children; 0 for the root.
    #[must_use]
    pub fn index(self) -> usize {
        self.slot().index
    }

    /// The parent, absent for the root.
    #[must_use]
    pub fn parent(self) -> Option<Self> {
        self.slot().parent.map(|id| self.at(id))
    }

    /// The children in document order.
    #[must_use]
    pub fn children(self) -> Children<'a> {
        Children {
            tree: self.tree,
            ids: self.slot().children.iter(),
        }
    }

    /// The child at `index`.
    #[must_use]
    pub fn child(self, index: usize) -> Option<Self> {
        self.slot().children.get(index).map(|&id| self.at(id))
    }

    /// The sibling `offset` places after this node, or before it when negative.
    #[must_use]
    pub fn sibling(self, offset: isize) -> Option<Self> {
        let index = self.index().checked_add_signed(offset)?;
        self.parent()?.child(index)
    }

    /// The parent, its parent, and so on up to the root.
    #[must_use]
    pub fn ancestors(self) -> Ancestors<'a> {
        Ancestors(self.parent())
    }

    /// This node and every node below it, in document order.
    #[must_use]
    pub fn descendants(self) -> Descendants<'a> {
        Descendants(vec![self])
    }
}

impl fmt::Debug for NodeRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRef")
            .field("id", &self.id)
            .field("node", self.node())
            .finish_non_exhaustive()
    }
}

impl PartialEq for NodeRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.tree, other.tree) && self.id == other.id
    }
}

impl Eq for NodeRef<'_> {}

/// The children of a node; see [`NodeRef::children`].
#[derive(Clone)]
pub struct Children<'a> {
    tree: &'a Tree,
    ids: std::slice::Iter<'a, NodeId>,
}

impl fmt::Debug for Children<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<'a> Iterator for Children<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let tree = self.tree;
        self.ids.next().map(|&id| NodeRef { tree, id })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.ids.size_hint()
    }
}

impl DoubleEndedIterator for Children<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let tree = self.tree;
        self.ids.next_back().map(|&id| NodeRef { tree, id })
    }
}

impl ExactSizeIterator for Children<'_> {}

impl FusedIterator for Children<'_> {}

/// The ancestors of a node, nearest first; see [`NodeRef::ancestors`].
#[derive(Debug, Clone)]
pub struct Ancestors<'a>(Option<NodeRef<'a>>);

impl<'a> Iterator for Ancestors<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.0?;
        self.0 = node.parent();
        Some(node)
    }
}

impl FusedIterator for Ancestors<'_> {}

/// A node and its descendants in document order; see
/// [`NodeRef::descendants`].
#[derive(Debug, Clone)]
pub struct Descendants<'a>(Vec<NodeRef<'a>>);

impl<'a> Iterator for Descendants<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.0.pop()?;
        self.0.extend(node.children().rev());
        Some(node)
    }
}

impl FusedIterator for Descendants<'_> {}

/// The `problem` nodes of a tree; see [`Tree::problems`].
#[derive(Debug, Clone)]
pub struct Problems<'a>(Descendants<'a>);

impl<'a> Iterator for Problems<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.find(|node| node.node().kind == PROBLEM)
    }
}

impl FusedIterator for Problems<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> Origin {
        Origin::new("a.md", None).unwrap()
    }

    fn node(kind: &str) -> Node {
        Node::new(kind, origin())
    }

    fn kinds<'a>(nodes: impl Iterator<Item = NodeRef<'a>>) -> Vec<&'a str> {
        nodes.map(|n| n.node().kind.as_str()).collect()
    }

    fn sample() -> (Tree, NodeId, NodeId, NodeId) {
        let mut tree = Tree::new(node("file"));
        let root = tree.root().id();
        let section = tree.append(root, node("section")).unwrap();
        let heading = tree.append(section, node("heading")).unwrap();
        let paragraph = tree.append(section, node("paragraph")).unwrap();
        (tree, section, heading, paragraph)
    }

    #[test]
    fn append_links_parent_index_and_siblings() {
        let (tree, section, heading, paragraph) = sample();
        let p = tree.get(paragraph).unwrap();
        assert_eq!(p.parent().map(NodeRef::id), Some(section));
        assert_eq!(p.index(), 1);
        assert_eq!(p.sibling(-1).map(NodeRef::id), Some(heading));
        assert!(p.sibling(1).is_none());
        assert!(p.sibling(-2).is_none());
        assert_eq!(kinds(p.ancestors()), ["section", "file"]);
    }

    #[test]
    fn iterators_run_in_document_order() {
        let (tree, section, ..) = sample();
        assert_eq!(
            kinds(tree.root().descendants()),
            ["file", "section", "heading", "paragraph"]
        );
        let children = tree.get(section).unwrap().children();
        assert_eq!(children.len(), 2);
        assert_eq!(kinds(children.rev()), ["paragraph", "heading"]);
    }

    #[test]
    fn graft_remaps_the_subtree_under_its_new_parent() {
        let (mut tree, section, ..) = sample();
        let (sub, ..) = sample();
        let grafted = tree.graft(section, sub).unwrap();
        let g = tree.get(grafted).unwrap();
        assert_eq!(g.parent().map(NodeRef::id), Some(section));
        assert_eq!(g.index(), 2);
        assert_eq!(
            kinds(g.descendants()),
            ["file", "section", "heading", "paragraph"]
        );
        assert_eq!(
            kinds(g.child(0).unwrap().children()),
            ["heading", "paragraph"]
        );
        assert_eq!(tree.node_count(), 8);
    }

    #[test]
    fn out_of_range_ids_are_rejected() {
        let mut small = Tree::new(node("file"));
        let (_, _, _, paragraph) = sample();
        assert!(small.get(paragraph).is_none());
        assert_eq!(
            small.append(paragraph, node("x")),
            Err(TreeError::UnknownNode(paragraph))
        );
    }

    #[test]
    fn problems_are_nodes() {
        let (mut tree, section, ..) = sample();
        tree.append(section, Node::problem("unclosed directive", origin()))
            .unwrap();
        let problems: Vec<_> = tree.problems().map(|p| p.node().text.as_str()).collect();
        assert_eq!(problems, ["unclosed directive"]);
    }

    #[test]
    fn handles_compare_by_tree_and_id() {
        let (tree, section, ..) = sample();
        let (other, ..) = sample();
        assert_eq!(tree.get(section), tree.get(section));
        assert_ne!(tree.get(section), other.get(section));
    }

    #[test]
    fn node_line_and_origin_line_stay_apart() {
        let body = Position::new(120..131, 10..=10, 1).unwrap();
        let mut heading = Node::new(
            "heading",
            Origin::new("a.md", Some(body.shifted(4))).unwrap(),
        );
        heading.position = Some(body);
        assert_eq!(heading.position.as_ref().map(Position::line), Some(10));
        assert_eq!(heading.origin.line(), Some(14));
    }

    #[test]
    fn trees_are_shareable_across_threads() {
        fn shareable<T: Send + Sync>() {}
        shareable::<Tree>();
        shareable::<NodeRef<'static>>();
    }
}
