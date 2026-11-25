//! The node tree every frontend builds and every query reads.
//!
//! A [`Tree`] is a flat arena: nodes live in one vector and refer to each
//! other by [`NodeId`]. `&mut Tree` builds and grafts; `&Tree` is read through
//! [`NodeRef`] handles, so one tree can be queried from several threads.

mod position;

use std::collections::BTreeMap;
use std::num::NonZeroU32;

pub use position::{Origin, Position, PositionError};

/// The data of one node; its place in the tree is the arena's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The node type, such as `heading`, `paragraph` or a caller's `skill`.
    pub kind: String,
    /// Keyed attributes, such as `level` or `url`.
    pub attrs: BTreeMap<String, String>,
    /// The node's text.
    pub text: String,
    /// The position in the parsed text, absent for a whole-file node.
    pub position: Option<Position>,
    /// The file and source position every reported position reads.
    pub origin: Origin,
}

impl Node {
    /// A node of `kind` from `origin`, with no attributes, text or position.
    #[must_use]
    pub fn new(kind: impl Into<String>, origin: Origin) -> Self {
        Self {
            kind: kind.into(),
            attrs: BTreeMap::new(),
            text: String::new(),
            position: None,
            origin,
        }
    }
}

/// Identifies a node within the tree that issued it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(NonZeroU32);

impl NodeId {
    fn from_slot(slot: usize) -> Self {
        let raw = u32::try_from(slot + 1).unwrap_or(u32::MAX);
        Self(NonZeroU32::new(raw).unwrap_or(NonZeroU32::MAX))
    }

    fn slot(self) -> usize {
        usize::try_from(self.0.get() - 1).unwrap_or(usize::MAX)
    }
}

/// A [`NodeId`] the tree did not issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("node {0:?} is not in this tree")]
pub struct UnknownNode(pub NodeId);

#[derive(Debug, Clone)]
struct Slot {
    node: Node,
    parent: Option<NodeId>,
    index: usize,
    children: Vec<NodeId>,
}

/// An arena of nodes under one root.
#[derive(Debug, Clone)]
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
            id: NodeId::from_slot(0),
        }
    }

    /// The node `id` names, if this tree issued it.
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<NodeRef<'_>> {
        (id.slot() < self.slots.len()).then_some(NodeRef { tree: self, id })
    }

    /// Mutable access to the data of node `id`.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.slots.get_mut(id.slot()).map(|slot| &mut slot.node)
    }

    /// The number of nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Always false: a tree holds at least its root.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Appends `node` as the last child of `parent`.
    ///
    /// # Errors
    /// Returns [`UnknownNode`] when this tree did not issue `parent`.
    pub fn append(&mut self, parent: NodeId, node: Node) -> Result<NodeId, UnknownNode> {
        let index = self.slot(parent)?.children.len();
        let id = NodeId::from_slot(self.slots.len());
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
    /// before do not name nodes here.
    ///
    /// # Errors
    /// Returns [`UnknownNode`] when this tree did not issue `parent`.
    pub fn graft(&mut self, parent: NodeId, subtree: Tree) -> Result<NodeId, UnknownNode> {
        let index = self.slot(parent)?.children.len();
        let offset = self.slots.len();
        let remap = |id: NodeId| NodeId::from_slot(id.slot() + offset);
        let root = NodeId::from_slot(offset);
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

    fn slot(&self, id: NodeId) -> Result<&Slot, UnknownNode> {
        self.slots.get(id.slot()).ok_or(UnknownNode(id))
    }

    fn slot_mut(&mut self, id: NodeId) -> Result<&mut Slot, UnknownNode> {
        self.slots.get_mut(id.slot()).ok_or(UnknownNode(id))
    }
}

/// A read handle on one node of a borrowed tree.
#[derive(Debug, Clone, Copy)]
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
    pub fn children(self) -> impl DoubleEndedIterator<Item = NodeRef<'a>> + ExactSizeIterator {
        self.slot().children.iter().map(move |&id| self.at(id))
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
    pub fn ancestors(self) -> impl Iterator<Item = NodeRef<'a>> {
        std::iter::successors(self.parent(), |node| node.parent())
    }

    /// This node and every node below it, in document order.
    pub fn descendants(self) -> impl Iterator<Item = NodeRef<'a>> {
        let mut stack = vec![self];
        std::iter::from_fn(move || {
            let node = stack.pop()?;
            stack.extend(node.children().rev());
            Some(node)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(kind: &str) -> Node {
        Node::new(kind, Origin::new("a.md", None).unwrap())
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
    fn descendants_run_in_document_order() {
        let (tree, ..) = sample();
        assert_eq!(
            kinds(tree.root().descendants()),
            ["file", "section", "heading", "paragraph"]
        );
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
        assert_eq!(tree.len(), 8);
    }

    #[test]
    fn foreign_ids_are_rejected() {
        let mut small = Tree::new(node("file"));
        let (big, _, _, paragraph) = sample();
        assert!(small.get(paragraph).is_none());
        assert_eq!(
            small.append(paragraph, node("x")),
            Err(UnknownNode(paragraph))
        );
        assert!(big.get(paragraph).is_some());
    }

    #[test]
    fn node_line_and_origin_line_stay_apart() {
        let body = Position::new(120, 131, 10, 10, 1).unwrap();
        let mut heading = Node::new(
            "heading",
            Origin::new("a.md", Some(body.shifted(4))).unwrap(),
        );
        heading.position = Some(body);
        assert_eq!(heading.position.map(Position::line), Some(10));
        assert_eq!(heading.origin.line(), Some(14));
    }

    #[test]
    fn trees_are_shareable_across_threads() {
        fn shareable<T: Send + Sync>() {}
        shareable::<Tree>();
        shareable::<NodeRef<'static>>();
    }
}
