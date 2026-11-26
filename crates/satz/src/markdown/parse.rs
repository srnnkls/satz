use std::fmt;
use std::sync::Arc;

use markdown_it::MarkdownIt;
use markdown_it::links::LinkFormatter;

use super::attributes::Fault;
use super::builder::{self, Element};
use super::frontmatter::{self, Frontmatter};
use super::{Document, Parsed, directive, document, mapping, render};
use crate::frontend::Frontend;
use crate::tree::{Node, NodeId, Origin, Position, Tree};

/// The Markdown frontend: Markdown with tables, raw HTML, directives and
/// YAML frontmatter, read into the core node types.
///
/// The file node holds a `frontmatter` node with an `entry` per scalar, a
/// `problem` node for malformed frontmatter or directives, and the body's
/// sections and blocks. Node positions are relative to the body; origins are
/// source positions.
pub struct Markdown {
    rules: Vec<Arc<dyn super::InlineRule>>,
    pub(super) parsers: Arc<Parsers>,
}

/// The parsers of a frontend, which its documents keep for rendering.
pub(super) struct Parsers {
    /// Reads Markdown bodies.
    pub(super) body: MarkdownIt,
    /// Reads directive labels: paragraphs of inline Markdown, no other
    /// blocks.
    pub(super) labels: MarkdownIt,
    /// The characters the registered inline rules start with.
    pub(super) triggers: Vec<char>,
}

impl Parsers {
    fn new(rules: &[Arc<dyn super::InlineRule>]) -> Self {
        let mut body = MarkdownIt::empty();
        markdown_it::plugins::cmark::add(&mut body);
        mapping::add(&mut body);
        markdown_it::plugins::html::add(&mut body);
        markdown_it::plugins::extra::tables::add(&mut body);
        directive::add(&mut body);
        directive::add_rules(&mut body, rules);
        body.link_formatter = Box::new(RawLinks);
        let mut labels = render::labels();
        directive::add_rules(&mut labels, rules);
        Self {
            body,
            labels,
            triggers: rules.iter().map(|rule| rule.trigger()).collect(),
        }
    }
}

impl Markdown {
    /// A frontend with its parser built once for every file it reads.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            parsers: Arc::new(Parsers::new(&[])),
        }
    }

    /// The frontend with `rule` registered after the rules it has: its
    /// trees, and the documents and renderings of [`Markdown::document`],
    /// read the text `rule` claims as an inline directive.
    #[must_use]
    pub fn with_inline_rule(mut self, rule: Arc<dyn super::InlineRule>) -> Self {
        self.rules.push(rule);
        self.parsers = Arc::new(Parsers::new(&self.rules));
        self
    }

    /// Parses the Markdown body `text` of `file` into an editable document,
    /// as [`Document::parse`] does, with this frontend's inline rules.
    #[must_use]
    pub fn document(&self, file: &str, text: &str) -> Parsed<Document> {
        document::parse(self, file, text)
    }

    /// An empty document for `file`, as [`Document::new`] makes, whose
    /// renderings apply this frontend's inline rules.
    #[must_use]
    pub fn empty_document(&self, file: impl Into<String>) -> Document {
        Document::empty(file.into(), Arc::clone(&self.parsers))
    }
}

impl Default for Markdown {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Markdown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Markdown").finish_non_exhaustive()
    }
}

impl Frontend for Markdown {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn extensions(&self) -> &[&str] {
        &["md"]
    }

    fn parse(&self, file: &str, text: &str) -> Tree {
        let file: Arc<str> = Arc::from(file);
        let parsed = Frontmatter::parse(&file, text);
        let frontmatter = parsed.value;
        let (offset, shift) = frontmatter.as_ref().map_or_else(
            || (frontmatter::body_offset(text), 0),
            |head| (head.body, head.lines),
        );
        let body = &text[offset..];
        let mut root = Node::new("file", Origin::named(&file, None));
        body.clone_into(&mut root.body);
        let mut tree = Tree::new(root);
        let root = tree.root().id();
        if let Some(problem) = parsed.problems.into_iter().next() {
            let mut node = Node::problem(problem.message, problem.origin.clone());
            node.attrs
                .insert("kind".to_owned(), "frontmatter".to_owned());
            node.position = problem.origin.position().cloned();
            let _ = tree.append(root, node);
            return tree;
        }
        if let Some(head) = &frontmatter {
            append_frontmatter(&mut tree, root, &file, text, head);
        }
        let doc = self.parsers.body.parse(body);
        let nuls = Nuls::new(body);
        let faults = directive::faults(&doc);
        if !faults.is_empty() {
            let at = (shift, offset);
            let _ = tree.append(root, problem(&file, body, &faults, at, &nuls));
        }
        let place = Place {
            file: &file,
            shift,
            offset,
            nuls,
        };
        for element in builder::build(&doc, true).children {
            place.graft(&mut tree, root, element);
        }
        tree
    }
}

/// Keeps link destinations as the Markdown spec reads them.
#[derive(Debug)]
struct RawLinks;

impl LinkFormatter for RawLinks {
    fn validate_link(&self, _: &str) -> Option<()> {
        Some(())
    }

    fn normalize_link(&self, url: &str) -> String {
        url.to_owned()
    }

    fn normalize_link_text(&self, url: &str) -> String {
        url.to_owned()
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The byte span of 1-based `line` of `text`, without its line break.
fn line_span(text: &str, line: u32) -> std::ops::Range<usize> {
    let start = text
        .match_indices('\n')
        .nth(
            usize::try_from(line)
                .unwrap_or(usize::MAX)
                .saturating_sub(2),
        )
        .filter(|_| line > 1)
        .map_or(0, |(at, _)| at + 1);
    let end = text[start..].find('\n').map_or(text.len(), |n| start + n);
    start..end
}

fn append_frontmatter(
    tree: &mut Tree,
    root: NodeId,
    file: &Arc<str>,
    text: &str,
    head: &Frontmatter,
) {
    let position = Position::new(0..head.body, 1..=head.lines.max(1), 1).ok();
    let mut node = Node::new("frontmatter", Origin::named(file, position.clone()));
    node.attrs.clone_from(&head.attrs);
    node.text.clone_from(&head.text);
    node.position = position;
    let Ok(id) = tree.append(root, node) else {
        return;
    };
    for entry in head.entries() {
        let position = Position::new(
            line_span(text, entry.line),
            entry.line..=entry.line,
            entry.column,
        )
        .ok();
        let mut node = Node::new("entry", Origin::named(file, position.clone()));
        node.attrs.insert("key".to_owned(), entry.key.clone());
        node.attrs.insert("value".to_owned(), entry.value.clone());
        node.attrs.insert("tag".to_owned(), entry.tag.clone());
        if let Some(index) = entry.index {
            node.attrs.insert("index".to_owned(), index.to_string());
        }
        node.text.clone_from(&entry.value);
        node.position = position;
        if tree.append(id, node).is_err() {
            return;
        }
    }
}

/// henia's markup problem: every fault located in the body, one per line,
/// at the first fault's position.
fn problem(
    file: &Arc<str>,
    body: &str,
    faults: &[Fault],
    (shift, offset): (u32, usize),
    nuls: &Nuls,
) -> Node {
    let located: Vec<(usize, u32, u32)> = faults
        .iter()
        .map(|fault| {
            let offset = nuls.body(fault.offset).min(body.len());
            let head = &body[..offset];
            let line = head.matches('\n').count() + 1;
            let column = offset - head.rfind('\n').map_or(0, |p| p + 1) + 1;
            (offset, count(line), count(column))
        })
        .collect();
    let message = faults
        .iter()
        .zip(&located)
        .map(|(fault, (_, line, column))| match fault.id {
            Some(id) => format!("{line}:{column}: {id}: {}", fault.message),
            None => format!("{line}:{column}: {}", fault.message),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let position = located.first().and_then(|&(offset, line, column)| {
        Position::new(offset..offset, line..=line, column).ok()
    });
    let mut node = Node::problem(
        message,
        Origin::named(file, position.as_ref().map(|p| p.moved(shift, offset))),
    );
    node.attrs.insert("kind".to_owned(), "markup".to_owned());
    node.position = position;
    node
}

/// Where elements land: their file, the lines and bytes frontmatter shifts
/// them by, and the body's NULs.
struct Place<'a> {
    file: &'a Arc<str>,
    shift: u32,
    offset: usize,
    nuls: Nuls,
}

/// The body's NULs, which the parser reads as the three bytes of U+FFFD:
/// their offsets in the parsed text, and the body's line starts when there
/// are any.
struct Nuls {
    parsed: Vec<usize>,
    lines: Vec<usize>,
}

impl Nuls {
    fn new(body: &str) -> Self {
        let parsed: Vec<usize> = body
            .bytes()
            .enumerate()
            .filter(|&(_, b)| b == 0)
            .enumerate()
            .map(|(before, (at, _))| at + 2 * before)
            .collect();
        let lines = if parsed.is_empty() {
            Vec::new()
        } else {
            std::iter::once(0)
                .chain(body.match_indices('\n').map(|(at, _)| at + 1))
                .collect()
        };
        Self { parsed, lines }
    }

    /// The body offset of offset `at` of the parsed text.
    fn body(&self, at: usize) -> usize {
        at.saturating_sub(2 * self.parsed.partition_point(|&nul| nul < at))
    }

    /// The body column of the parsed text's `column` on 1-based `line`.
    fn column(&self, line: usize, column: usize) -> usize {
        let Some(&start) = line.checked_sub(1).and_then(|i| self.lines.get(i)) else {
            return column;
        };
        let parsed_start = start + 2 * self.parsed.partition_point(|&nul| self.body(nul) < start);
        self.body(parsed_start + column.saturating_sub(1)) - start + 1
    }
}

impl Place<'_> {
    fn graft(&self, tree: &mut Tree, parent: NodeId, element: Element) {
        let (parsed, end) = marked(&element);
        let start = self.nuls.body(parsed);
        let position = Position::new(
            start..self.nuls.body(end).max(start),
            count(element.line)..=count(element.end_line.max(element.line)),
            count(self.nuls.column(element.line, element.column)),
        )
        .ok();
        let mut node = Node::new(
            element.kind,
            Origin::named(
                self.file,
                position.as_ref().map(|p| p.moved(self.shift, self.offset)),
            ),
        );
        node.attrs = element.attrs;
        if let Some(label) = element.label {
            node.attrs.insert("label".to_owned(), label);
        }
        node.text = element.text;
        node.position = position;
        let Ok(id) = tree.append(parent, node) else {
            return;
        };
        for child in element.children {
            self.graft(tree, id, child);
        }
    }
}

/// The parsed span `element` edits: a list, item or quote from its first
/// marker, a heading from its marker through its underline or closing `#`s.
fn marked(element: &Element) -> (usize, usize) {
    let (start, end) = (element.start.unwrap_or_default(), element.end);
    match (element.kind, element.source) {
        ("list" | "item" | "quote", Some((marker, _))) => (marker.min(start), end),
        ("heading", Some((marker, last))) => (marker, end.max(last)),
        _ => (start, end),
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    fn outline(tree: &Tree) -> String {
        fn walk(node: crate::tree::NodeRef<'_>, depth: usize, out: &mut String) {
            let data = node.node();
            let mut label = data.kind.clone();
            if let Some(id) = data.attrs.get("id").filter(|id| !id.is_empty()) {
                label = format!("{label}#{id}");
            }
            if let Some(lang) = data.attrs.get("lang").filter(|lang| !lang.is_empty()) {
                label = format!("{label}:{lang}");
            }
            let (line, end) = data
                .position
                .as_ref()
                .map_or((1, 1), |p| (p.line(), p.end_line()));
            let _ = writeln!(out, "{}{label} L{line}-{end}", "  ".repeat(depth));
            for child in node.children() {
                walk(child, depth + 1, out);
            }
        }
        let mut out = String::new();
        for child in tree.root().children() {
            walk(child, 0, &mut out);
        }
        out
    }

    fn parse(text: &str) -> Tree {
        Markdown::new().parse("a.md", text)
    }

    fn problems(tree: &Tree) -> Vec<String> {
        tree.problems().map(|p| p.node().text.clone()).collect()
    }

    fn find<'a>(tree: &'a Tree, kind: &str) -> Vec<crate::tree::NodeRef<'a>> {
        tree.root()
            .descendants()
            .filter(|n| n.node().kind == kind)
            .collect()
    }

    fn span_text<'a>(text: &'a str, node: crate::tree::NodeRef<'_>) -> &'a str {
        let span = node
            .node()
            .position
            .as_ref()
            .map(Position::span)
            .unwrap_or_default();
        &text[span]
    }

    #[test]
    fn tree() {
        let tree = parse(
            "# Title\n\nIntro [x](u).\n\n## Usage\n\nFirst.\n\n```bash\necho\n```\n\n- one\n- two\n\n### Deep\n\n> quote\n\n## Usage\n\n:::note\ninside\n:::\n",
        );
        assert_eq!(problems(&tree), Vec::<String>::new());
        assert_eq!(
            outline(&tree),
            "section#title L1-24
  heading L1-1
  paragraph L3-3
    link L3-3
  section#usage L5-18
    heading L5-5
    paragraph L7-7
    code:bash L9-11
    list L13-14
      item L13-13
        paragraph L13-13
      item L14-14
        paragraph L14-14
    section#deep L16-18
      heading L16-16
      quote L18-18
        paragraph L18-18
  section#usage-1 L20-24
    heading L20-20
    directive L22-24
      paragraph L23-23
"
        );
    }

    #[test]
    fn anchors_count_every_heading() {
        let tree = parse("# A\n\n## B b\n\n## B b\n\n### C: d\n\n> # A\n\n# A\n");
        let ids: Vec<_> = find(&tree, "section")
            .into_iter()
            .map(|s| s.node().attrs["id"].clone())
            .collect();
        assert_eq!(ids, ["a", "b-b", "b-b-1", "c-d", "a-2"]);
    }

    #[test]
    fn sections_inside_directives() {
        let tree = parse(
            "# Skill\n\n:::note\n## Routes\n\nGo.\n\n## Slots\n\nNone.\n:::\n\n## Detail\n\nMore.\n",
        );
        let got: Vec<_> = find(&tree, "section")
            .into_iter()
            .map(|s| {
                let parent = s
                    .parent()
                    .map(|p| p.node().kind.clone())
                    .unwrap_or_default();
                format!("{}<{parent}", s.node().attrs["id"])
            })
            .collect();
        assert_eq!(
            got,
            [
                "skill<file",
                "routes<directive",
                "slots<directive",
                "detail<section"
            ]
        );
    }

    #[test]
    fn concurrent_parses_keep_their_own_problems() {
        let markdown = Markdown::new();
        std::thread::scope(|scope| {
            for i in 0..16 {
                let markdown = &markdown;
                scope.spawn(move || {
                    let broken = i % 2 == 0;
                    let source = if broken {
                        "::: note\nbody\n"
                    } else {
                        "::: note\nbody\n:::\n"
                    };
                    let tree = markdown.parse("a.md", source);
                    assert_eq!(tree.problems().count(), usize::from(broken), "{source:?}");
                });
            }
        });
    }

    #[test]
    fn directive_syntax() {
        let text = ":::routes{ordered protocol}\n:::\n\n::item[see [doc](u) and :term[[ref](v)]]{#main k=\"a b\"}\n\nafter\n";
        let tree = parse(text);
        assert_eq!(problems(&tree), Vec::<String>::new());
        let top: Vec<_> = tree.root().children().collect();
        assert_eq!(top.len(), 3);
        let (routes, item, after) = (top[0].node(), top[1].node(), top[2].node());
        assert_eq!(routes.attrs["name"], "routes");
        assert_eq!(routes.attrs["ordered"], "true");
        assert_eq!(routes.attrs["protocol"], "true");
        assert_eq!(item.kind, "directive");
        assert_eq!(item.attrs["name"], "item");
        assert!(!item.attrs.contains_key("inline"));
        assert_eq!(
            item.position.as_ref().map(|p| (p.line(), p.end_line())),
            Some((4, 4))
        );
        assert_eq!(item.attrs["id"], "main");
        assert_eq!(item.attrs["k"], "a b");
        assert_eq!(
            (after.kind.as_str(), after.text.as_str()),
            ("paragraph", "after")
        );
        let mut label = Vec::new();
        for c in top[1].children() {
            let attr = |n: &Node, k: &str| n.attrs.get(k).cloned().unwrap_or_default();
            let n = c.node();
            label.push(format!("{}:{}{}", n.kind, attr(n, "url"), attr(n, "name")));
            for g in c.children() {
                let n = g.node();
                label.push(format!(
                    "  {}:{}{}",
                    n.kind,
                    attr(n, "url"),
                    attr(n, "name")
                ));
            }
        }
        assert_eq!(label, ["link:u", "directive:term", "  link:v"]);
    }

    #[test]
    fn every_directive_error_is_reported() {
        let tree = parse("intro\n\n:x[a]{k=1 k=2}\n\n:::menu\nbody\n");
        assert_eq!(
            problems(&tree),
            [
                "3:11: duplicate-attribute: attribute \"k\" is repeated\n5:1: unclosed-directive: directive menu is not closed"
            ]
        );
        let problem = tree.problems().next().unwrap().node();
        assert_eq!(problem.attrs["kind"], "markup");
        assert_eq!(problem.origin.to_string(), "a.md:3:11");
    }

    #[test]
    fn labels_resolve_reference_links() {
        for text in [
            ":x[[doc][r]]\n\n[r]: https://example.test\n",
            ":x[:y[[doc][r]]]\n\n[r]: https://example.test\n",
        ] {
            let tree = parse(text);
            let urls: Vec<_> = find(&tree, "link")
                .into_iter()
                .map(|l| l.node().attrs["url"].clone())
                .collect();
            assert_eq!(urls, ["https://example.test"], "{text:?}");
        }
    }

    #[test]
    fn every_block_is_kept() {
        let tree = parse(":::x\n---\n:::\n\n<div>\nhi\n</div>\n\n> ***\n");
        assert_eq!(problems(&tree), Vec::<String>::new());
        assert_eq!(
            outline(&tree),
            "directive L1-3\n  rule L2-2\nhtml L5-7\nquote L9-9\n  rule L9-9\n"
        );
    }

    #[test]
    fn inline_code_spans_its_ticks() {
        let text = "Write `{{.name}}` in ``a ` b``.\n";
        let tree = parse(text);
        let got: Vec<_> = find(&tree, "code")
            .into_iter()
            .map(|c| format!("{}:{}", c.node().attrs["inline"], span_text(text, c)))
            .collect();
        assert_eq!(got, ["true:`{{.name}}`", "true:``a ` b``"]);
    }

    #[test]
    fn unclosed_empty_directive_in_an_item() {
        let tree = parse("- :::note\nb\n  :::\n");
        let problems = problems(&tree);
        assert!(problems[0].starts_with("1:3: ") && problems[0].contains("unclosed-directive"));
        let note = find(&tree, "directive")[0];
        assert_eq!(note.node().text, ":::note");
        assert_eq!(
            note.node().position.as_ref().map(Position::span),
            Some(2..9)
        );
    }

    #[test]
    fn link_spans_cover_code_span_labels() {
        for (text, code) in [
            ("[`peer path`](../peer/SKILL.md#x)\n", "`peer path`"),
            ("[see `b`](y.md)\n", "`b`"),
            ("[`a` then](y.md)\n", "`a`"),
            ("[``a`b``](y.md)\n", "``a`b``"),
            ("[*`e` f*](w.md)\n", "`e`"),
            ("[**`e`**](w.md)\n", "`e`"),
        ] {
            let tree = parse(text);
            let (link, span) = (find(&tree, "link")[0], find(&tree, "code")[0]);
            assert_eq!(span_text(text, span), code);
            let (l, c) = (
                link.node()
                    .position
                    .as_ref()
                    .map(Position::span)
                    .unwrap_or_default(),
                span.node()
                    .position
                    .as_ref()
                    .map(Position::span)
                    .unwrap_or_default(),
            );
            assert!(c.start >= l.start && c.end <= l.end, "{text:?}");
        }
    }

    #[test]
    fn table_rows_and_cells() {
        let text = "# Conventions\n\n| Axis | Rule |\n| --- | --- |\n| Errors | Wrap with [context](u). |\n| Effects |   |\n| Panics |\n||\n\n> | Quoted | Too |\n> | --- | --- |\n> | a | b |\n";
        let tree = parse(text);
        let got: Vec<_> = tree
            .root()
            .descendants()
            .filter(|n| matches!(n.node().kind.as_str(), "table" | "row" | "cell" | "link"))
            .map(|n| {
                let n = n.node();
                let attrs: String = ["index", "header", "column"]
                    .iter()
                    .filter_map(|k| n.attrs.get(*k).map(|v| format!(" {k}={v}")))
                    .collect();
                let p = n.position.as_ref().unwrap();
                format!(
                    "{}{attrs} L{}:{}-{} {:?}",
                    n.kind,
                    p.line(),
                    p.column(),
                    p.end_line(),
                    n.text
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                r#"table L3:1-8 "| Axis | Rule |\n| --- | --- |\n| Errors | Wrap with [context](u). |\n| Effects |   |\n| Panics |\n||""#,
                r#"row index=0 header=true L3:1-3 "| Axis | Rule |""#,
                r#"cell index=0 column=Axis L3:3-3 "Axis""#,
                r#"cell index=1 column=Rule L3:10-3 "Rule""#,
                r#"row index=1 header=false L5:1-5 "| Errors | Wrap with [context](u). |""#,
                r#"cell index=0 column=Axis L5:3-5 "Errors""#,
                r#"cell index=1 column=Rule L5:12-5 "Wrap with [context](u).""#,
                r#"link L5:23-5 "context""#,
                r#"row index=2 header=false L6:1-6 "| Effects |   |""#,
                r#"cell index=0 column=Axis L6:3-6 "Effects""#,
                r#"cell index=1 column=Rule L6:15-6 """#,
                r#"row index=3 header=false L7:1-7 "| Panics |""#,
                r#"cell index=0 column=Axis L7:3-7 "Panics""#,
                r#"cell index=1 column=Rule L7:11-7 """#,
                r#"row index=4 header=false L8:1-8 "||""#,
                r#"cell index=0 column=Axis L8:3-8 """#,
                r#"cell index=1 column=Rule L8:3-8 """#,
                r#"table L10:3-12 "| Quoted | Too |\n> | --- | --- |\n> | a | b |""#,
                r#"row index=0 header=true L10:3-10 "| Quoted | Too |""#,
                r#"cell index=0 column=Quoted L10:5-10 "Quoted""#,
                r#"cell index=1 column=Too L10:14-10 "Too""#,
                r#"row index=1 header=false L12:3-12 "| a | b |""#,
                r#"cell index=0 column=Quoted L12:5-12 "a""#,
                r#"cell index=1 column=Too L12:9-12 "b""#,
            ]
        );
    }

    #[test]
    fn frontmatter_shifts_origins_but_not_positions() {
        let text = "---\na: 1\n---\n\n\n\n\n\n\n\n\n\n\n# Heading\n";
        let tree = parse(text);
        let heading = find(&tree, "heading")[0].node();
        assert_eq!(heading.position.as_ref().map(Position::line), Some(10));
        assert_eq!(heading.origin.line(), Some(14));
        assert_eq!(heading.origin.to_string(), "a.md:14:3");
        let span = heading.origin.position().unwrap().span();
        assert_eq!(&text[span], "# Heading");
        let head: Vec<_> = tree
            .root()
            .children()
            .map(|c| c.node().kind.clone())
            .collect();
        assert_eq!(head[0], "frontmatter");
        let entry = find(&tree, "entry")[0].node();
        assert_eq!(entry.origin.to_string(), "a.md:2:1");
        assert_eq!(&text[entry.position.as_ref().unwrap().span()], "a: 1");
    }

    #[test]
    fn broken_frontmatter_is_the_only_child() {
        let tree = parse("---\na: [\n# Title\n");
        let kids: Vec<_> = tree
            .root()
            .children()
            .map(|c| c.node().kind.clone())
            .collect();
        assert_eq!(kids, ["problem"]);
        let problem = tree.problems().next().unwrap().node();
        assert_eq!(problem.attrs["kind"], "frontmatter");
        assert_eq!(problem.text, "unclosed YAML frontmatter");
        assert_eq!(problem.origin.to_string(), "a.md:1:1");
    }

    #[test]
    fn the_file_body_is_the_text_after_its_frontmatter() {
        for (text, body) in [
            ("# T\n", "# T\n"),
            ("---\na: 1\n---\n\n# T\n", "# T\n"),
            ("---\na: [\n---\nafter\n", "after\n"),
            ("---\na: [\n# T\n", "---\na: [\n# T\n"),
        ] {
            let tree = parse(text);
            let file = tree.root().node();
            assert_eq!(file.body, body, "{text:?}");
            assert!(file.text.is_empty());
        }
    }

    #[test]
    fn goldmark_quirks_are_fixed() {
        let text = "# Links\n\n[ref]: https://example.com/a\n\nSee <https://example.com/auto> and [x](a\\_b.md) and [y](a&amp;b.md).\n";
        let tree = parse(text);
        assert_eq!(find(&tree, "paragraph").len(), 1);
        let links: Vec<_> = find(&tree, "link")
            .into_iter()
            .map(|l| {
                let n = l.node();
                (
                    n.attrs["url"].clone(),
                    n.text.clone(),
                    n.position.as_ref().map(Position::line),
                )
            })
            .collect();
        assert_eq!(
            links,
            [
                (
                    "https://example.com/auto".to_owned(),
                    "https://example.com/auto".to_owned(),
                    Some(5)
                ),
                ("a_b.md".to_owned(), "x".to_owned(), Some(5)),
                ("a&b.md".to_owned(), "y".to_owned(), Some(5)),
            ]
        );
    }

    #[test]
    fn hostile_input_never_panics() {
        let deep = ":a[".repeat(500) + &"]".repeat(500);
        for text in [
            "",
            "\0",
            ":::",
            "::",
            ":",
            "---",
            "- :::x\n",
            &deep,
            "\u{feff}# A\r\n\r\n> - [x](\0)\n",
        ] {
            let _ = parse(text);
        }
    }

    #[test]
    fn positions_index_the_body_around_nuls() {
        let text = "\0\n***\n\0\0 `c`\n";
        let tree = parse(text);
        assert_eq!(span_text(text, find(&tree, "rule")[0]), "***");
        let code = find(&tree, "code")[0];
        assert_eq!(span_text(text, code), "`c`");
        assert_eq!(code.node().position.as_ref().map(Position::column), Some(4));
    }

    #[test]
    fn heading_columns_point_at_the_title_with_and_without_nuls() {
        for (text, column) in [("# Title\n", 3), ("\0\n## \0Title\n", 4)] {
            let tree = parse(text);
            let heading = find(&tree, "heading")[0];
            assert_eq!(
                heading.node().position.as_ref().map(Position::column),
                Some(column),
                "{text:?}"
            );
        }
    }

    fn rows(tree: &Tree) -> Vec<(String, String, u32, u32)> {
        tree.root()
            .descendants()
            .skip(1)
            .map(|n| {
                let data = n.node();
                let (line, end) = data
                    .origin
                    .position()
                    .map_or((0, 0), |p| (p.line(), p.end_line()));
                (data.kind.clone(), data.text.clone(), line, end)
            })
            .collect()
    }

    fn row(kind: &str, text: &str, line: u32, end: u32) -> (String, String, u32, u32) {
        (kind.to_owned(), text.to_owned(), line, end)
    }

    #[test]
    fn text_after_a_partly_stripped_tab_keeps_its_place() {
        let tree = parse("- a\n \t}\n-\tb");
        assert_eq!(
            rows(&tree),
            [
                row("list", "a\n \t}\n-\tb", 1, 3),
                row("item", "a\n \t}", 1, 2),
                row("paragraph", "a\n \t}", 1, 2),
                row("item", "b", 3, 3),
                row("paragraph", "b", 3, 3),
            ]
        );
    }

    #[test]
    fn partly_stripped_tabs_match_henia() {
        for (text, kind, nth, want) in [
            ("- a\n\t}\n- b\n", "paragraph", 0, "a\n\t}"),
            ("- a\n   \t}\n- b\n", "paragraph", 0, "a\n   \t}"),
            ("1. a\n  \t}\n2. b\n", "paragraph", 0, "a\n  \t}"),
            ("- a\n  - b\n   \tc\n  - d\n", "paragraph", 1, "b\n   \tc"),
            ("- > a\n  > \tb\n- c\n", "quote", 0, "a\n  > \tb"),
            ("- a\n\n  > b\n \t> c\n- d\n", "quote", 0, "b\n \t> c"),
            ("- x\n\n  a\n \tb\n  ===\n- c\n", "heading", 0, "a\nb"),
            ("- a\n \t[l](u) `c`\n- b\n", "link", 0, "l"),
            ("- a\n \t`x\n \ty` z\n- b\n", "code", 0, "x\n \ty"),
            (
                "- a\n \tb\\\n \tc\n- d\n",
                "paragraph",
                0,
                "a\n \tb\\\n \tc",
            ),
            ("- a\n\n \t\tcode\n- b\n", "code", 0, "code"),
            ("- a\n\n \t\t\tcode\n- b\n", "code", 0, "\tcode"),
            ("    \tcode\n", "code", 0, "\tcode"),
            ("- a\n\n \t```\n \t\tx\n \t```\n- b\n", "code", 0, "\tx\n"),
            (
                "- a\n\n \t<div>\n \tx\n \t</div>\n- b\n",
                "html",
                0,
                "<div>\n \tx\n \t</div>",
            ),
        ] {
            let tree = parse(text);
            let node = find(&tree, kind)[nth].node();
            assert_eq!(node.text, want, "{text:?}");
        }
    }

    #[derive(Debug)]
    struct Sigil;

    impl crate::markdown::InlineRule for Sigil {
        fn trigger(&self) -> char {
            '$'
        }

        fn claim(&self, src: &str) -> Option<crate::markdown::Claim> {
            let name = src.strip_prefix('$')?;
            let len = name
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(name.len());
            (len > 0).then(|| crate::markdown::Claim {
                len: 1 + len,
                name: "ref".into(),
                label: name[..len].into(),
                attrs: [("via".to_owned(), "sigil".to_owned())]
                    .into_iter()
                    .collect(),
            })
        }
    }

    #[derive(Debug)]
    struct Greedy;

    impl crate::markdown::InlineRule for Greedy {
        fn trigger(&self) -> char {
            ':'
        }

        fn claim(&self, src: &str) -> Option<crate::markdown::Claim> {
            let len = src.find(char::is_whitespace).unwrap_or(src.len());
            (len > 1).then(|| crate::markdown::Claim {
                len,
                name: "sugar".into(),
                label: src[1..len].into(),
                attrs: std::iter::empty().collect(),
            })
        }
    }

    #[test]
    fn registered_inline_rules_yield_directives_at_their_source() {
        let text = "---\na: 1\n---\nSee $git and :ref[git]{via=sigil} or :ref[**x** y].\n";
        let plain = parse(text);
        let tree = Markdown::new()
            .with_inline_rule(Arc::new(Sigil))
            .parse("a.md", text);
        assert_eq!(problems(&tree), Vec::<String>::new());
        let body = &tree.root().node().body;
        let directives: Vec<_> = find(&tree, "directive")
            .into_iter()
            .map(|n| {
                let d = n.node();
                let span = d.position.as_ref().map(Position::span).unwrap_or_default();
                (
                    d.kind.clone(),
                    d.attrs.clone(),
                    body[span].to_owned(),
                    d.origin.to_string(),
                )
            })
            .collect();
        let attrs = |pairs: &[(&str, &str)]| -> std::collections::BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect()
        };
        let sigil = attrs(&[
            ("inline", "true"),
            ("label", "git"),
            ("name", "ref"),
            ("via", "sigil"),
        ]);
        assert_eq!(
            directives,
            [
                (
                    "directive".to_owned(),
                    sigil.clone(),
                    "$git".to_owned(),
                    "a.md:4:5".to_owned()
                ),
                (
                    "directive".to_owned(),
                    sigil,
                    ":ref[git]{via=sigil}".to_owned(),
                    "a.md:4:14".to_owned()
                ),
                (
                    "directive".to_owned(),
                    attrs(&[("inline", "true"), ("label", "**x** y"), ("name", "ref")]),
                    ":ref[**x** y]".to_owned(),
                    "a.md:4:38".to_owned()
                ),
            ]
        );
        assert_eq!(
            find(&plain, "directive").len(),
            2,
            "a rule of one instance only"
        );
    }

    #[test]
    fn built_in_directives_run_before_registered_rules() {
        let tree = Markdown::new()
            .with_inline_rule(Arc::new(Greedy))
            .parse("a.md", ":ref[git] :$git\n");
        let names: Vec<_> = find(&tree, "directive")
            .into_iter()
            .map(|n| n.node().attrs["name"].clone())
            .collect();
        assert_eq!(names, ["ref", "sugar"]);
    }

    #[test]
    fn documents_of_an_instance_render_its_rules() {
        let markdown = Markdown::new().with_inline_rule(Arc::new(Sigil));
        let parsed = markdown.document("a.md", "See $git now.\n");
        assert_eq!(parsed.problems, []);
        let mut doc = parsed.value;
        assert_eq!(doc.render(crate::markdown::RenderFormat::Xml), []);
        assert_eq!(doc.to_string(), "See <ref via=\"sigil\">git</ref> now.\n");
    }

    #[test]
    fn empty_documents_of_an_instance_render_its_rules() {
        let markdown = Markdown::new().with_inline_rule(Arc::new(Sigil));
        let parsed = markdown.document("a.md", "See $git now.\n");
        let mut doc = markdown.empty_document("out.md");
        doc.children = parsed.value.children;
        assert_eq!(doc.render(crate::markdown::RenderFormat::Xml), []);
        assert_eq!(doc.to_string(), "See <ref via=\"sigil\">git</ref> now.\n");
    }
}
