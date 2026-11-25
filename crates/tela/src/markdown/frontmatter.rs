use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use saphyr_parser::{Event, Parser, ScalarStyle, Span, Tag};

use super::yaml::{self, Value};
use super::{Parsed, Problem};
use crate::tree::{Origin, Position};

/// One frontmatter scalar: the value of a key, or an element of a list value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The dotted key path, such as `nested.key`.
    pub key: String,
    /// The scalar value as written.
    pub value: String,
    /// The YAML short tag, such as `!!str` or `!!int`.
    pub tag: String,
    /// The element's index when the value is a list.
    pub index: Option<usize>,
    /// The source line of its key, or of the list element, 1-based.
    pub line: u32,
    /// The column there, 1-based.
    pub column: u32,
}

/// A file's leading YAML block between `---` fences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    /// The YAML between the fences, trimmed.
    pub text: String,
    /// The top-level keys whose values are strings, booleans or numbers,
    /// with the values as written by henia.
    pub attrs: BTreeMap<String, String>,
    /// The YAML document, if the block holds one.
    pub root: Option<YamlNode>,
    /// The lines the block spans, fences and the blank line after them
    /// included; body lines shift by this.
    pub lines: u32,
    /// The byte offset the body starts at.
    pub body: usize,
}

impl Frontmatter {
    /// Splits the frontmatter off `text` of `file`, if it opens with one. An
    /// unclosed block or YAML that does not read as a mapping is a problem
    /// on line 1.
    #[must_use]
    pub fn parse(file: &str, text: &str) -> Parsed<Option<Self>> {
        let problem = |message: String| Parsed {
            value: None,
            problems: vec![Problem {
                origin: Origin::named(&Arc::from(file), Position::new(0..0, 1..=1, 1).ok()),
                message,
            }],
        };
        let (header, body) = match fence(text) {
            None => {
                return Parsed {
                    value: None,
                    problems: Vec::new(),
                };
            }
            Some(Fence::Unclosed) => return problem("unclosed YAML frontmatter".to_owned()),
            Some(Fence::Closed { header, body }) => (header, body),
        };
        let document = match Document::read(header) {
            Ok(document) => document,
            Err(message) => return problem(format!("parse frontmatter: {message}")),
        };
        Parsed {
            value: Some(Self {
                text: header.trim().to_owned(),
                attrs: document.attrs,
                root: document.root,
                lines: line_count(&text[..body]),
                body,
            }),
            problems: Vec::new(),
        }
    }

    /// The scalar entries in source order: a mapping extends the key path,
    /// a sequence numbers its elements, and every other node is an entry
    /// at the key or element that leads to it.
    #[must_use]
    pub fn entries(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        if let Some(root) = &self.root {
            walk(&mut entries, "", None, root, root);
        }
        entries
    }
}

fn line_count(text: &str) -> u32 {
    u32::try_from(text.matches('\n').count()).unwrap_or(u32::MAX)
}

/// A leading `---` fence and what follows it.
enum Fence<'a> {
    /// The YAML between the fences and the byte offset of the body.
    Closed { header: &'a str, body: usize },
    /// No closing fence follows.
    Unclosed,
}

/// The fenced block `text` opens with, if any.
fn fence(text: &str) -> Option<Fence<'_>> {
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    if first.trim_end_matches('\r') != "---" {
        return None;
    }
    if first.len() == text.len() {
        return Some(Fence::Unclosed);
    }
    let mut end = 0;
    loop {
        let tail = &rest[end..];
        let (line, remaining) = tail
            .split_once('\n')
            .map_or((tail, None), |(l, r)| (l, Some(r)));
        if line.trim_end_matches('\r') == "---" {
            let remaining = remaining.unwrap_or_default();
            let body = remaining
                .strip_prefix("\r\n")
                .or_else(|| remaining.strip_prefix('\n'))
                .unwrap_or(remaining);
            return Some(Fence::Closed {
                header: &rest[..end],
                body: text.len() - body.len(),
            });
        }
        if remaining.is_none() {
            return Some(Fence::Unclosed);
        }
        end += line.len() + 1;
    }
}

/// A node of the frontmatter's YAML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlNode {
    /// What the node holds.
    pub value: YamlValue,
    /// The YAML short tag: a scalar's resolved tag, such as `!!str` or
    /// `!!int`; a collection's `!!seq` or `!!map` unless tagged otherwise;
    /// an alias's the tag of the node its anchor names.
    pub tag: String,
    /// The source line, 1-based and counted from the opening fence.
    pub line: u32,
    /// The column there, 1-based.
    pub column: u32,
}

/// What a [`YamlNode`] holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YamlValue {
    /// A scalar as written.
    Scalar(String),
    /// A sequence's elements in source order.
    Sequence(Vec<YamlNode>),
    /// A mapping's key-value pairs in source order.
    Mapping(Vec<(YamlNode, YamlNode)>),
    /// An alias.
    Alias {
        /// The anchor name.
        name: String,
        /// The scalar the anchor names, if it names one.
        scalar: Option<String>,
    },
}

impl YamlNode {
    /// A scalar's value or an alias's anchor name; empty for a collection.
    fn text(&self) -> &str {
        match &self.value {
            YamlValue::Scalar(value) | YamlValue::Alias { name: value, .. } => value,
            YamlValue::Sequence(_) | YamlValue::Mapping(_) => "",
        }
    }

    /// The scalar the node holds or names.
    fn scalar(&self) -> Option<&str> {
        match &self.value {
            YamlValue::Scalar(value) => Some(value),
            YamlValue::Alias { scalar, .. } => scalar.as_deref(),
            YamlValue::Sequence(_) | YamlValue::Mapping(_) => None,
        }
    }
}

/// The first YAML document of a header, read as henia reads it into a map.
struct Document {
    root: Option<YamlNode>,
    attrs: BTreeMap<String, String>,
}

impl Document {
    fn read(header: &str) -> Result<Self, String> {
        let mut reader = Reader {
            header,
            events: Parser::new_from_str(header),
            anchors: HashMap::new(),
        };
        let root = reader.document()?;
        let mut errors = Vec::new();
        let mut attrs = BTreeMap::new();
        if let Some(root) = &root {
            match &root.value {
                YamlValue::Mapping(pairs) => {
                    unique_keys(root, &mut errors);
                    if errors.is_empty() {
                        for (key, value) in pairs {
                            decode(value)?;
                            if let (YamlValue::Scalar(name), Some(content)) =
                                (&key.value, value.scalar())
                                && let Some(shown) = scalar(&value.tag, content)?.display()
                            {
                                attrs.insert(name.clone(), shown);
                            }
                        }
                    }
                }
                YamlValue::Scalar(_) if root.tag == "!!null" => {}
                YamlValue::Scalar(value) | YamlValue::Alias { name: value, .. } => {
                    let value = if value.len() > 10 {
                        format!(" `{}...`", value.chars().take(7).collect::<String>())
                    } else {
                        format!(" `{value}`")
                    };
                    errors.push(format!(
                        "line {}: cannot unmarshal {}{value} into map[string]interface {{}}",
                        root.line - 1,
                        root.tag
                    ));
                }
                YamlValue::Sequence(_) => errors.push(format!(
                    "line {}: cannot unmarshal !!seq into map[string]interface {{}}",
                    root.line - 1
                )),
            }
        }
        if !errors.is_empty() {
            return Err(format!(
                "yaml: unmarshal errors:\n  {}",
                errors.join("\n  ")
            ));
        }
        Ok(Self { root, attrs })
    }
}

/// yaml.v3's check that no mapping repeats a key, depth first.
fn unique_keys(node: &YamlNode, errors: &mut Vec<String>) {
    match &node.value {
        YamlValue::Mapping(pairs) => {
            let before = errors.len();
            for (i, (first, _)) in pairs.iter().enumerate() {
                for (later, _) in &pairs[i + 1..] {
                    if std::mem::discriminant(&first.value) == std::mem::discriminant(&later.value)
                        && first.text() == later.text()
                    {
                        errors.push(format!(
                            "line {}: mapping key {:?} already defined at line {}",
                            later.line - 1,
                            later.text(),
                            first.line - 1
                        ));
                    }
                }
            }
            if errors.len() == before {
                for (_, value) in pairs {
                    unique_keys(value, errors);
                }
            }
        }
        YamlValue::Sequence(items) => items.iter().for_each(|item| unique_keys(item, errors)),
        YamlValue::Scalar(_) | YamlValue::Alias { .. } => {}
    }
}

/// Decodes every scalar below `node`, failing as yaml.v3 does on a scalar
/// that does not decode as its tag.
fn decode(node: &YamlNode) -> Result<(), String> {
    match &node.value {
        YamlValue::Scalar(value) => scalar(&node.tag, value).map(drop),
        YamlValue::Sequence(items) => items.iter().try_for_each(decode),
        YamlValue::Mapping(pairs) => pairs.iter().try_for_each(|(_, value)| decode(value)),
        YamlValue::Alias { .. } => Ok(()),
    }
}

fn scalar(tag: &str, value: &str) -> Result<Value, String> {
    yaml::decode(tag, value).map_err(|message| format!("yaml: {message}"))
}

/// Pushes the entries below `node`, which the key or element `at` leads to.
fn walk(entries: &mut Vec<Entry>, key: &str, index: Option<usize>, node: &YamlNode, at: &YamlNode) {
    match &node.value {
        YamlValue::Mapping(pairs) => {
            for (k, value) in pairs {
                let path = format!("{key}.{}", k.text());
                let path = path.strip_prefix('.').unwrap_or(&path);
                walk(entries, path, index, value, k);
            }
        }
        YamlValue::Sequence(items) => {
            for (i, item) in items.iter().enumerate() {
                walk(entries, key, Some(i), item, item);
            }
        }
        YamlValue::Scalar(_) | YamlValue::Alias { .. } => entries.push(Entry {
            key: key.to_owned(),
            value: node.text().to_owned(),
            tag: node.tag.clone(),
            index,
            line: at.line,
            column: at.column,
        }),
    }
}

/// Builds the node tree of the first document from parser events.
struct Reader<'a> {
    header: &'a str,
    events: Parser<'a, saphyr_parser::StrInput<'a>>,
    anchors: HashMap<usize, (String, Option<String>)>,
}

impl<'a> Reader<'a> {
    fn next(&mut self) -> Result<Option<(Event<'a>, Span)>, String> {
        match self.events.next_event() {
            None => Ok(None),
            Some(Ok(event)) => Ok(Some(event)),
            Some(Err(error)) => Err(format!(
                "yaml: line {}: {}",
                error.marker().line(),
                error.info()
            )),
        }
    }

    fn document(&mut self) -> Result<Option<YamlNode>, String> {
        loop {
            match self.next()? {
                None | Some((Event::StreamEnd, _)) => return Ok(None),
                Some((Event::DocumentStart(_), _)) => break,
                Some(_) => {}
            }
        }
        match self.next()? {
            None | Some((Event::DocumentEnd | Event::StreamEnd, _)) => Ok(None),
            Some(event) => self.node(event).map(Some),
        }
    }

    fn node(&mut self, (event, span): (Event<'a>, Span)) -> Result<YamlNode, String> {
        let (value, tag, anchor) = match event {
            Event::Scalar(value, style, anchor, tag) => {
                let plain = style == ScalarStyle::Plain;
                let tag = yaml::scalar_tag(full_tag(tag.as_ref()).as_deref(), plain, &value);
                (YamlValue::Scalar(value.into_owned()), tag, anchor)
            }
            Event::SequenceStart(anchor, tag) => {
                let mut items = Vec::new();
                loop {
                    match self.next()? {
                        Some((Event::SequenceEnd, _)) | None => break,
                        Some(event) => items.push(self.node(event)?),
                    }
                }
                (
                    YamlValue::Sequence(items),
                    collection_tag(tag.as_ref(), "!!seq"),
                    anchor,
                )
            }
            Event::MappingStart(anchor, tag) => {
                let mut pairs = Vec::new();
                loop {
                    let key = match self.next()? {
                        Some((Event::MappingEnd, _)) | None => break,
                        Some(event) => self.node(event)?,
                    };
                    let value = match self.next()? {
                        Some((Event::MappingEnd, _)) | None => break,
                        Some(event) => self.node(event)?,
                    };
                    pairs.push((key, value));
                }
                (
                    YamlValue::Mapping(pairs),
                    collection_tag(tag.as_ref(), "!!map"),
                    anchor,
                )
            }
            Event::Alias(id) => {
                let name = alias_name(self.header, span);
                let (tag, scalar) = self.anchors.get(&id).cloned().unwrap_or_default();
                (YamlValue::Alias { name, scalar }, tag, 0)
            }
            _ => return Err("yaml: unexpected event".to_owned()),
        };
        let number = |n: usize| u32::try_from(n).unwrap_or(u32::MAX).saturating_add(1);
        let node = YamlNode {
            value,
            tag,
            line: number(span.start.line()),
            column: number(span.start.col()),
        };
        if anchor != 0 {
            let scalar = node.scalar().map(str::to_owned);
            self.anchors.insert(anchor, (node.tag.clone(), scalar));
        }
        Ok(node)
    }
}

fn full_tag(tag: Option<&Cow<'_, Tag>>) -> Option<String> {
    tag.map(|tag| match tag.handle.as_str() {
        "!!" => format!("tag:yaml.org,2002:{}", tag.suffix),
        handle => format!("{handle}{}", tag.suffix),
    })
}

fn collection_tag(tag: Option<&Cow<'_, Tag>>, default: &str) -> String {
    full_tag(tag)
        .filter(|tag| !tag.is_empty() && tag != "!")
        .map_or_else(|| default.to_owned(), |tag| yaml::short_tag(&tag))
}

/// The anchor name an alias at `span` names, read from the source.
fn alias_name(header: &str, span: Span) -> String {
    header
        .chars()
        .skip(span.start.index() + 1)
        .take_while(|c| !c.is_whitespace() && !",[]{}".contains(*c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Frontmatter {
        let parsed = Frontmatter::parse("a.md", text);
        assert_eq!(parsed.problems, []);
        parsed.value.unwrap()
    }

    fn problem(text: &str) -> String {
        let parsed = Frontmatter::parse("a.md", text);
        assert_eq!(parsed.value, None);
        parsed.problems.into_iter().next().unwrap().message
    }

    fn rows(frontmatter: &Frontmatter) -> Vec<String> {
        frontmatter
            .entries()
            .into_iter()
            .map(|e| {
                let index = e.index.map(|i| format!("[{i}]")).unwrap_or_default();
                format!(
                    "{}{index}={} {} L{}:{}",
                    e.key, e.value, e.tag, e.line, e.column
                )
            })
            .collect()
    }

    #[test]
    fn entries_carry_keys_tags_indexes_and_lines() {
        let text = "---\nname: tags\ncount: 12\nratio: 0.5\nenabled: true\nempty: null\nquoted: \"123\"\ndate: 2026-11-25\nlist: [a, b]\nnested:\n  key: value\nhenia.lint.disable: duplicate-heading\n---\n\n# Body\n";
        let frontmatter = parse(text);
        assert_eq!(
            rows(&frontmatter),
            [
                "name=tags !!str L2:1",
                "count=12 !!int L3:1",
                "ratio=0.5 !!float L4:1",
                "enabled=true !!bool L5:1",
                "empty=null !!null L6:1",
                "quoted=123 !!str L7:1",
                "date=2026-11-25 !!timestamp L8:1",
                "list[0]=a !!str L9:8",
                "list[1]=b !!str L9:11",
                "nested.key=value !!str L11:3",
                "henia.lint.disable=duplicate-heading !!str L12:1",
            ]
        );
        assert_eq!(frontmatter.lines, 14);
        assert_eq!(&text[frontmatter.body..], "# Body\n");
        assert!(frontmatter.text.starts_with("name: tags\n"));
        assert!(frontmatter.text.ends_with("duplicate-heading"));
        let attrs: Vec<_> = frontmatter.attrs.keys().map(String::as_str).collect();
        assert_eq!(
            attrs,
            [
                "count",
                "enabled",
                "henia.lint.disable",
                "name",
                "quoted",
                "ratio"
            ]
        );
    }

    #[test]
    fn block_lists_and_explicit_tags() {
        let frontmatter = parse("---\ntags:\n  - one\n  - !!int 2\nx: !custom y\n---\n");
        assert_eq!(
            rows(&frontmatter),
            [
                "tags[0]=one !!str L3:5",
                "tags[1]=2 !!int L4:5",
                "x=y !custom L5:1"
            ]
        );
        assert_eq!(frontmatter.attrs["x"], "y");
    }

    #[test]
    fn aliases_name_their_anchor_and_value() {
        let frontmatter = parse(
            "---
anchor: &a hello
ref: *a
---
",
        );
        assert_eq!(
            rows(&frontmatter),
            ["anchor=hello !!str L2:1", "ref=a !!str L3:1"]
        );
        assert_eq!(frontmatter.attrs["ref"], "hello");
    }

    #[test]
    fn files_without_a_fence_have_no_frontmatter() {
        let parsed = Frontmatter::parse("a.md", "# Title\n---\n");
        assert_eq!(parsed.value, None);
        assert_eq!(parsed.problems, []);
    }

    #[test]
    fn malformed_blocks_are_problems() {
        assert_eq!(problem("---\na: 1\n"), "unclosed YAML frontmatter");
        assert_eq!(problem("---"), "unclosed YAML frontmatter");
        assert_eq!(
            problem("---\na: 1\na: 2\n---\n"),
            "parse frontmatter: yaml: unmarshal errors:\n  line 2: mapping key \"a\" already defined at line 1"
        );
        assert_eq!(
            problem("---\nhello\n---\n"),
            "parse frontmatter: yaml: unmarshal errors:\n  line 1: cannot unmarshal !!str `hello` into map[string]interface {}"
        );
        assert_eq!(
            problem("---\nx: !!int abc\n---\n"),
            "parse frontmatter: yaml: cannot decode !!str `abc` as a !!int"
        );
        assert!(problem("---\na: [1\n---\n").starts_with("parse frontmatter: yaml: line"));
    }

    #[test]
    fn crlf_fences_close_the_block() {
        let text = "---\r\na: 1\r\n---\r\nbody\r\n";
        let frontmatter = parse(text);
        assert_eq!(&text[frontmatter.body..], "body\r\n");
        assert_eq!(frontmatter.lines, 3);
    }

    #[test]
    fn the_value_tree_keeps_empty_containers() {
        let frontmatter = parse("---\ndata:\n  outdated: {}\n  banned: []\n---\n");
        assert_eq!(rows(&frontmatter), [] as [String; 0]);
        let node = |value, tag: &str, line, column| YamlNode {
            value,
            tag: tag.to_owned(),
            line,
            column,
        };
        let key = |name: &str, line| node(YamlValue::Scalar(name.to_owned()), "!!str", line, 3);
        let data = YamlValue::Mapping(vec![
            (
                key("outdated", 3),
                node(YamlValue::Mapping(Vec::new()), "!!map", 3, 13),
            ),
            (
                key("banned", 4),
                node(YamlValue::Sequence(Vec::new()), "!!seq", 4, 11),
            ),
        ]);
        let root = YamlValue::Mapping(vec![(
            node(YamlValue::Scalar("data".to_owned()), "!!str", 2, 1),
            node(data, "!!map", 3, 3),
        )]);
        assert_eq!(frontmatter.root, Some(node(root, "!!map", 2, 1)));
    }
}
