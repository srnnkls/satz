//! A run's view of the tree and the environment's data tables: every node in
//! document order, addressed by its position in that order.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use super::predicate::{Facts, fact_kind};
use super::{CORPUS, Hit, Table};
use crate::tree::{Node, NodeId, NodeRef, Origin, Tree};

/// The node type holding the data table rows.
const DATA: &str = "data";

pub(super) struct Entry<'t> {
    pub(super) node: &'t Node,
    /// The node's id in the queried tree; `None` for data table nodes.
    pub(super) id: Option<NodeId>,
    /// Whether patterns can match it: not for corpus and data holders.
    pub(super) matchable: bool,
    pub(super) parent: Option<usize>,
    /// The node's index among its parent's children.
    pub(super) index: usize,
    pub(super) children: Vec<usize>,
    /// The end of the node's subtree in document order, exclusive.
    pub(super) end: usize,
}

pub(super) struct View<'t> {
    pub(super) entries: Vec<Entry<'t>>,
    pub(super) by_id: HashMap<NodeId, usize>,
}

/// The data tables and relation facts as a corpus holding a `data` node of
/// `row` nodes, tables in name order and rows in table order, then of fact
/// nodes, relations in name order and facts in value order.
pub(super) fn data_tree(tables: &BTreeMap<String, Table>, facts: &Facts) -> Option<Tree> {
    if tables.is_empty() && facts.values().all(BTreeSet::is_empty) {
        return None;
    }
    let origin = Origin::named(&Arc::from(""), None);
    let mut tree = Tree::new(Node::new(CORPUS, origin.clone()));
    let holder = tree
        .append(tree.root().id(), Node::new(DATA, origin.clone()))
        .ok()?;
    for (name, table) in tables {
        let rows: Vec<(&str, String, &str)> = match table {
            Table::Map(rows) => rows
                .iter()
                .map(|(key, value)| ("key", key.clone(), value.as_str()))
                .collect(),
            Table::List(rows) => rows
                .iter()
                .enumerate()
                .map(|(i, value)| ("index", i.to_string(), value.as_str()))
                .collect(),
        };
        for (field, at, value) in rows {
            let mut row = Node::new("row", origin.clone());
            row.attrs.insert("table".to_owned(), name.clone());
            row.attrs.insert("value".to_owned(), value.to_owned());
            row.attrs.insert(field.to_owned(), at);
            tree.append(holder, row).ok()?;
        }
    }
    for (name, tuples) in facts {
        for tuple in tuples {
            let mut fact = Node::new(fact_kind(name), origin.clone());
            for (i, value) in tuple.iter().enumerate() {
                fact.attrs.insert(i.to_string(), value.clone());
            }
            tree.append(holder, fact).ok()?;
        }
    }
    Some(tree)
}

impl<'t> View<'t> {
    /// The view of `tree` with the data tree's rows after its nodes. A root
    /// that is not a corpus is wrapped in the data tree's corpus, so data
    /// rows never sit inside a file.
    pub(super) fn new(tree: &'t Tree, data: Option<&'t Tree>) -> Self {
        let mut view = Self {
            entries: Vec::new(),
            by_id: HashMap::new(),
        };
        let root = tree.root();
        match data.map(|data| (data.root(), data.root().child(0))) {
            Some((corpus, Some(holder))) => {
                let parent = (root.node().kind != CORPUS)
                    .then(|| view.add(corpus.node(), None, None, false));
                view.push(root, parent, true);
                view.push(holder, Some(0), false);
            }
            _ => view.push(root, None, true),
        }
        for at in (0..view.entries.len()).rev() {
            view.entries[at].end = view.entries[at]
                .children
                .last()
                .map_or(at + 1, |&child| view.entries[child].end);
        }
        view
    }

    fn add(
        &mut self,
        node: &'t Node,
        id: Option<NodeId>,
        parent: Option<usize>,
        matchable: bool,
    ) -> usize {
        let at = self.entries.len();
        let index = parent.map_or(0, |p| {
            let siblings = &mut self.entries[p].children;
            siblings.push(at);
            siblings.len() - 1
        });
        if let Some(id) = id {
            self.by_id.insert(id, at);
        }
        self.entries.push(Entry {
            node,
            id,
            matchable,
            parent,
            index,
            children: Vec::new(),
            end: at + 1,
        });
        at
    }

    /// Adds the subtree of `root` in document order.
    fn push(&mut self, root: NodeRef<'t>, parent: Option<usize>, queried: bool) {
        let mut stack = vec![(root, parent)];
        while let Some((node, parent)) = stack.pop() {
            let data = node.node();
            let matchable = if queried {
                data.kind != CORPUS
            } else {
                data.kind != DATA && data.kind != CORPUS
            };
            let id = queried.then(|| node.id());
            let at = self.add(data, id, parent, matchable);
            stack.extend(node.children().rev().map(|child| (child, Some(at))));
        }
    }

    pub(super) fn kind(&self, at: usize) -> &'t str {
        &self.entries[at].node.kind
    }

    pub(super) fn node(&self, at: usize) -> &'t Node {
        self.entries[at].node
    }

    /// The node and its ancestors, nearest first.
    pub(super) fn ancestors(&self, at: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(at), |&at| self.entries[at].parent)
    }

    /// What a capture of the node reports.
    pub(super) fn hit(&self, at: usize) -> Hit {
        let entry = &self.entries[at];
        match entry.id {
            Some(id) => Hit::Node(id),
            None => Hit::Row(entry.node.attrs.clone()),
        }
    }
}
