//! henia's pattern tests, ported, with satz's schema, environment and
//! regexp checks.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::*;
use crate::frontend::Frontend;
use crate::lint::module::Module;
use crate::markdown::Markdown;
use crate::similarity::{Embed, EmbedError};
use crate::template::{Sigil, placeholders};
use crate::tree::{Node, NodeRef, Origin};

fn schema() -> Schema {
    let keys = |kind: &str, keys: &[&str]| {
        (
            kind.to_owned(),
            keys.iter().map(|&k| k.to_owned()).collect::<BTreeSet<_>>(),
        )
    };
    Schema::new(SchemaSpec {
        types: vec![TypeSpec {
            name: "skill".to_owned(),
            keys: vec!["id".to_owned(), "name".to_owned(), "package".to_owned()],
        }],
        keys: BTreeMap::from([
            keys("file", &["main", "kind", "file"]),
            keys("link", &["target", "path", "anchor"]),
        ]),
        unit: Some(Unit {
            kind: "skill".to_owned(),
            id_key: "id".to_owned(),
            qualifier_key: Some("package".to_owned()),
        }),
        ..SchemaSpec::default()
    })
    .unwrap()
}

/// A corpus of skills with files and links, as henia's pattern tests build
/// one: link targets resolve through the environment.
struct Corpus {
    tree: Tree,
    env: Environment,
    schema: Schema,
}

impl Corpus {
    fn new(skills: &[(&str, &[(&str, &str)])], links: &[(&str, &[&str])]) -> Self {
        let origin = || Origin::new("corpus", None).unwrap();
        let mut tree = Tree::new(Node::new(CORPUS, origin()));
        let root = tree.root().id();
        let mut env = Environment::new(UNIX_EPOCH);
        for &(id, files) in skills {
            let mut skill = Node::new("skill", origin());
            skill.attrs.insert("id".to_owned(), id.to_owned());
            let skill = tree.append(root, skill).unwrap();
            env.targets.insert(id.to_owned(), skill);
            let mut main = None;
            for path in ["SKILL.md", "refs/more.md"] {
                let Some(&(_, text)) = files.iter().find(|(p, _)| *p == path) else {
                    continue;
                };
                let file = tree
                    .graft(skill, Markdown::new().parse(path, text))
                    .unwrap();
                let node = tree.node_mut(file).unwrap();
                node.attrs.insert("path".to_owned(), path.to_owned());
                node.attrs
                    .insert("main".to_owned(), (path == "SKILL.md").to_string());
                main.get_or_insert(file);
            }
            let specs = links
                .iter()
                .find(|(s, _)| *s == id)
                .map_or(&[][..], |l| l.1);
            for spec in specs {
                let mut link = Node::new("link", origin());
                let (target, anchor) = spec.split_once('#').unwrap_or((spec, ""));
                link.attrs.insert("target".to_owned(), target.to_owned());
                if !anchor.is_empty() {
                    link.attrs.insert("path".to_owned(), "SKILL.md".to_owned());
                    link.attrs.insert("anchor".to_owned(), anchor.to_owned());
                }
                (*spec).clone_into(&mut link.text);
                tree.append(main.unwrap(), link).unwrap();
            }
        }
        Self {
            tree,
            env,
            schema: schema(),
        }
    }

    fn read(&self, query: &str) -> Query {
        Query::read(query, &self.schema, None)
            .unwrap_or_else(|e| panic!("read {query:?}: {e} ({:?})", e.hint()))
    }

    fn run(&self, query: &str) -> Vec<Row> {
        self.run_with(query, &self.env)
    }

    fn run_with(&self, query: &str, env: &Environment) -> Vec<Row> {
        self.read(query).run(&self.tree, env).unwrap()
    }

    fn node(&self, hit: &Hit) -> Option<NodeRef<'_>> {
        match hit {
            Hit::Node(id) => self.tree.get(*id),
            Hit::Row(_) => None,
        }
    }

    /// Each row as `capture=text+text`, captures in query order.
    fn texts(&self, query: &str, rows: &[Row]) -> Vec<String> {
        let query = self.read(query);
        rows.iter()
            .map(|row| {
                query
                    .captures()
                    .iter()
                    .map(|name| {
                        let texts: Vec<&str> = hits(row, name)
                            .iter()
                            .map(|hit| self.node(hit).map_or("", |n| n.node().text.as_str()))
                            .collect();
                        format!("{name}={}", texts.join("+"))
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    fn texts_of(&self, query: &str) -> Vec<String> {
        self.texts(query, &self.run(query))
    }

    /// Each row as `capture=ids` in query order, a link by its text, rows
    /// sorted.
    fn idents(&self, query: &str) -> Vec<String> {
        self.identify(query, false)
    }

    /// As [`Self::idents`], with each row's cells sorted too.
    fn sorted_cells(&self, query: &str) -> Vec<String> {
        self.identify(query, true)
    }

    fn identify(&self, query: &str, sort_cells: bool) -> Vec<String> {
        let rows = self.run(query);
        let query = self.read(query);
        let mut out: Vec<String> = rows
            .iter()
            .map(|row| {
                let mut cells: Vec<String> = query
                    .captures()
                    .iter()
                    .map(|name| {
                        let ids: Vec<&str> = hits(row, name)
                            .iter()
                            .filter_map(|hit| self.node(hit))
                            .map(|n| {
                                let node = n.node();
                                if node.kind == "link" {
                                    node.text.as_str()
                                } else {
                                    node.attrs.get("id").map_or("", String::as_str)
                                }
                            })
                            .collect();
                        format!("{name}={}", ids.join("+"))
                    })
                    .collect();
                if sort_cells {
                    cells.sort();
                }
                cells.join(" ")
            })
            .collect();
        out.sort();
        out
    }

    /// Each row as sorted parts: node captures by skill id or trimmed text,
    /// bound value captures as `@name=v+v`, variables as `?name=v`.
    fn groups(&self, query: &Query, rows: &[Row]) -> Vec<String> {
        let mut out: Vec<String> = rows
            .iter()
            .map(|row| {
                let mut parts = Vec::new();
                for name in query.captures() {
                    if query.values.contains(name) {
                        let values: Vec<String> = match row.captures.get(name) {
                            Some(Capture::Value(v)) => vec![v.to_string()],
                            Some(Capture::Values(vs)) => vs.iter().map(Value::to_string).collect(),
                            _ => continue,
                        };
                        parts.push(format!("@{name}={}", values.join("+")));
                        continue;
                    }
                    let nodes: Vec<String> = hits(row, name)
                        .iter()
                        .filter_map(|hit| self.node(hit))
                        .map(|n| {
                            let node = n.node();
                            if node.kind == "skill" {
                                node.attrs["id"].clone()
                            } else {
                                node.text.trim().to_owned()
                            }
                        })
                        .collect();
                    parts.push(format!("{name}={}", nodes.join("+")));
                }
                parts.extend(row.vars.iter().map(|(k, v)| format!("?{k}={v}")));
                parts.sort();
                parts.join(" ")
            })
            .collect();
        out.sort();
        out
    }

    fn grouped(&self, query: &str, env: &Environment) -> Vec<String> {
        let q = self.read(query);
        let rows = q.run(&self.tree, env).unwrap();
        self.groups(&q, &rows)
    }
}

/// The nodes a capture holds in a row.
fn hits<'a>(row: &'a Row, name: &str) -> &'a [Hit] {
    match row.captures.get(name) {
        Some(Capture::Nodes(hits) | Capture::Collected(hits)) => hits,
        _ => &[],
    }
}

fn fixture() -> Corpus {
    Corpus::new(
        &[
            (
                "a",
                &[
                    (
                        "SKILL.md",
                        "# A\n\nIntro.\n\n## Usage\n\nFirst para.\n\nSecond has Term here.\n\nThird para.\n\n```bash\necho top\n```\n\n### Deeper\n\n```bash\necho deep\n```\n\n```go\nfunc x() {}\n```\n\n## Other\n\nTerm leads this section.\n\n- item one\n- item two\n",
                    ),
                    ("refs/more.md", "# More\n\nResource para with term.\n"),
                ],
            ),
            (
                "b",
                &[("SKILL.md", "# B\n\nPlain.\n\nSecond has Term here too.\n")],
            ),
            (
                "c",
                &[("SKILL.md", "# C\n\nNo links.\n\n## Usage\n\nC usage.\n")],
            ),
        ],
        &[("a", &["b", "c#usage", "c#nope"]), ("b", &["a", "c"])],
    )
}

fn self_linked() -> Corpus {
    Corpus::new(
        &[
            ("a", &[("SKILL.md", "# A\n\nIntro.\n")]),
            ("b", &[("SKILL.md", "# B\n\nIntro.\n")]),
            (
                "c",
                &[("SKILL.md", "# C\n\nIntro.\n\n## Usage\n\nC usage.\n")],
            ),
        ],
        &[("a", &["a"]), ("b", &["a", "c#usage"])],
    )
}

fn chained() -> Corpus {
    Corpus::new(
        &[
            ("a", &[("SKILL.md", "# A\n\nIntro.\n")]),
            ("b", &[("SKILL.md", "# B\n\nIntro.\n")]),
            (
                "c",
                &[("SKILL.md", "# C\n\nIntro.\n\n## Usage\n\nC usage.\n")],
            ),
        ],
        &[("a", &["b"]), ("b", &["c#usage"])],
    )
}

fn grouped() -> Corpus {
    Corpus::new(
        &[
            (
                "a",
                &[(
                    "SKILL.md",
                    "# A\n\nOne two three.\n\nSeven eight.\n\n```go\nx\n```\n\n```bash\ny\n```\n\n```bash\nz\n```\n",
                )],
            ),
            ("b", &[("SKILL.md", "# B\n\nFour five.\n\n```go\nw\n```\n")]),
            ("c", &[("SKILL.md", "# C\n\nSix.\n")]),
        ],
        &[],
    )
}

fn tabled() -> Corpus {
    Corpus::new(
        &[(
            "a",
            &[(
                "SKILL.md",
                "# A\n\n## Conventions\n\n| Axis | Rule |\n| --- | --- |\n| Errors | Wrap them. |\n\n## Invariants\n\n| Invariant | Check |\n| --- | --- |\n| Rows bound | `go test` |\n| Lines exact |  |\n",
            )],
        )],
        &[],
    )
}

fn axes() -> BTreeMap<String, Table> {
    BTreeMap::from([(
        "axes".to_owned(),
        Table::List(vec!["Errors".to_owned(), "Effects".to_owned()]),
    )])
}

fn read_error(query: &str) -> ReadError {
    match Query::read(query, &schema(), None) {
        Ok(_) => panic!("{query}: read, want an error"),
        Err(e) => e,
    }
}

fn module_error(source: &str) -> ReadError {
    match Module::read(source, &schema(), |name: &str| {
        Err(format!("no module {name}"))
    }) {
        Ok(_) => panic!("{source}: read, want an error"),
        Err(e) => e,
    }
}

fn mentions(error: &ReadError) -> String {
    format!("{} {}", error.message(), error.hint().unwrap_or_default())
}

#[test]
fn run_joins_negations_and_relations() {
    let c = fixture();
    let cases: &[(&str, &[&str])] = &[
        (
            r#"(skill :id "a" (heading) @h)"#,
            &["h=A", "h=Usage", "h=Deeper", "h=Other", "h=More"],
        ),
        (
            r#"(skill :id "a" (paragraph)? @prev . (paragraph :contains "term") @hit . (paragraph)? @next)"#,
            &[
                "prev=First para. hit=Second has Term here. next=Third para.",
                "prev= hit=Term leads this section. next=",
                "prev= hit=Resource para with term. next=",
            ],
        ),
        (
            r#"(section :id "usage" (code :lang "bash") @c)"#,
            &["c=echo top\n", "c=echo deep\n"],
        ),
        (
            r#"(section :id "usage" > (code :lang "bash") @c)"#,
            &["c=echo top\n"],
        ),
        (r#"(skill (link :target "a")) @s"#, &["s="]),
        (
            r#"(skill :id "c" (inbound (skill) @from))"#,
            &["from=", "from="],
        ),
        (
            "(skill :id ?x (heading :text ?t) @h1) (skill :id (not ?x) (heading :text ?t) @h2)",
            &["h1=Usage h2=Usage", "h1=Usage h2=Usage"],
        ),
        (
            "(skill (link :target ?s :path ?p :anchor ?a) @l) (not (skill :id ?s (file :path ?p (section :id ?a))))",
            &["l=c#nope"],
        ),
        (
            r#"(skill :id "a" (code :lang ?l) @c (not (section :id "usage" > (code :lang ?l))))"#,
            &["c=func x() {}\n"],
        ),
        (
            "(paragraph :words 4..)",
            &[
                "=Second has Term here.",
                "=Term leads this section.",
                "=Resource para with term.",
                "=Second has Term here too.",
            ],
        ),
        (
            "(skill :id ?x (paragraph :text ?t) @a) (skill :id (after ?x) (paragraph :text (near ?t 0.6)) @b)",
            &["a=Second has Term here. b=Second has Term here too."],
        ),
        (
            "(link (to (section) @s)) @l",
            &["s=## Usage\n\nC usage. l=c#usage"],
        ),
        (r#"(link :target "c" (to (skill) @s)) @l"#, &["s= l=c"]),
        ("(link :anchor /./ (not (to (section)))) @l", &["l=c#nope"]),
        (
            r#"(section :id "usage" (from (link) @l)) @s"#,
            &["l=c#usage s=## Usage\n\nC usage."],
        ),
        (
            r#"(section :level 2 :title (not "Usage") > (heading) @h)"#,
            &["h=Other"],
        ),
        ("(skill (not (inbound (skill)))) @s", &[]),
        ("(skill (not (reaches (skill)))) @s", &["s="]),
        (
            r#"(skill :id "a" (reaches (skill) @t))"#,
            &["t=", "t=", "t="],
        ),
        (
            "(section :level 2 (not (code)) (heading) @h)",
            &["h=Other", "h=Usage"],
        ),
        (
            "(section :level 3 (heading) @title (code) @c)",
            &["title=Deeper c=echo deep\n", "title=Deeper c=func x() {}\n"],
        ),
    ];
    for (query, want) in cases {
        assert_eq!(c.texts_of(query), *want, "{query}");
    }
}

#[test]
fn run_shapes_and_values() {
    let c = fixture();
    let cases: &[(&str, &[&str])] = &[
        (r#"(heading :level 1..1 :matches "^[BC]$")"#, &["=B", "=C"]),
        (
            r#"(section :id "deeper" [(code :lang "go") (code :lang "rust")] @c)"#,
            &["c=func x() {}\n"],
        ),
        ("(item . (paragraph) @p)", &["p=item one", "p=item two"]),
        (r#"(file :main "false" (heading) @h)"#, &["h=More"]),
        ("(section :title /^D/ > (heading) @h)", &["h=Deeper"]),
        (
            r#"(skill :id "a" (section :id "usage" > (heading) @h))"#,
            &["h=Usage"],
        ),
    ];
    for (query, want) in cases {
        assert_eq!(c.texts_of(query), *want, "{query}");
    }
}

#[test]
fn reach_follows_targets() {
    let c = fixture();
    let rows = c.run(r#"(skill :id "a" (reaches (skill) @t))"#);
    let ids: Vec<&str> = rows
        .iter()
        .map(|row| c.node(&hits(row, "t")[0]).unwrap().node().attrs["id"].as_str())
        .collect();
    assert_eq!(ids, ["a", "b", "c"]);
}

/// Queries the reader rejects, with the message, hint and offset of the
/// error.
const READ_ERRORS: &[(&str, &str, &str, usize)] = &[
    (
        "(skill (headng) @h)",
        r#"unknown type "headng""#,
        "did you mean heading?",
        8,
    ),
    (
        r#"(skill :id "x" (heading)"#,
        "unclosed (",
        "add the matching )",
        0,
    ),
    (
        r#"(code :language "go")"#,
        "code has no key :language",
        "did you mean lang?",
        6,
    ),
    (
        "(skill :id 'x')",
        r"unexpected '\''",
        "quote strings with \"",
        11,
    ),
    (
        "(heading :level two)",
        ":level takes a number or a range",
        ":level 2",
        16,
    ),
    (
        r#"(paragraph :matches "(")"#,
        "invalid regular expression",
        "",
        20,
    ),
    (
        "(not (code))",
        "a query needs a pattern outside (not ...)",
        "",
        0,
    ),
    (
        "(heading)?",
        "a query needs a pattern that is not optional",
        "",
        0,
    ),
    ("(section . )", "anchor . needs a pattern after it", "", 11),
    ("(link :url /^https)", "unterminated /regexp/", "", 11),
    (
        "(heading :text ?t) (skill)",
        "a query of several patterns prints its captures",
        "",
        0,
    ),
    (
        "(paragraph :text (near ?t 2))",
        "(near ?x T) takes a threshold above 0 and at most 1",
        "e.g. (near ?t 0.7)",
        26,
    ),
    (
        r#"(paragraph :text (near "x" 0.5))"#,
        "expected a ?variable",
        "",
        23,
    ),
    (
        "(paragraph :text (twin ?t))",
        "expected not, after, before, contains",
        "",
        18,
    ),
    ("(skill :id ?x)", "?x appears only once", "use ?x again", 11),
    (
        r#"(skill :id "a" (heading :text (not ?t)) @h) (skill :id "b" (heading :text (not ?t)))"#,
        "?t is only compared, never bound",
        "bind it with :key ?t",
        35,
    ),
    (
        "(skill (paragraph :text ?t)? @p (heading :text ?t)) @s",
        "?t is bound in an optional pattern",
        "",
        24,
    ),
    (
        "(skill :id ?s (not (section :id ?a @x)) (link :anchor ?a :target ?s)) @k",
        "@x is inside (not ...), so it never prints",
        "capture a value outside the negation",
        35,
    ),
    (
        "(skill (heading :text ?t)) @a (skill (heading :text ?t)) @b (code) @c",
        "shares no variable with the first",
        "share a ?variable",
        60,
    ),
    (
        "(skill :id ?s (link :target ?s) @l) (not (section :id ?a (heading :text ?a)))",
        "(not ...) shares no variable",
        "",
        41,
    ),
    (
        "(skill [(heading :text ?t) (code :lang ?l)] (paragraph :text ?t)) @s",
        "alternatives bind different variables: ?t and ?l",
        "",
        27,
    ),
    (
        "(heading :contains ?t)",
        ":contains takes a string, not a variable",
        "bind the text with :text ?x",
        19,
    ),
];

#[test]
fn read_errors() {
    for &(query, message, hint, offset) in READ_ERRORS {
        let e = read_error(query);
        assert!(
            e.message().contains(message)
                && e.hint().unwrap_or_default().contains(hint)
                && e.offset() == offset,
            "{query}: got {:?} / {:?} at {}, want {message:?} / {hint:?} at {offset}",
            e.message(),
            e.hint(),
            e.offset(),
        );
    }
}

#[test]
fn read_errors_carry_their_kind() {
    assert_eq!(
        read_error("(skill (headng) @h)").kind(),
        ReadErrorKind::UnknownType
    );
    assert_eq!(
        read_error(r#"(code :language "go")"#).kind(),
        ReadErrorKind::UnknownKey
    );
    assert_eq!(read_error("(skill :id ?x)").kind(), ReadErrorKind::Unbound);
    assert_eq!(read_error("(skill :id 'x')").kind(), ReadErrorKind::Syntax);
}

#[derive(Debug)]
struct TermModel;

impl Embed for TermModel {
    fn model(&self) -> &'static str {
        "term"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        Ok(if text.to_lowercase().contains("term") {
            vec![1.0, 0.0]
        } else {
            vec![0.0, 0.0]
        })
    }
}

#[test]
fn similar_needs_a_model_and_skips_zero_vectors() {
    let c = fixture();
    let source = r#"(skill :id "a" (paragraph :text ?t) @a) (skill :id "b" (paragraph :text (similar ?t 0.9)) @b)"#;
    let query = c.read(source);
    assert!(matches!(query.run(&c.tree, &c.env), Err(RunError::NoModel)));
    let mut env = c.env.clone();
    env.embed = Some(Arc::new(TermModel));
    let rows = query.run(&c.tree, &env).unwrap();
    assert_eq!(
        c.texts(source, &rows),
        [
            "a=Second has Term here. b=Second has Term here too.",
            "a=Term leads this section. b=Second has Term here too.",
            "a=Resource para with term. b=Second has Term here too.",
        ]
    );
}

fn october_fifth() -> SystemTime {
    UNIX_EPOCH + Duration::from_hours(497_544)
}

#[test]
fn environment_params_data_and_clock() {
    let c = fixture();
    let mut env = c.env.clone();
    env.params = BTreeMap::from([
        ("max".to_owned(), Value::Number(10.0)),
        ("days".to_owned(), Value::Number(30.0)),
    ]);
    env.data = BTreeMap::from([
        (
            "outdated".to_owned(),
            Table::Map(vec![("Term".to_owned(), "Concept".to_owned())]),
        ),
        (
            "dates".to_owned(),
            Table::List(vec!["2020-01-01".to_owned(), "2026-09-30".to_owned()]),
        ),
    ]);
    env.now = october_fifth();
    let cases = [
        ("(file :lines (> $max)) @f", "f="),
        (
            "(heading :norm ?t :node ?n) @dup (heading :norm ?t :node (before ?n)) @first",
            "dup=Usage first=Usage",
        ),
        (
            r#"(row :table "outdated" :key ?old) @r (paragraph :text (contains ?old)) @p"#,
            "r= p=Second has Term here.|r= p=Term leads this section.|r= p=Resource para with term.|r= p=Second has Term here too.",
        ),
        (r#"(row :table "dates" :value (older $days)) @r"#, "r="),
    ];
    for (query, want) in cases {
        let rows = c.run_with(query, &env);
        assert_eq!(c.texts(query, &rows).join("|"), want, "{query}");
    }
    let query = c.read("(file :lines (> $missing)) @f");
    let missing = query.run(&c.tree, &c.env).unwrap_err();
    assert!(missing.to_string().contains("$missing has no value"));
    let query = c.read("(file :lines (> $max)) @f");
    let mut text = c.env.clone();
    text.params = BTreeMap::from([("max".to_owned(), Value::Text("many".to_owned()))]);
    let error = query.run(&c.tree, &text).unwrap_err();
    assert!(error.to_string().contains("compares it as a number"));
}

#[test]
fn data_rows_are_hits_with_their_attributes() {
    let c = fixture();
    let mut env = c.env.clone();
    env.data = BTreeMap::from([(
        "outdated".to_owned(),
        Table::Map(vec![("Term".to_owned(), "Concept".to_owned())]),
    )]);
    let rows = c.run_with(r#"(row :table "outdated") @r"#, &env);
    let row = BTreeMap::from([
        ("table".to_owned(), "outdated".to_owned()),
        ("key".to_owned(), "Term".to_owned()),
        ("value".to_owned(), "Concept".to_owned()),
    ]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].captures["r"], Capture::Nodes(vec![Hit::Row(row)]));
    let anywhere = c.run_with("(file (row) @r) @f", &env);
    assert!(anywhere.is_empty(), "data rows sit outside every file");
}

#[test]
fn capture_rows() {
    let c = grouped();
    let cases: &[(&str, &[&str])] = &[
        (r#"(code :lang "go") @c"#, &["c=w", "c=x"]),
        (
            "(code :lang ?l @lang) @c",
            &[
                "?l=bash @lang=bash c=y",
                "?l=bash @lang=bash c=z",
                "?l=go @lang=go c=w",
                "?l=go @lang=go c=x",
            ],
        ),
        (
            "(skill :id ?s @s (code) @c) (group ?s (count @c @n))",
            &["?s=a @n=3 @s=a c=x+y+z", "?s=b @n=1 @s=b c=w"],
        ),
        (
            r#"(code :lang "bash" @l) @c"#,
            &["@l=bash c=y", "@l=bash c=z"],
        ),
        (r#"(skill :id "b" @s)"#, &["@s=b"]),
        (
            "(code :lang ?l @l) @c (group @l (count @c @n))",
            &["@l=bash @n=2 c=y+z", "@l=go @n=2 c=x+w"],
        ),
        (
            "(code :lang ?l @l) @c (group (count @l @k))",
            &["@k=2 @l=go+bash c=x+y+z+w"],
        ),
    ];
    for (query, want) in cases {
        assert_eq!(c.grouped(query, &c.env), *want, "{query}");
    }
}

#[test]
fn score_capture() {
    let c = Corpus::new(
        &[
            (
                "a",
                &[(
                    "SKILL.md",
                    "# A\n\nKeep the branch short and rebase it often.\n",
                )],
            ),
            (
                "b",
                &[(
                    "SKILL.md",
                    "# B\n\nKeep the branch short and rebase it daily.\n",
                )],
            ),
        ],
        &[],
    );
    let rows = c.run(
        "(skill :id ?x (paragraph :text ?t) @a) (skill :id (after ?x) (paragraph :text (near ?t 0.3 @score)) @b)",
    );
    assert_eq!(rows.len(), 1);
    assert!(
        matches!(rows[0].value("score"), Some(Value::Number(n)) if *n > 0.0),
        "@score = {:?}",
        rows[0].value("score")
    );
    assert!(!rows[0].vars.contains_key("score"));
}

#[test]
fn capture_rejected() {
    let cases = [
        ("(skill @s (code))", "captures what precedes it"),
        ("(skill (code) @c @d)", "captures what precedes it"),
        ("(code :lang ?l @c) @c", "already names a value"),
        ("(code :lang ?l @l :text ?t @l) @c", "already captured"),
        ("(skill (code :lang ?l @l)+ @c) @s", "under * or +"),
        (
            "(skill :id ?s (not (code :lang ?l @l)) (link :target ?s)) @k",
            "inside (not ...)",
        ),
        (
            "(paragraph :text ?t) @a (paragraph :text (near ?t 0.6 ?score)) @b",
            "@score, not ?score",
        ),
        ("(code :lang ?l) @c (group ?l (count @c ?n))", "@n, not ?n"),
        (
            "(code :lang ?l) @c (group ?l (count @c @c))",
            "already captured",
        ),
        (
            "(code :lang ?l) @c (group @n (count @c @n))",
            "cannot key it",
        ),
        (
            "(code :lang ?l @l) @c (group (sum :words @l @w))",
            "captures a value",
        ),
    ];
    for (query, want) in cases {
        let e = read_error(query);
        assert!(mentions(&e).contains(want), "{query}: {e:?}, want {want:?}");
    }
}

#[test]
fn sort_by_value_capture() {
    let cases = [
        ("(code :lang ?l) @c (group ?l (count @c @n))", "-@n", ""),
        ("(code :lang ?l @l) @c", "@l", ""),
        ("(code :lang ?l) @c", "@c", "captures nodes"),
        (
            "(code :lang ?l) @c (group ?l (count @c @n))",
            "@n.words",
            "captures a value",
        ),
        (
            "(code :lang ?l) @c (group ?l (count @c @n))",
            "-?n",
            "aggregates are captures",
        ),
    ];
    for (query, sort, want) in cases {
        match Query::read(query, &schema(), Some(sort)) {
            Ok(_) => assert!(
                want.is_empty(),
                "{query} --sort {sort}: read, want {want:?}"
            ),
            Err(e) => assert!(
                !want.is_empty() && mentions(&e).contains(want),
                "{query} --sort {sort}: {e:?}, want {want:?}"
            ),
        }
    }
}

#[test]
fn sort_orders_rows() {
    let c = grouped();
    let query = Query::read(
        "(code :lang ?l) @c (group ?l (count @c @n))",
        &c.schema,
        Some("-@n"),
    )
    .unwrap();
    let rows = query.run(&c.tree, &c.env).unwrap();
    let langs: Vec<&str> = rows.iter().map(|r| r.vars["l"].as_str()).collect();
    assert_eq!(langs, ["go", "bash"]);
    let query = Query::read("(paragraph) @p", &c.schema, Some("-@p.words")).unwrap();
    let rows = query.run(&c.tree, &c.env).unwrap();
    let first = c.node(&hits(&rows[0], "p")[0]).unwrap();
    assert_eq!(first.node().text, "One two three.");
}

#[test]
fn optional_binding_joins_required_binder() {
    assert_eq!(
        self_linked().sorted_cells("(skill :id ?s (link :target ?s)? @l) @t"),
        ["l= t=b", "l= t=c", "l=a t=a"]
    );
}

#[test]
fn optional_binding_without_required_binder_fails() {
    for query in [
        "(skill (code :lang ?l)? @c (paragraph :text ?l)? @p) @s",
        "(skill (link :target ?s)? @l (not (link :anchor ?s))) @t",
        "(skill :id ?s (file (link :target ?s)? @l)) @t",
        "(skill (link :target ?s)? @l (inbound (skill :id ?s) @f)) @t",
        "(skill (link :target ?s)? @l) @t (skill :id ?s) @u",
    ] {
        let e = read_error(query);
        assert!(
            e.message().contains("is bound in an optional pattern"),
            "{query}: {e:?}"
        );
    }
}

#[test]
fn optional_binding_after_required_sibling_parses() {
    Query::read(
        "(skill (link :target ?s) @m (link :target ?s)? @l) @t",
        &schema(),
        None,
    )
    .unwrap();
}

const MISSING_AXIS: &str = r#"(row :table "axes" :value ?axis)
  (file :path ?p (section :title "Conventions") @s)
  (not (file :path ?p (section :title "Conventions" (table (row (cell :column "Axis" :text ?axis))))))"#;

#[test]
fn negation_connects_patterns() {
    let c = Corpus::new(
        &[
            (
                "a",
                &[(
                    "refs/more.md",
                    "# More\n\n## Conventions\n\n| Axis | Rule |\n| --- | --- |\n| Errors | Wrap them. |\n",
                )],
            ),
            (
                "b",
                &[(
                    "SKILL.md",
                    "# B\n\n## Conventions\n\n| Axis | Rule |\n| --- | --- |\n| Errors | Wrap them. |\n| Effects | Name them. |\n",
                )],
            ),
        ],
        &[],
    );
    let mut env = c.env.clone();
    env.data = axes();
    let rows = c.run_with(MISSING_AXIS, &env);
    let got: Vec<String> = rows
        .iter()
        .map(|row| {
            let section = c.node(&hits(row, "s")[0]).unwrap().node();
            let line = section.position.as_ref().unwrap().line();
            format!("{} {} L{line}", row.vars["p"], row.vars["axis"])
        })
        .collect();
    assert_eq!(got, ["refs/more.md Effects L3"]);
}

#[test]
fn negation_joins_both_members() {
    let c = fixture();
    let mut got: Vec<String> = c
        .run("(skill :id ?a) @x (skill :id ?b) @y (not (skill :id ?a (link :target ?b)))")
        .iter()
        .map(|row| format!("{}>{}", row.vars["a"], row.vars["b"]))
        .collect();
    got.sort();
    assert_eq!(got, ["a>a", "b>b", "c>a", "c>b", "c>c"]);
}

#[test]
fn negation_that_connects_nothing_fails() {
    for (query, offset) in [
        ("(skill :id ?a) @x (code) @c (not (skill :id ?a))", 18),
        (
            "(skill :id ?a) @x (code :lang ?l) @c (not (skill :id ?a (not (code :lang ?l))))",
            18,
        ),
        (
            "(skill :id ?a) @x (code :lang ?l) @c (paragraph) @p (not (skill :id ?a (code :lang ?l)))",
            37,
        ),
    ] {
        let e = read_error(query);
        assert!(
            e.message().contains("shares no variable with the first") && e.offset() == offset,
            "{query}: {e:?}, want offset {offset}"
        );
    }
}

/// Lines of the directives a query captures, with `level` typed as a number
/// on directives named in `declared`.
fn directive_lines(text: &str, declared: &[&str], query: &str) -> Vec<u32> {
    let mut c = Corpus::new(&[("a", &[("SKILL.md", text)])], &[]);
    let directives: Vec<(NodeId, f64)> = c
        .tree
        .root()
        .descendants()
        .filter(|n| {
            let node = n.node();
            node.kind == "directive"
                && node
                    .attrs
                    .get("name")
                    .is_some_and(|name| declared.contains(&name.as_str()))
        })
        .filter_map(|n| Some((n.id(), n.node().attrs.get("level")?.parse().ok()?)))
        .collect();
    for (id, level) in directives {
        c.env.typed.insert(
            id,
            BTreeMap::from([("level".to_owned(), Attribute::Amount(level))]),
        );
    }
    c.run(query)
        .iter()
        .flat_map(|row| hits(row, "d").to_vec())
        .map(|hit| {
            c.node(&hit)
                .unwrap()
                .node()
                .position
                .as_ref()
                .unwrap()
                .line()
        })
        .collect()
}

#[test]
fn declared_number_level_compares_by_value() {
    let text = ":::routes{level=3.0}\none\n:::\n\n:::routes{level=3}\ntwo\n:::\n\n:::routes{level=2.5}\nthree\n:::\n\n:::other{level=3.0}\nfour\n:::\n";
    let text = format!("# A\n\n{text}");
    for (query, want) in [
        ("(directive :level 3) @d", vec![3, 7]),
        ("(directive :level 3.0) @d", vec![3, 7]),
        ("(directive :level 2.5) @d", vec![11]),
        ("(directive :level 2..3) @d", vec![3, 7, 11]),
        ("(directive :level 3..) @d", vec![3, 7]),
        ("(directive :level (>= 3)) @d", vec![3, 7, 15]),
    ] {
        assert_eq!(directive_lines(&text, &["routes"], query), want, "{query}");
    }
}

#[test]
fn undeclared_level_keeps_string_semantics() {
    let text = "# A\n\n:::other{level=3.0}\none\n:::\n\n:::other{level=3}\ntwo\n:::\n";
    assert_eq!(directive_lines(text, &[], "(directive :level 3) @d"), [7]);
    assert_eq!(directive_lines(text, &[], "(directive :level 3.0) @d"), [7]);
}

#[test]
fn heading_level_accepts_integral_decimal() {
    let c = Corpus::new(&[("a", &[("SKILL.md", "# A\n\n## B\n")])], &[]);
    assert_eq!(c.texts_of("(heading :level 2.0) @h"), ["h=B"]);
}

#[test]
fn typed_flags_and_members_compare_by_type() {
    let mut c = Corpus::new(&[("a", &[("SKILL.md", "# A\n")])], &[]);
    let skill = c.env.targets["a"];
    c.tree.node_mut(skill).unwrap().attrs.extend([
        ("name".to_owned(), "yes".to_owned()),
        ("package".to_owned(), "x y".to_owned()),
    ]);
    c.env.typed.insert(
        skill,
        BTreeMap::from([
            ("name".to_owned(), Attribute::Flag(true)),
            (
                "package".to_owned(),
                Attribute::Members(vec!["x".to_owned(), "y".to_owned()]),
            ),
        ]),
    );
    assert_eq!(c.run(r#"(skill :name "T") @s"#).len(), 1);
    assert_eq!(c.run(r#"(skill :package "y") @s"#).len(), 1);
    assert_eq!(c.run("(skill :package /^y$/) @s").len(), 1);
    assert!(c.run(r#"(skill :package "x y") @s"#).is_empty());
}

#[test]
fn qualified_unit_ids_match() {
    let mut c = Corpus::new(&[("a", &[("SKILL.md", "# A\n")])], &[]);
    let skill = c.env.targets["a"];
    c.tree
        .node_mut(skill)
        .unwrap()
        .attrs
        .insert("package".to_owned(), "pkg".to_owned());
    assert_eq!(c.run(r#"(skill :id "pkg:a") @s"#).len(), 1);
    assert_eq!(c.run(r#"(skill :id "a") @s"#).len(), 1);
    assert!(c.run(r#"(skill :id "other:a") @s"#).is_empty());
}

#[test]
fn files_and_units_measure_their_bodies() {
    let c = Corpus::new(
        &[(
            "a",
            &[
                (
                    "SKILL.md",
                    "---\nname: a\n---\n\n# A\n\nHello wörld, two\nlines.\n",
                ),
                ("refs/more.md", "More words here.\n"),
            ],
        )],
        &[],
    );
    let sizes = |query: &str| -> Vec<String> {
        c.run(query)
            .iter()
            .map(|row| {
                ["l", "w", "c"]
                    .map(|name| row.value(name).unwrap().to_string())
                    .join(" ")
            })
            .collect()
    };
    let measures = "(sum :lines @n @l) (sum :words @n @w) (sum :chars @n @c)";
    assert_eq!(
        sizes(&format!("(file) @n (group @n {measures})")),
        ["4 5 29", "1 3 17"]
    );
    assert_eq!(
        sizes(&format!("(skill) @n (group @n {measures})")),
        ["5 8 46"]
    );
    assert!(c.run(r#"(file :contains "hello") @f"#).is_empty());
    assert!(c.run("(file :matches /Hello/) @f").is_empty());
}

#[test]
fn group_reader() {
    for query in [
        "(code :lang ?l) @c (group ?l)",
        "(code :lang ?l) @c (group ?l (count @c @n))",
        "(skill (code) @c) @s (group @s (count @c (< $max)))",
        "(code :lang ?l) @c (group (count @c @n (>= 3)) (count ?l @k))",
        "(skill (paragraph) @p) @s (group @s (sum :words @p @w) (min :lines @p @m) (max :level @p @x))",
    ] {
        Query::read(query, &schema(), None).unwrap_or_else(|e| panic!("{query}: {e}"));
    }
    for (query, want) in [
        ("(code :lang ?l) @c (group ?n (count @c @n))", "?n"),
        (
            "(code :lang ?l) @c (group ?l (count @c @n) (count ?l @n))",
            "@n",
        ),
        ("(code :lang ?l :text ?n) @c (group ?l (count @c @n))", "?n"),
        ("(code :lang ?l) @c (group (sum ?l @n))", "sum"),
        (
            "(skill (paragraph) @p) @s (group @s (sum :bogus @p @n))",
            "bogus",
        ),
        (
            "(skill (paragraph) @p) @s (group @s (sum :level @p @n))",
            "level",
        ),
        (
            "(skill (paragraph) @p) @s (group @s (count @p @n (older 30)))",
            "older",
        ),
        ("(skill (paragraph)* @p) @s (group @p (count @s @n))", "@p"),
    ] {
        let e = read_error(query);
        assert!(mentions(&e).contains(want), "{query}: {e:?}, want {want:?}");
    }
}

#[test]
fn group_evaluation() {
    let c = grouped();
    let mut env = c.env.clone();
    env.params = BTreeMap::from([("max".to_owned(), Value::Number(2.0))]);
    let cases: &[(&str, &[&str])] = &[
        (
            "(code :lang ?l) @c (group ?l)",
            &["?l=bash c=y+z", "?l=go c=x+w"],
        ),
        (
            "(code :lang ?l) (group ?l)",
            &["=x+w ?l=go", "=y+z ?l=bash"],
        ),
        (
            "(code :lang ?l) (group ?l (count ?l @n))",
            &["?l=bash @n=1", "?l=go @n=1"],
        ),
        (
            "(code :lang ?l) @c (group (count @c @n) (count ?l @k))",
            &["@k=2 @n=4 c=x+y+z+w"],
        ),
        (
            "(skill (code :lang ?l)) @s (group ?l (count @s @n))",
            &["?l=bash @n=1 s=a", "?l=go @n=2 s=a+b"],
        ),
        (
            r#"(code :lang "rust") @c (group (count @c @n) (sum :words @c @w))"#,
            &["@n=0 c="],
        ),
        (
            r#"(code :lang ?l :text "nothing") @c (group ?l (count @c @n))"#,
            &[],
        ),
        (
            r#"(skill (code :lang "bash")? @c) @s (group @s (count @c @n))"#,
            &["@n=0 c= s=b", "@n=0 c= s=c", "@n=2 c=y+z s=a"],
        ),
        (
            "(skill (paragraph :words 3..)? @p) @s (group @p (count @s @n))",
            &["@n=1 p=One two three. s=a", "@n=2 p= s=b+c"],
        ),
        (
            "(skill (paragraph) @p) @s (group @s (sum :words @p @w) (min :words @p @m) (max :words @p @x))",
            &[
                "@m=1 @w=1 @x=1 p=Six. s=c",
                "@m=2 @w=2 @x=2 p=Four five. s=b",
                "@m=2 @w=5 @x=3 p=One two three.+Seven eight. s=a",
            ],
        ),
        (
            "(skill (paragraph :words 3..)? @p) @s (group @s (sum :words @p @w))",
            &["@w=3 p=One two three. s=a", "p= s=b", "p= s=c"],
        ),
        ("(code :lang ?l) @c (group (min ?l @n))", &["c=x+y+z+w"]),
        (
            "(skill (code) @c) @s (group @s (count @c @n (>= 2)))",
            &["@n=3 c=x+y+z s=a"],
        ),
        (
            "(skill (code) @c) @s (group @s (count @c (< $max)))",
            &["c=w s=b"],
        ),
        (
            "(skill (paragraph :words 3..)? @p) @s (group @s (sum :words @p @w (>= 0)))",
            &["@w=3 p=One two three. s=a"],
        ),
    ];
    for (query, want) in cases {
        assert_eq!(c.grouped(query, &env), *want, "{query}");
    }
}

#[test]
fn group_skips_non_finite_operands() {
    let c = Corpus::new(&[("a", &[("SKILL.md", "# A\n\n```NaN\nq\n```\n")])], &[]);
    assert_eq!(
        c.grouped("(code :lang ?l) @c (group (min ?l @n))", &c.env),
        ["c=q"]
    );
}

#[test]
fn group_captures_publish_collected() {
    let c = grouped();
    let query = c.read("(code :lang ?l @l) @c (group ?l (count @c @n))");
    let rows = query.run(&c.tree, &c.env).unwrap();
    let go = rows.iter().find(|r| r.vars["l"] == "go").unwrap();
    assert!(matches!(&go.captures["c"], Capture::Collected(hits) if hits.len() == 2));
    assert_eq!(go.captures["n"], Capture::Value(Value::Number(2.0)));
    assert_eq!(
        go.captures["l"],
        Capture::Values(vec![Value::Text("go".to_owned())])
    );
    assert_eq!(query.shown_vars(), ["l"]);
}

#[test]
fn top_level_optional_left_joins() {
    let query = "(skill :id ?s) @t (skill :id (not ?s) (link :target ?s) @l)?";
    let cases: [(Corpus, &str, &[&str]); 5] = [
        (chained(), query, &["l= t=a", "l=b t=b", "l=c#usage t=c"]),
        (
            fixture(),
            query,
            &[
                "l=a t=a",
                "l=b t=b",
                "l=c t=c",
                "l=c#nope t=c",
                "l=c#usage t=c",
            ],
        ),
        (
            self_linked(),
            query,
            &["l= t=b", "l=a t=a", "l=c#usage t=c"],
        ),
        (
            chained(),
            "(skill :id (not ?s) (link :target ?s) @l)? (skill :id ?s) @t",
            &["l= t=a", "l=b t=b", "l=c#usage t=c"],
        ),
        (
            chained(),
            r#"(skill :id ?s) @t (skill :id (not ?s) (link :target ?s) @l)? (not (skill :id ?s (section :id "usage")))"#,
            &["l= t=a", "l=b t=b"],
        ),
    ];
    for (c, query, want) in cases {
        assert_eq!(c.sorted_cells(query), want, "{query}");
    }
}

#[test]
fn top_level_optional_reader_rules() {
    for (query, want) in [
        (
            "(skill :id ?s) @t (skill :id (not ?s) (link :target ?s) @l)*",
            "quantifiers apply to patterns nested",
        ),
        (
            "(skill :id ?s) @t (skill :id (not ?s) (link :target ?s) @l)+",
            "quantifiers apply to patterns nested",
        ),
        (
            "(skill :id ?t) @a (skill :id ?t (link :target ?s) @l)? (skill :id ?s)? @b",
            "is bound in an optional pattern",
        ),
        (
            "(skill :id ?s) @u (skill (link :target ?s)? @l) @t",
            "is bound in an optional pattern",
        ),
        (
            "(skill :id ?s (link :target ?s) @l) (skill (heading) @h)?",
            "shares no variable with the first",
        ),
    ] {
        let e = read_error(query);
        assert!(e.message().contains(want), "{query}: {e:?}, want {want:?}");
    }
    for query in [
        "(skill)? @s",
        "(skill :id ?t) @a (skill :id ?s (link :target ?t) @l)? (not (skill :id ?s (heading)))",
    ] {
        let e = read_error(query);
        assert!(!e.message().contains("quantifiers apply"), "{query}: {e:?}");
    }
    for query in [
        "(skill :id ?s) @t (skill :id (not ?s) (link :target ?s) @l)?",
        r#"(skill :id ?t) @a (skill (link :target ?t) @l)? (not (skill :id ?t (heading :text "X")))"#,
    ] {
        Query::read(query, &schema(), None).unwrap_or_else(|e| panic!("{query}: {e}"));
    }
}

#[test]
fn top_level_optional_bounded_by_max_bindings() {
    let text = format!("# A\n{}", "\npara.\n".repeat(400));
    let c = Corpus::new(&[("a", &[("SKILL.md", text.as_str())])], &[]);
    let query = c.read("(skill :id ?s (paragraph) @p) (skill :id ?s (paragraph) @q)?");
    assert!(matches!(query.run(&c.tree, &c.env), Err(RunError::TooMany)));
}

#[test]
fn impossible_ignores_optional_members() {
    let mut env = Environment::new(UNIX_EPOCH);
    env.params = BTreeMap::from([("th".to_owned(), Value::Number(0.0))]);
    for (query, want) in [
        (
            "(skill :id ?s (paragraph :text ?t) @a) (skill :id ?s (paragraph :text (near ?t $th)) @b)?",
            false,
        ),
        (
            "(skill :id ?s (paragraph :text ?t) @a) (skill :id ?s (paragraph :text (near ?t $th)) @b)",
            true,
        ),
    ] {
        let query = Query::read(query, &schema(), None).unwrap();
        assert_eq!(query.impossible(&env), want, "{}", query.source());
    }
}

#[test]
fn score_is_not_a_variable() {
    let e = read_error(
        "(paragraph :text ?t) @a (paragraph :text (near ?t 0.6 @score)) @b (paragraph :text ?score)? @c",
    );
    assert!(e.message().contains("?score appears only once"), "{e:?}");
}

#[test]
fn read_module() {
    let shared = "(define (titled ?t) (section > (heading :text ?t)))";
    let source = r#"
(import std)
(define (usage-code ?lang) (section :id "usage" (code :lang ?lang)))
(rule bash-usage
  :severity error
  :message "{@c} is {?l}; use {$want}"
  (skill :id "a" (usage-code ?l) (code :lang ?l :text /top/) @c))
(rule usage :message "{@s}" (titled "Usage") @s)"#;
    let import = |name: &str| {
        if name != "std" {
            return Err(format!("no module {name}"));
        }
        Module::read(shared, &schema(), |_: &str| Err("no imports"))
            .map(|m| m.defines)
            .map_err(|e| e.to_string())
    };
    let module = Module::read(source, &schema(), import).unwrap();
    assert_eq!(module.rules.len(), 2);
    assert_eq!(module.imports, ["std"]);
    let rule = &module.rules[0];
    assert_eq!(rule.id.as_str(), "bash-usage");
    assert_eq!(rule.severity, crate::lint::Severity::Error);
    assert_eq!(
        rule.params(),
        BTreeMap::from([("want".to_owned(), ParamKind::Text)])
    );
    let c = fixture();
    let mut env = c.env.clone();
    env.params = BTreeMap::from([("want".to_owned(), Value::Text("go".to_owned()))]);
    let rows = rule.query.run(&c.tree, &env).unwrap();
    assert_eq!(rows.len(), 1);
    let code = c.node(&hits(&rows[0], "c")[0]).unwrap();
    assert_eq!(code.node().text, "echo top\n");
    assert_eq!(module.rules[1].query.run(&c.tree, &c.env).unwrap().len(), 2);
}

#[test]
fn read_module_errors() {
    for (source, want) in [
        (r#"(rule Bad :message "m" (heading))"#, "lowercase letters"),
        ("(rule r (heading))", "needs a :message"),
        (r#"(rule r :message "{@x}" (heading) @h)"#, "uses @x"),
        (
            r#"(rule r :message "m" :at @x (heading) @h)"#,
            "points at @x",
        ),
        (
            r#"(define (loop ?a) (loop ?a)) (rule r :message "m" (loop "x") @h)"#,
            "is a recursive define of values, so it takes no quantifier or capture",
        ),
        (
            r#"(define (one ?a) (heading :text ?a)) (rule r :message "m" (one) @h)"#,
            "takes 1 arguments, got 0",
        ),
        ("(define (heading) (code)) ", "cannot name a pattern"),
        ("(define (paragraph) (code)) ", "cannot name a pattern"),
        ("(query (heading))", "unknown module form"),
        ("(import other)", "no module other"),
    ] {
        let e = module_error(source);
        assert!(
            e.to_string().contains(want),
            "{source}: {e:?}, want {want:?}"
        );
    }
}

#[test]
fn rule_rows_differ_by_printed_variables() {
    let source =
        format!(r#"(rule missing-axis :message "no {{?axis}} row" :at @s {MISSING_AXIS})"#);
    let module = Module::read(&source, &schema(), |_: &str| Err("no imports")).unwrap();
    let c = Corpus::new(
        &[(
            "a",
            &[(
                "SKILL.md",
                "# A\n\n## Conventions\n\n| Axis | Rule |\n| --- | --- |\n| Naming | Say it. |\n",
            )],
        )],
        &[],
    );
    let mut env = c.env.clone();
    env.data = axes();
    let rows = module.rules[0].query.run(&c.tree, &env).unwrap();
    let mut axes: Vec<&str> = rows.iter().map(|r| r.vars["axis"].as_str()).collect();
    axes.sort_unstable();
    assert_eq!(axes, ["Effects", "Errors"]);
}

#[test]
fn optional_relation_target_falls_back_per_row() {
    let c = chained();
    let cases: &[(&str, &[&str])] = &[
        (
            "(skill (inbound (skill)? @from)) @s",
            &["from= s=a", "from=a s=b", "from=a s=c", "from=b s=c"],
        ),
        (
            "(skill (from (link)? @l)) @s",
            &["l= s=a", "l= s=c", "l=b s=b"],
        ),
        (
            "(skill (reaches (skill)? @t)) @s",
            &["t= s=c", "t=b s=a", "t=c s=a", "t=c s=b"],
        ),
        (
            "(link (to (section)? @t)) @l",
            &["t= l=b", "t=usage l=c#usage"],
        ),
        (
            "(skill (from (link)* @l)) @s",
            &["l= s=a", "l= s=c", "l=b s=b"],
        ),
    ];
    for (query, want) in cases {
        assert_eq!(c.idents(query), *want, "{query}");
    }
}

#[test]
fn optional_sibling_falls_back_per_row() {
    assert_eq!(
        self_linked().idents("(skill :id ?x (link :target (not ?x))? @l) @s"),
        ["l= s=a", "l= s=c", "l=a s=b", "l=c#usage s=b"]
    );
}

#[test]
fn top_level_repeat_hint() {
    for query in ["(code)+ @c", "(code)* @c"] {
        assert!(
            read_error(query).hint().is_some_and(|h| !h.is_empty()),
            "{query}"
        );
    }
}

#[test]
fn submatches_bind_variables() {
    let tree = Markdown::new().parse("a.md", "Use `$review`, `@scout`, `plain` and `$git`.\n");
    let source = r#"
(define (sigiled @r ?sigil ?name) (code :ticks 1 :matches /^(?<sigil>[$@])(?<name>[a-z]+)$/) @r)
(rule all :message "{?s}{?n}" (sigiled @r ?s ?n))
(rule skills :message "{?n}" (sigiled @r "$" ?n))
(rule pair :message "{?n}" (code :matches /^\$(?<n>[a-z]+)$/) @a (paragraph :text (contains ?n)) @p)"#;
    let module = Module::read(source, &schema(), |_: &str| Err("no imports")).unwrap();
    let env = Environment::new(UNIX_EPOCH);
    let want = ["$review @scout $git", "review git", "review git"];
    for (rule, want) in module.rules.iter().zip(want) {
        let rows = rule.query.run(&tree, &env).unwrap();
        let got: Vec<String> = rows
            .iter()
            .map(|row| {
                let var = |name: &str| row.vars.get(name).cloned().unwrap_or_default();
                format!("{}{}", var("s"), var("n"))
            })
            .collect();
        assert_eq!(got.join(" "), want, "rule {}", rule.id);
    }
}

#[test]
fn submatch_errors() {
    for (source, want) in [
        (
            r#"(rule r :message "m" (code :matches /(?<x>a)/) @c)"#,
            "?x appears only once",
        ),
        (
            r#"(rule r :message "m" (code :text (not /(?<x>a)/) :lang ?x) @c)"#,
            "cannot bind ?x",
        ),
        (
            r#"(define (d @c ?x) (code :matches /(?<x>a)/) @c) (rule r :message "m" (d @c (heading)))"#,
            "takes a ?variable or a value",
        ),
    ] {
        let e = module_error(source);
        assert!(
            e.to_string().contains(want),
            "{source}: {e:?}, want {want:?}"
        );
    }
}

#[test]
fn table_rows_and_cells() {
    let c = tabled();
    for (query, want) in [
        (
            "(table (row :header true) @r)",
            "r=| Axis | Rule |\nr=| Invariant | Check |",
        ),
        (
            r#"(section :title "Invariants" (table (row (cell :column "Check" :text "")) @r))"#,
            "r=| Lines exact |  |",
        ),
        (
            "(row :index 1 (cell :index 0) @c)",
            "c=Errors\nc=Rows bound",
        ),
        (
            r#"(row :header false (cell :column "Axis") @c)"#,
            "c=Errors",
        ),
        (
            r#"(cell :column "Check" (code :inline true) @k)"#,
            "k=go test",
        ),
        (r#"(row :contains "wrap") @r"#, "r=| Errors | Wrap them. |"),
        (
            "(table :matches /Rows bound/) @t",
            "t=| Invariant | Check |\n| --- | --- |\n| Rows bound | `go test` |\n| Lines exact |  |",
        ),
    ] {
        assert_eq!(c.texts_of(query).join("\n"), want, "{query}");
    }
}

#[test]
fn table_cell_lines() {
    let c = tabled();
    let rows = c.run(r#"(row (cell :column "Check" :text "") @c) @r"#);
    assert_eq!(rows.len(), 1);
    let at = |name: &str| {
        let node = c.node(&hits(&rows[0], name)[0]).unwrap().node();
        let position = node.position.as_ref().unwrap();
        (position.line(), position.column())
    };
    assert_eq!((at("r"), at("c")), ((14, 1), (14, 18)));
}

#[test]
fn data_rows_and_table_rows_stay_apart() {
    let c = tabled();
    let mut env = c.env.clone();
    env.data = axes();
    for (query, want) in [
        (
            r#"(row :table "axes" :value ?v) @d (cell :text ?v) @c"#,
            "d= c=Errors",
        ),
        (
            "(row :header false) @r",
            "r=| Errors | Wrap them. |\nr=| Rows bound | `go test` |\nr=| Lines exact |  |",
        ),
    ] {
        let rows = c.run_with(query, &env);
        assert_eq!(c.texts(query, &rows).join("\n"), want, "{query}");
    }
}

#[test]
fn table_keys() {
    for (query, message, hint) in [
        (
            r#"(cell :colum "Axis")"#,
            "cell has no key :colum",
            "did you mean column?",
        ),
        (
            "(row :headr true)",
            "row has no key :headr",
            "did you mean header?",
        ),
    ] {
        let e = read_error(query);
        assert!(
            e.message().contains(message) && e.hint().unwrap_or_default().contains(hint),
            "{query}: {e:?}"
        );
    }
}

#[test]
fn core_schema_rejects_the_unit_a_caller_registers() {
    let e = Query::read("(skill :id ?s) @k", &Schema::core(), None).unwrap_err();
    assert_eq!(e.kind(), ReadErrorKind::UnknownType);
    let c = chained();
    let rows = c.run(r#"(skill :id "a" (reaches (skill :id ?s @s) @k))"#);
    let reached: Vec<&str> = rows.iter().map(|r| r.vars["s"].as_str()).collect();
    assert_eq!(reached, ["b", "c"]);
}

#[test]
fn a_caller_file_key_stays_an_attribute() {
    let mut tree = Markdown::new().parse("docs/a.md", "# A\n");
    let root = tree.root().id();
    tree.node_mut(root)
        .unwrap()
        .attrs
        .insert("file".to_owned(), "physical/a.md".to_owned());
    let query = Query::read(
        r#"(file :file "physical/a.md" :position ?p @p) @f"#,
        &schema(),
        None,
    )
    .unwrap();
    let rows = query.run(&tree, &Environment::new(UNIX_EPOCH)).unwrap();
    assert_eq!(rows[0].captures["f"], Capture::Nodes(vec![Hit::Node(root)]));
    assert_eq!(
        rows[0].value("p"),
        Some(&Value::Text("docs/a.md\u{0}000000000".to_owned()))
    );
    let position = Query::read("(heading :position ?p @p) @h", &schema(), None).unwrap();
    let rows = position.run(&tree, &Environment::new(UNIX_EPOCH)).unwrap();
    assert_eq!(
        rows[0].value("p"),
        Some(&Value::Text("docs/a.md\u{0}000000000".to_owned()))
    );
}

#[test]
fn positions_and_lines_read_origins() {
    let text = "---\ntitle: Parity guide\ntags: [query, tree]\nversion: 3\n---\n\n# Guide\n\nIntro.\n\n## Install\n\nA second install section with a longer paragraph that spans\ntwo source lines.\n";
    let tree = Markdown::new().parse("docs/guide.md", text);
    let query = Query::read(
        "(section :position ?p @pos :lines ?l @lines) @s",
        &schema(),
        Some("-@pos"),
    )
    .unwrap();
    let rows = query.run(&tree, &Environment::new(UNIX_EPOCH)).unwrap();
    let got: Vec<(String, String)> = rows
        .iter()
        .map(|r| (r.vars["p"].clone(), r.vars["l"].clone()))
        .collect();
    assert_eq!(
        got,
        [
            ("docs/guide.md\u{0}000000017".to_owned(), "4".to_owned()),
            ("docs/guide.md\u{0}000000000".to_owned(), "8".to_owned()),
        ]
    );
}

#[test]
fn runs_share_one_tree_across_threads() {
    let c = fixture();
    let query =
        c.read("(skill :id ?x (heading :text ?t) @h1) (skill :id (not ?x) (heading :text ?t) @h2)");
    let before = c.tree.clone();
    let rows = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..2)
            .map(|_| scope.spawn(|| query.run(&c.tree, &c.env).unwrap()))
            .collect();
        runs.into_iter()
            .map(|run| run.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(rows[0], rows[1]);
    assert_eq!(rows[0].len(), 2);
    assert_eq!(c.tree, before);
}

#[test]
fn regexps_follow_the_regex_crate() {
    let tree = Markdown::new().parse("a.md", "Digits ٣ here.\n\nWord café.\n\nPlain x.\n");
    let env = Environment::new(UNIX_EPOCH);
    let count = |query: &str| {
        Query::read(query, &Schema::core(), None)
            .unwrap_or_else(|e| panic!("{query}: {e}"))
            .run(&tree, &env)
            .unwrap()
            .len()
    };
    assert_eq!(count(r"(paragraph :matches /\d/)"), 1);
    assert_eq!(count(r"(paragraph :matches /(?-u:\d)/)"), 0);
    assert_eq!(count(r"(paragraph :matches /caf\w\b/)"), 1);
    assert_eq!(count(r"(paragraph :matches /[a-z&&[^aeiou]]\./)"), 1);
    assert_eq!(count(r"(paragraph :matches /\<Plain\>/)"), 1);
    for unsupported in [
        r"(paragraph :matches /\Qa.b\E/)",
        r"(paragraph :matches /\C/)",
    ] {
        let e = Query::read(unsupported, &Schema::core(), None).unwrap_err();
        assert!(
            e.message().starts_with("invalid regular expression"),
            "{unsupported}"
        );
    }
}

#[test]
fn placeholders_name_sigils_and_keys() {
    let found = placeholders("{@swap.key} is {?old}, not {$x}; {@a.b.c} {bad} {@}");
    let got: Vec<(Sigil, &str, Option<&str>)> =
        found.iter().map(|p| (p.sigil, p.name, p.key)).collect();
    assert_eq!(
        got,
        [
            (Sigil::Capture, "swap", Some("key")),
            (Sigil::Var, "old", None),
            (Sigil::Param, "x", None),
            (Sigil::Capture, "a", Some("b.c")),
        ]
    );
    assert_eq!(found[0].span, 0..11);
}

fn skill_spec() -> SchemaSpec {
    SchemaSpec {
        types: vec![TypeSpec {
            name: "skill".to_owned(),
            keys: vec!["id".to_owned(), "package".to_owned()],
        }],
        keys: BTreeMap::from([("file".to_owned(), BTreeSet::from(["file".to_owned()]))]),
        unit: Some(Unit {
            kind: "skill".to_owned(),
            id_key: "id".to_owned(),
            qualifier_key: Some("package".to_owned()),
        }),
        ..SchemaSpec::default()
    }
}

#[test]
fn relations_need_a_unit() {
    for query in [
        "(link (to (section) @t)) @l",
        "(file (reaches (file)) @f)",
        "(file (not (inbound (file)))) @f",
    ] {
        let Err(e) = Query::read(query, &Schema::core(), None) else {
            panic!("{query}: read, want an error");
        };
        assert_eq!(e.kind(), ReadErrorKind::NoUnit, "{query}");
        assert!(e.message().contains("the schema has none"), "{e}");
    }
}

#[test]
fn core_schema_knows_no_unit_and_no_skill() {
    let core = Schema::core();
    assert!(core.unit().is_none());
    assert!(!core.has_type("skill"));
    assert!(core.has_key("heading", "level"));
    assert!(core.has_key("paragraph", "dynamic"));
    assert!(!core.has_key("link", "exists"));
    assert!(core.has_key("directive", "anything"));
}

#[test]
fn a_caller_registers_the_unit_and_its_keys() {
    let schema = Schema::new(skill_spec()).unwrap();
    assert!(schema.has_key("skill", "package"));
    assert!(schema.has_key("file", "file"));
    assert_eq!(schema.unit().map(|u| u.kind.as_str()), Some("skill"));
}

#[test]
fn registering_a_core_type_again_fails() {
    let spec = SchemaSpec {
        types: vec![TypeSpec {
            name: "heading".to_owned(),
            keys: vec![],
        }],
        ..SchemaSpec::default()
    };
    assert_eq!(
        Schema::new(spec),
        Err(SchemaError::DuplicateType("heading".to_owned()))
    );
}

#[test]
fn keys_and_unit_must_name_known_types() {
    let mut spec = skill_spec();
    spec.types.clear();
    spec.keys.clear();
    assert_eq!(
        Schema::new(spec),
        Err(SchemaError::UnknownType("skill".to_owned()))
    );
}

#[test]
fn values_print_like_henia() {
    assert_eq!(Value::Number(3.0).to_string(), "3");
    assert_eq!(Value::Number(0.25).to_string(), "0.25");
    assert_eq!(Value::Text("x".to_owned()).to_string(), "x");
}

fn files(docs: &[(&str, &str)]) -> Tree {
    let trees = docs.iter().map(|(file, text)| {
        let mut tree = Markdown::new().parse(file, text);
        let root = tree.root().id();
        if let Some(node) = tree.node_mut(root) {
            node.attrs.insert("path".to_owned(), (*file).to_owned());
        }
        tree
    });
    crate::lint::Corpus::new(trees, Environment::new(UNIX_EPOCH))
        .unwrap()
        .tree
}

fn texts(rows: &[Row], tree: &Tree, capture: &str) -> Vec<String> {
    rows.iter()
        .filter_map(|row| match row.captures.get(capture) {
            Some(Capture::Nodes(hits)) => match hits.first() {
                Some(Hit::Node(id)) => tree.get(*id).map(|n| n.node().text.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn comparisons_take_arithmetic_over_variables() {
    let tree = files(&[(
        "a.md",
        "# A\n\n## B\n\n#### Skipped\n\n### C\n\n##### Deep\n",
    )]);
    let query = Query::read(
        "(section :level ?a > (section :level (> (+ ?a 1))) @s)",
        &Schema::core(),
        None,
    )
    .unwrap();
    let rows = query.run(&tree, &Environment::new(UNIX_EPOCH)).unwrap();
    let titles: Vec<String> = texts(&rows, &tree, "s")
        .iter()
        .map(|text| text.lines().next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(titles.len(), 2, "{titles:?}");
    for (source, message) in [
        ("(heading :level (> (+ 1 2))) @h", "a constant"),
        ("(heading :level (* ?a)) @h", "at least 2 arguments"),
    ] {
        let error = Query::read(source, &Schema::core(), None).unwrap_err();
        assert!(error.message().contains(message), "{source}: {error:?}");
    }
}

#[test]
fn recursive_defines_derive_facts_to_a_fixpoint() {
    let tree = files(&[
        ("a.md", "# A\n\n[b](b.md)\n"),
        ("b.md", "# B\n\n[c](c.md)\n"),
        ("c.md", "# C\n\nEnd.\n"),
    ]);
    let defines = "(define (reach ?a ?b) (file :path ?a (link :url ?b))) \
                   (define (reach ?a ?b) (file :path ?a (link :url ?m)) (reach ?m ?b))";
    let read = |query: &str| Query::read(&format!("{defines} {query}"), &Schema::core(), None);
    let query = read(r#"(file :path ?p) @f (reach ?p "c.md")"#).unwrap();
    let rows = query.run(&tree, &Environment::new(UNIX_EPOCH)).unwrap();
    let reached: Vec<&str> = rows.iter().map(|row| row.vars["p"].as_str()).collect();
    assert_eq!(reached, ["a.md", "b.md"]);
    for (source, message) in [
        ("(section (reach ?a ?b)) @s", "called at the top level"),
        (
            "(file :path ?a) @f (reach ?a ?b) @r",
            "takes no quantifier or capture",
        ),
    ] {
        let error = read(source).unwrap_err();
        assert!(error.message().contains(message), "{source}: {error:?}");
    }
    let negated = "(define (odd ?a) (file :path ?a) (not (odd ?a))) (file :path ?p) @f (odd ?p)";
    let error = Query::read(negated, &Schema::core(), None).unwrap_err();
    assert!(error.message().contains("still growing"), "{error:?}");
}

#[test]
fn edits_change_the_file_text_past_the_frontmatter() {
    let text = "---\ntitle: T\n---\n\n# Guide\n\n## SETUP\n\nRun it.\n";
    let tree = files(&[("a.md", text)]);
    let query = Query::read(
        r#"(heading :level ?l :text ?t) @h (replace @h "{?l|times:#} {?t|capitalize}")"#,
        &Schema::core(),
        None,
    )
    .unwrap();
    let env = Environment::new(UNIX_EPOCH);
    let rows = query.run(&tree, &env).unwrap();
    let changes = query.changes(&rows, &tree, &env.params).unwrap();
    let edited = crate::edit::apply(text, &changes).unwrap();
    assert_eq!(
        edited,
        "---\ntitle: T\n---\n\n# Guide\n\n## Setup\n\nRun it.\n"
    );
    let error = Query::read(r#"(heading) @h (replace @x "y")"#, &Schema::core(), None).unwrap_err();
    assert!(error.message().contains("captures no node"), "{error:?}");
}

#[test]
fn division_follows_an_open_paren_and_hash_quotes_hold_slashes() {
    let tree = files(&[(
        "a.md",
        "# A\n\n[x](https://example.com/a) and [y](docs/b.md)\n\n## B\n\n#### D\n",
    )]);
    let env = Environment::new(UNIX_EPOCH);
    let count = |source: &str| {
        let query = Query::read(source, &Schema::core(), None).unwrap();
        query.run(&tree, &env).unwrap().len()
    };
    assert_eq!(
        count("(heading :level ?a) @h (heading :level (/ ?a 2)) @half"),
        2
    );
    assert_eq!(
        count("(heading :level ?a) @h (heading :level (> (/ ?a 0))) @g"),
        0
    );
    assert_eq!(count(r#"(link :url #"^https://example\.com/") @l"#), 1);
    assert_eq!(count(r#"(link :url #"^docs/") @l"#), 1);
    assert_eq!(count(r"(link :url /^https:\/\//) @l"), 1);
    let error = Query::read(r#"(link :url #"^docs/) @l"#, &Schema::core(), None).unwrap_err();
    assert!(error.message().contains("unterminated"), "{error:?}");
}

fn edited(text: &str, source: &str) -> String {
    let tree = files(&[("a.md", text)]);
    let query = Query::read(source, &Schema::core(), None).unwrap();
    let env = Environment::new(UNIX_EPOCH);
    let rows = query.run(&tree, &env).unwrap();
    let changes = query.changes(&rows, &tree, &env.params).unwrap();
    crate::edit::apply(text, &changes).unwrap()
}

#[test]
fn edits_on_items_and_quotes_include_their_markers() {
    let list = "- one\n- two\n";
    assert_eq!(
        edited(list, r#"(item :text "one") @i (insert-before @i "* ")"#),
        "* - one\n- two\n"
    );
    assert_eq!(
        edited(list, r#"(item :text "one") @i (delete @i)"#),
        "- two\n"
    );
    assert_eq!(
        edited("1. one\n2. two\n", r#"(item :text "one") @i (delete @i)"#),
        "2. two\n"
    );
    assert_eq!(
        edited("Intro.\n\n> Quoted.\n\nAfter.\n", "(quote) @q (delete @q)"),
        "Intro.\n\nAfter.\n"
    );
}

#[test]
fn edits_on_lists_include_their_first_marker() {
    let list = "- one\n- two\n";
    assert_eq!(
        edited(
            "Intro.\n\n- one\n- two\n\nAfter.\n",
            "(list) @l (delete @l)"
        ),
        "Intro.\n\nAfter.\n"
    );
    assert_eq!(edited(list, r#"(list) @l (replace @l "- new")"#), "- new\n");
    assert_eq!(
        edited(list, r#"(list) @l (insert-before @l "X")"#),
        "X- one\n- two\n"
    );
    assert_eq!(
        edited("1. one\n2. two\n", r#"(list) @l (replace @l "1. new")"#),
        "1. new\n"
    );
    assert_eq!(
        edited("> - one\n> - two\n", r#"(list) @l (replace @l "- new")"#),
        "> - new\n"
    );
    assert_eq!(
        edited("> - one\n> - two\n", r#"(list) @l (insert-before @l "X")"#),
        "> X- one\n> - two\n"
    );
}

#[test]
fn edits_on_headings_include_their_marker() {
    assert_eq!(
        edited("Intro.\n\n# Title\n\nBody.\n", "(heading) @h (delete @h)"),
        "Intro.\n\nBody.\n"
    );
    assert_eq!(
        edited(
            "# Title\n\nBody.\n",
            r###"(heading) @h (replace @h "## New")"###
        ),
        "## New\n\nBody.\n"
    );
    assert_eq!(
        edited(
            "# Title\n\nBody.\n",
            r#"(heading) @h (insert-before @h "X")"#
        ),
        "X# Title\n\nBody.\n"
    );
    assert_eq!(
        edited("- # G\n", r##"(heading) @h (replace @h "# X")"##),
        "- # X\n"
    );
    assert_eq!(
        edited("- # G\n", r#"(heading) @h (insert-before @h "X")"#),
        "- X# G\n"
    );
    assert_eq!(
        edited("> # H\n", r##"(heading) @h (replace @h "# X")"##),
        "> # X\n"
    );
    assert_eq!(
        edited("> # H\n", r#"(heading) @h (insert-before @h "X")"#),
        "> X# H\n"
    );
    assert_eq!(
        edited("> Title\n> =====\n", r##"(heading) @h (replace @h "# T")"##),
        "> # T\n"
    );
}

#[test]
fn edits_on_setext_headings_include_their_underline() {
    assert_eq!(
        edited(
            "Title\n=====\n\nBody.\n",
            r##"(heading) @h (replace @h "# Title")"##
        ),
        "# Title\n\nBody.\n"
    );
    assert_eq!(
        edited("Intro.\n\nSub\n---\n\nBody.\n", "(heading) @h (delete @h)"),
        "Intro.\n\nBody.\n"
    );
}

#[test]
fn arithmetic_compares_equal_as_numbers() {
    let tree = files(&[(
        "a.md",
        "---\none: \"1\"\none-again: \"1.0\"\nnan: \"NaN\"\nnan-again: \"NaN\"\nzero: \"0\"\nminus-zero: \"-0\"\n---\n\n# A\n",
    )]);
    let env = Environment::new(UNIX_EPOCH);
    let equal = |a: &str, b: &str| {
        let source =
            format!(r#"(entry :key "{a}" :value ?x) @a (entry :key "{b}" :value (+ ?x 0)) @b"#);
        let query = Query::read(&source, &Schema::core(), None).unwrap();
        !query.run(&tree, &env).unwrap().is_empty()
    };
    assert!(equal("one", "one-again"));
    assert!(!equal("nan", "nan-again"), "NaN equals nothing");
    assert!(equal("zero", "minus-zero"), "-0 equals 0");
}

#[test]
fn edit_templates_name_only_known_variables_and_params() {
    let tree = files(&[("a.md", "# Guide\n")]);
    let env = Environment::new(UNIX_EPOCH);
    let read = |source: &str| Query::read(source, &Schema::core(), None);
    assert!(read(r#"(heading :level ?l) @h (replace @h "{?l|times:#} X")"#).is_ok());
    let error = read(r#"(heading :level ?l) @h (replace @h "{?l|times:#} {?tilte}")"#).unwrap_err();
    assert!(error.message().contains("tilte"), "{error:?}");
    let param = read(r#"(heading :level ?l) @h (replace @h "{?l|times:#} {$tilte}")"#);
    assert!(
        param.is_err() || param.unwrap().run(&tree, &env).is_err(),
        "an unset $tilte must not blank the heading"
    );
}
