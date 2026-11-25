//! Structural query and lint over text.
#![forbid(unsafe_code)]

pub mod frontend;
pub mod lint;
pub mod markdown;
pub mod plain;
pub mod query;
pub mod similarity;
pub mod tree;

pub use frontend::{Frontend, Frontends};
pub use lint::{Diagnostic, Plan, RuleId, Severity};
pub use query::{Query, Schema};
pub use tree::{Node, NodeId, NodeRef, Origin, Position, Tree};
