//! A module's frontmatter: its params, data tables and the caller's meta keys.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use saphyr_parser::{Event, Parser, ScalarStyle, StrInput, Tag};

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
    /// A key neither tela nor the caller declares.
    UnknownKey(String),
    /// A value of the wrong shape, or YAML the reader cannot take.
    Invalid(String),
}

impl Header {
    /// Reads the YAML of a module's frontmatter, accepting `meta_keys`
    /// beyond tela's own `description`, `params` and `data`.
    pub(super) fn read(yaml: &str, meta_keys: &BTreeSet<String>) -> Result<Self, Fault> {
        let mut header = Self::default();
        let pairs = match Reader::new(yaml).document().map_err(Fault::Invalid)? {
            Yaml::Map(pairs) => pairs,
            Yaml::Null => return Ok(header),
            _ => return Err(Fault::Invalid("frontmatter must be a mapping".to_owned())),
        };
        for (key, value) in pairs {
            match key.as_str() {
                "description" => {}
                "params" => header.params = params(value)?,
                "data" => header.data = data(value)?,
                _ if meta_keys.contains(&key) => {
                    let meta = meta(&key, value)?;
                    header.meta.insert(key, meta);
                }
                _ => return Err(Fault::UnknownKey(key)),
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

fn params(value: Yaml) -> Result<BTreeMap<String, Value>, Fault> {
    entries(value, "params")?
        .into_iter()
        .map(|(key, value)| match value {
            Yaml::Scalar {
                number: Some(n), ..
            } => Ok((key, Value::Number(n))),
            Yaml::Scalar { text, .. } => Ok((key, Value::Text(text))),
            _ => Err(Fault::Invalid(format!(
                "param {key} must be a number, string or boolean"
            ))),
        })
        .collect()
}

fn data(value: Yaml) -> Result<BTreeMap<String, Table>, Fault> {
    entries(value, "data")?
        .into_iter()
        .map(|(key, value)| {
            let rows = |text: Option<String>| {
                text.ok_or_else(|| {
                    Fault::Invalid(format!(
                        "data {key}: rows must be strings, numbers or booleans"
                    ))
                })
            };
            let table = match value {
                Yaml::Map(pairs) => Table::Map(
                    pairs
                        .into_iter()
                        .map(|(row, value)| Ok((row, rows(value.into_text())?)))
                        .collect::<Result<_, Fault>>()?,
                ),
                Yaml::List(items) => Table::List(
                    items
                        .into_iter()
                        .map(|value| rows(value.into_text()))
                        .collect::<Result<_, Fault>>()?,
                ),
                _ => {
                    return Err(Fault::Invalid(format!(
                        "data {key} must be a table or a list"
                    )));
                }
            };
            Ok((key, table))
        })
        .collect()
}

fn meta(key: &str, value: Yaml) -> Result<Meta, Fault> {
    let invalid = || {
        Fault::Invalid(format!(
            "{key} must be a value or a table of values by rule"
        ))
    };
    match value {
        Yaml::Map(pairs) => pairs
            .into_iter()
            .map(|(rule, value)| Ok((rule, value.into_text().ok_or_else(invalid)?)))
            .collect::<Result<_, Fault>>()
            .map(Meta::ByRule),
        value => value.into_text().map(Meta::All).ok_or_else(invalid),
    }
}

/// The pairs of a mapping named `name`; an empty value has none.
fn entries(value: Yaml, name: &str) -> Result<Vec<(String, Yaml)>, Fault> {
    match value {
        Yaml::Map(pairs) => Ok(pairs),
        Yaml::Null => Ok(Vec::new()),
        _ => Err(Fault::Invalid(format!("{name} must be a mapping"))),
    }
}

/// A YAML node, with scalars resolved as the core schema resolves them.
#[derive(Debug)]
enum Yaml {
    Null,
    Scalar { text: String, number: Option<f64> },
    List(Vec<Yaml>),
    Map(Vec<(String, Yaml)>),
}

impl Yaml {
    /// The scalar as written.
    fn into_text(self) -> Option<String> {
        match self {
            Self::Scalar { text, .. } => Some(text),
            _ => None,
        }
    }

    fn scalar(text: Cow<'_, str>, style: ScalarStyle, tag: Option<&Tag>) -> Self {
        let tag = tag.map(|tag| tag.suffix.as_str());
        let plain = style == ScalarStyle::Plain;
        match tag {
            Some("null") => return Self::Null,
            None if plain && matches!(&*text, "" | "~" | "null" | "Null" | "NULL") => {
                return Self::Null;
            }
            _ => {}
        }
        let number = match tag {
            Some("int" | "float") => text.parse().ok(),
            None if plain
                && text
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b)) =>
            {
                text.parse().ok()
            }
            _ => None,
        };
        Self::Scalar {
            text: text.into_owned(),
            number,
        }
    }
}

/// Builds the first document of a header from parser events.
struct Reader<'a> {
    events: Parser<'a, StrInput<'a>>,
}

impl<'a> Reader<'a> {
    fn new(yaml: &'a str) -> Self {
        Self {
            events: Parser::new_from_str(yaml),
        }
    }

    fn next(&mut self) -> Result<Event<'a>, String> {
        match self.events.next_event() {
            None => Ok(Event::StreamEnd),
            Some(Ok((event, _))) => Ok(event),
            Some(Err(error)) => Err(format!("line {}: {}", error.marker().line(), error.info())),
        }
    }

    fn document(&mut self) -> Result<Yaml, String> {
        loop {
            match self.next()? {
                Event::StreamEnd | Event::DocumentEnd => return Ok(Yaml::Null),
                Event::StreamStart | Event::DocumentStart(_) | Event::Nothing => {}
                event => return self.node(event),
            }
        }
    }

    fn node(&mut self, event: Event<'a>) -> Result<Yaml, String> {
        match event {
            Event::Scalar(text, style, _, tag) => Ok(Yaml::scalar(text, style, tag.as_deref())),
            Event::SequenceStart(..) => {
                let mut items = Vec::new();
                loop {
                    match self.next()? {
                        Event::SequenceEnd => return Ok(Yaml::List(items)),
                        event => items.push(self.node(event)?),
                    }
                }
            }
            Event::MappingStart(..) => {
                let mut pairs = Vec::new();
                loop {
                    let key = match self.next()? {
                        Event::MappingEnd => return Ok(Yaml::Map(pairs)),
                        event => self.node(event)?,
                    };
                    let key = key
                        .into_text()
                        .ok_or_else(|| "mapping keys must be scalars".to_owned())?;
                    let event = self.next()?;
                    pairs.push((key, self.node(event)?));
                }
            }
            Event::Alias(_) => Err("aliases are not supported".to_owned()),
            _ => Err("unexpected end of the YAML".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(yaml: &str) -> Result<Header, Fault> {
        Header::read(yaml, &BTreeSet::from(["stage".to_owned()]))
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
}
