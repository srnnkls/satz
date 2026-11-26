//! The patterns a query reads into.

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Quant {
    Optional,
    Star,
    Plus,
}

impl Quant {
    pub(super) fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "?" => Some(Self::Optional),
            "*" => Some(Self::Star),
            "+" => Some(Self::Plus),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Pattern {
    /// Unique within its query; keys the matcher's memo.
    pub(super) id: usize,
    /// The node type; `None` for `_` and alternations.
    pub(super) kind: Option<String>,
    pub(super) alts: Vec<Pattern>,
    pub(super) attrs: Vec<Attr>,
    pub(super) chains: Vec<Chain>,
    pub(super) nots: Vec<Negation>,
    pub(super) relations: Vec<Relation>,
    pub(super) quant: Option<Quant>,
    pub(super) capture: Option<String>,
    pub(super) pos: usize,
}

impl Pattern {
    pub(super) fn new(id: usize, pos: usize) -> Self {
        Self {
            id,
            kind: None,
            alts: Vec::new(),
            attrs: Vec::new(),
            chains: Vec::new(),
            nots: Vec::new(),
            relations: Vec::new(),
            quant: None,
            capture: None,
            pos,
        }
    }

    /// Whether the pattern may match nothing: `?` or `*`.
    pub(super) fn optional(&self) -> bool {
        matches!(self.quant, Some(Quant::Optional | Quant::Star))
    }

    /// Whether the pattern repeats: `*` or `+`.
    pub(super) fn repeats(&self) -> bool {
        matches!(self.quant, Some(Quant::Star | Quant::Plus))
    }

    /// The node types the pattern can match, or `None` when it matches any.
    pub(super) fn kinds(&self) -> Option<Vec<&str>> {
        if self.alts.is_empty() {
            return self.kind.as_deref().map(|kind| vec![kind]);
        }
        let mut kinds = Vec::new();
        for alt in &self.alts {
            for kind in alt.kinds()? {
                if !kinds.contains(&kind) {
                    kinds.push(kind);
                }
            }
        }
        Some(kinds)
    }
}

/// Siblings matched in sequence.
#[derive(Debug, Clone)]
pub(super) struct Chain {
    pub(super) links: Vec<Link>,
    pub(super) first_child: bool,
    pub(super) last_child: bool,
}

impl Chain {
    /// The link the matcher anchors on: the first that must match.
    pub(super) fn pivot(&self) -> Option<usize> {
        self.links
            .iter()
            .position(|link| matches!(link.pattern.quant, None | Some(Quant::Plus)))
    }

    pub(super) fn optional(&self) -> bool {
        self.pivot().is_none()
    }
}

#[derive(Debug, Clone)]
pub(super) struct Link {
    pub(super) pattern: Pattern,
    pub(super) direct: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum RelationKind {
    Reaches,
    Inbound,
    To,
    From,
}

impl RelationKind {
    pub(super) fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "reaches" => Some(Self::Reaches),
            "inbound" => Some(Self::Inbound),
            "to" => Some(Self::To),
            "from" => Some(Self::From),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Relation {
    pub(super) kind: RelationKind,
    pub(super) target: Box<Pattern>,
}

#[derive(Debug, Clone)]
pub(super) enum Negation {
    Pattern(Pattern),
    Relation(Relation),
}

impl Negation {
    pub(super) fn target(&self) -> &Pattern {
        match self {
            Self::Pattern(pattern) => pattern,
            Self::Relation(relation) => &relation.target,
        }
    }
}

/// One `:key` of a pattern.
#[derive(Debug, Clone)]
pub(super) struct Attr {
    pub(super) key: String,
    /// The offset of the variable or regexp the attribute binds from.
    pub(super) pos: usize,
    pub(super) check: Check,
    /// The value capture after the attribute, and its offset.
    pub(super) capture: Option<(String, usize)>,
}

impl Attr {
    /// The variables the attribute binds.
    pub(super) fn bindings(&self) -> impl Iterator<Item = &str> {
        let var = match &self.check {
            Check::Bind(var) => Some(var.as_str()),
            _ => None,
        };
        let groups = match &self.check {
            Check::Groups { groups, .. } => groups.as_slice(),
            _ => &[],
        };
        var.into_iter()
            .chain(groups.iter().filter_map(|g| g.var.as_deref()))
    }

    /// The variable the attribute binds or constrains.
    pub(super) fn var(&self) -> Option<&str> {
        match &self.check {
            Check::Bind(var) | Check::Constrain { var, .. } => Some(var),
            _ => None,
        }
    }

    /// The score capture of a similarity constraint.
    pub(super) fn score(&self) -> Option<&str> {
        self.score_at().map(|(name, _)| name)
    }

    /// The score capture of a similarity constraint, and its offset.
    pub(super) fn score_at(&self) -> Option<(&str, usize)> {
        match &self.check {
            Check::Constrain {
                relate: Relate::Similarity(similarity),
                ..
            } => similarity
                .score
                .as_ref()
                .map(|(name, pos)| (name.as_str(), *pos)),
            _ => None,
        }
    }

    /// The value capture, or else the score capture, and its offset.
    pub(super) fn output(&self) -> Option<(&str, usize)> {
        self.capture
            .as_ref()
            .map(|(name, pos)| (name.as_str(), *pos))
            .or_else(|| self.score_at())
    }
}

#[derive(Debug, Clone)]
pub(super) enum Check {
    /// The node's value passes `test`, or fails it when negated.
    Test { test: Test, negate: bool },
    /// The node's value binds the variable.
    Bind(String),
    /// The node's value relates to the variable's bound value.
    Constrain { var: String, relate: Relate },
    /// The regexp's named groups bind or equal values.
    Groups { regex: Regex, groups: Vec<Submatch> },
}

#[derive(Debug, Clone)]
pub(super) enum Test {
    Equals(String),
    Param(String),
    Range(f64, f64),
    Regex(Regex),
    Compare(Comparison, Operand),
    Older(Operand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Comparison {
    Above,
    AtLeast,
    Below,
    AtMost,
}

impl Comparison {
    pub(super) fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            ">" => Some(Self::Above),
            ">=" => Some(Self::AtLeast),
            "<" => Some(Self::Below),
            "<=" => Some(Self::AtMost),
            _ => None,
        }
    }

    pub(super) fn holds(self, value: f64, limit: f64) -> bool {
        match self {
            Self::Above => value > limit,
            Self::AtLeast => value >= limit,
            Self::Below => value < limit,
            Self::AtMost => value <= limit,
        }
    }
}

/// A number written in the query or a `$param`.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Operand {
    Number(f64),
    Param(String),
}

/// How a constrained value relates to the variable's bound value.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Relate {
    Not,
    After,
    Before,
    Contains,
    Covers,
    Within,
    Similarity(Similarity),
}

impl Relate {
    pub(super) fn score(&self) -> Option<&str> {
        match self {
            Self::Similarity(similarity) => {
                similarity.score.as_ref().map(|(name, _)| name.as_str())
            }
            _ => None,
        }
    }

    /// Whether the relation is a shingle measure an index can narrow.
    pub(super) fn shingled(&self) -> bool {
        matches!(
            self,
            Self::Similarity(Similarity {
                measure: Measure::Near | Measure::Overlap,
                ..
            })
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Similarity {
    pub(super) measure: Measure,
    pub(super) threshold: Operand,
    /// The score capture, and its offset.
    pub(super) score: Option<(String, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Measure {
    Near,
    Overlap,
    Similar,
}

/// A named regexp group: it binds `var`, or must equal `value`.
#[derive(Debug, Clone)]
pub(super) struct Submatch {
    pub(super) name: String,
    pub(super) var: Option<String>,
    pub(super) value: String,
}
