//! A module's frontmatter: its params, data tables and the caller's meta keys.

use std::collections::{BTreeMap, BTreeSet};

use crate::markdown::{YamlNode, YamlValue};
use crate::query::{Table, Value};

/// What a module declares in its frontmatter.
#[derive(Debug, Default)]
pub(super) struct Header {
    /// Param defaults of every rule in the module.
    pub(super) params: BTreeMap<String, Value>,
    /// Data tables of every rule in the module.
    pub(super) data: BTreeMap<String, Table>,
    meta: BTreeMap<String, Meta>,
}

/// A meta key's value: one for every rule, or one per rule id.
#[derive(Debug)]
enum Meta {
    All(String),
    ByRule(BTreeMap<String, String>),
}

/// Why a header was rejected.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Fault {
    /// A key neither satz nor the caller declares.
    UnknownKey(String),
    /// A value of the wrong shape, or YAML the header cannot take.
    Invalid(String),
}

impl Header {
    /// Reads the YAML document of a module's frontmatter, accepting
    /// `meta_keys` beyond satz's own `description`, `params` and `data`.
    pub(super) fn read(
        root: Option<&YamlNode>,
        meta_keys: &BTreeSet<String>,
    ) -> Result<Self, Fault> {
        let mut header = Self::default();
        let root = root.map(Yaml::of).transpose().map_err(Fault::Invalid)?;
        let pairs = match root {
            Some(Yaml::Map(pairs)) => pairs,
            None | Some(Yaml::Null) => return Ok(header),
            Some(_) => return Err(Fault::Invalid("frontmatter must be a mapping".to_owned())),
        };
        for (key, value) in pairs {
            match key {
                "description" => {}
                "params" => header.params = params(value)?,
                "data" => header.data = data(value)?,
                _ if meta_keys.contains(key) => {
                    let meta = meta(key, value)?;
                    header.meta.insert(key.to_owned(), meta);
                }
                _ => return Err(Fault::UnknownKey(key.to_owned())),
            }
        }
        Ok(header)
    }

    /// The raw meta values of rule `id`: a key's value when it is a scalar,
    /// or its entry for `id` when it is a table by rule id.
    pub(super) fn meta(&self, id: &str) -> BTreeMap<String, String> {
        self.meta
            .iter()
            .filter_map(|(key, meta)| {
                let value = match meta {
                    Meta::All(value) => value,
                    Meta::ByRule(values) => values.get(id)?,
                };
                Some((key.clone(), value.clone()))
            })
            .collect()
    }
}

fn params(value: Yaml<'_>) -> Result<BTreeMap<String, Value>, Fault> {
    entries(value, "params")?
        .into_iter()
        .map(|(key, value)| match value {
            Yaml::Scalar {
                number: Some(n), ..
            } => Ok((key.to_owned(), Value::Number(n))),
            Yaml::Scalar { text, .. } => Ok((key.to_owned(), Value::Text(text.to_owned()))),
            _ => Err(Fault::Invalid(format!(
                "param {key} must be a number, string or boolean"
            ))),
        })
        .collect()
}

fn data(value: Yaml<'_>) -> Result<BTreeMap<String, Table>, Fault> {
    entries(value, "data")?
        .into_iter()
        .map(|(key, value)| {
            let rows = |value: Yaml<'_>| {
                value.text().map(str::to_owned).ok_or_else(|| {
                    Fault::Invalid(format!(
                        "data {key}: rows must be strings, numbers or booleans"
                    ))
                })
            };
            let table = match value {
                Yaml::Map(pairs) => Table::Map(
                    pairs
                        .into_iter()
                        .map(|(row, value)| Ok((row.to_owned(), rows(value)?)))
                        .collect::<Result<_, Fault>>()?,
                ),
                Yaml::List(items) => {
                    Table::List(items.into_iter().map(rows).collect::<Result<_, Fault>>()?)
                }
                _ => {
                    return Err(Fault::Invalid(format!(
                        "data {key} must be a table or a list"
                    )));
                }
            };
            Ok((key.to_owned(), table))
        })
        .collect()
}

fn meta(key: &str, value: Yaml<'_>) -> Result<Meta, Fault> {
    let invalid = || {
        Fault::Invalid(format!(
            "{key} must be a value or a table of values by rule"
        ))
    };
    let text = |value: Yaml<'_>| value.text().map(str::to_owned).ok_or_else(invalid);
    match value {
        Yaml::Map(pairs) => pairs
            .into_iter()
            .map(|(rule, value)| Ok((rule.to_owned(), text(value)?)))
            .collect::<Result<_, Fault>>()
            .map(Meta::ByRule),
        value => text(value).map(Meta::All),
    }
}

/// The pairs of a mapping named `name`; an empty value has none.
fn entries<'a>(value: Yaml<'a>, name: &str) -> Result<Vec<(&'a str, Yaml<'a>)>, Fault> {
    match value {
        Yaml::Map(pairs) => Ok(pairs),
        Yaml::Null => Ok(Vec::new()),
        _ => Err(Fault::Invalid(format!("{name} must be a mapping"))),
    }
}

/// A frontmatter node as a header takes it: nulls apart, numbers read from
/// `!!int` and `!!float` scalars, and keys as text.
#[derive(Debug)]
enum Yaml<'a> {
    Null,
    Scalar { text: &'a str, number: Option<f64> },
    List(Vec<Yaml<'a>>),
    Map(Vec<(&'a str, Yaml<'a>)>),
}

impl<'a> Yaml<'a> {
    /// The header view of `node`, refusing aliases and keys that are not
    /// text, in source order.
    fn of(node: &'a YamlNode) -> Result<Self, String> {
        Ok(match &node.value {
            YamlValue::Scalar(_) if node.tag == "!!null" => Self::Null,
            YamlValue::Scalar(text) => Self::Scalar {
                text,
                number: matches!(node.tag.as_str(), "!!int" | "!!float")
                    .then(|| text.parse().ok())
                    .flatten(),
            },
            YamlValue::Sequence(items) => {
                Self::List(items.iter().map(Self::of).collect::<Result<_, _>>()?)
            }
            YamlValue::Mapping(pairs) => Self::Map(
                pairs
                    .iter()
                    .map(|(key, value)| {
                        let key = Self::of(key)?
                            .text()
                            .ok_or_else(|| "mapping keys must be scalars".to_owned())?;
                        Ok((key, Self::of(value)?))
                    })
                    .collect::<Result<_, String>>()?,
            ),
            YamlValue::Alias { .. } => return Err("aliases are not supported".to_owned()),
        })
    }

    /// The scalar as written.
    fn text(&self) -> Option<&'a str> {
        match self {
            Self::Scalar { text, .. } => Some(text),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::Frontmatter;

    fn read(yaml: &str) -> Result<Header, Fault> {
        let parsed = Frontmatter::parse("module.md", &format!("---\n{yaml}---\n"));
        assert_eq!(parsed.problems, []);
        let root = parsed.value.and_then(|frontmatter| frontmatter.root);
        Header::read(root.as_ref(), &BTreeSet::from(["stage".to_owned()]))
    }

    #[test]
    fn params_are_numbers_unless_quoted() {
        let header =
            read("params:\n  min-words: 12\n  ratio: 0.5\n  name: \"12\"\n  flag: true\n").unwrap();
        assert_eq!(
            header.params,
            BTreeMap::from([
                ("flag".to_owned(), Value::Text("true".to_owned())),
                ("min-words".to_owned(), Value::Number(12.0)),
                ("name".to_owned(), Value::Text("12".to_owned())),
                ("ratio".to_owned(), Value::Number(0.5)),
            ])
        );
    }

    #[test]
    fn data_keeps_empty_tables_and_their_shape() {
        let header = read("data:\n  outdated: {}\n  banned: [usage, setup]\n").unwrap();
        assert_eq!(header.data["outdated"], Table::Map(Vec::new()));
        assert_eq!(
            header.data["banned"],
            Table::List(vec!["usage".to_owned(), "setup".to_owned()])
        );
    }

    #[test]
    fn meta_keys_apply_to_every_rule_or_by_rule() {
        let all = read("stage: source\n").unwrap();
        assert_eq!(all.meta("any")["stage"], "source");
        let by_rule = read("stage:\n  large: rendered\n").unwrap();
        assert_eq!(by_rule.meta("large")["stage"], "rendered");
        assert!(by_rule.meta("other").is_empty());
    }

    #[test]
    fn undeclared_keys_and_bad_shapes_are_faults() {
        assert_eq!(
            read("owner: me\n").unwrap_err(),
            Fault::UnknownKey("owner".to_owned())
        );
        assert!(matches!(read("params: [1]\n"), Err(Fault::Invalid(_))));
        assert!(matches!(read("data:\n  t: x\n"), Err(Fault::Invalid(_))));
        assert!(matches!(read("stage: [a]\n"), Err(Fault::Invalid(_))));
    }

    #[test]
    fn aliases_and_null_keys_are_faults() {
        let invalid = |message: &str| Err(Fault::Invalid(message.to_owned()));
        assert_eq!(
            read("description: &d x\nstage: *d\n").map(drop),
            invalid("aliases are not supported")
        );
        assert_eq!(
            read("params:\n  ~: 1\n").map(drop),
            invalid("mapping keys must be scalars")
        );
    }
}
