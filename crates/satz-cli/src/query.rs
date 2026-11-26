//! `satz query`: rows of a tq pattern over files, as a listing, whole node
//! texts, JSON or a count.

use std::collections::BTreeMap;
use std::io::Write;
use std::iter;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use satz::lint::Corpus;
use satz::query::{Capture, Environment, Hit, Query, Row, Schema, Value};
use satz::{NodeRef, Tree};
use serde_json::{Map, Number, Value as Json};

use crate::config::{self, Source};
use crate::failure::Failure;
use crate::{Status, input, json};

/// A snippet longer than this many characters is cut.
const SNIPPET: usize = 80;

#[derive(Debug, clap::Args)]
#[command(group = clap::ArgGroup::new("output").args(["json", "text", "count"]))]
pub(crate) struct Args {
    /// The tq pattern.
    pattern: String,
    /// A file or directory to read instead of the current directory
    /// (repeatable).
    #[arg(long = "path", value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Print rows as JSON objects keyed by capture.
    #[arg(long)]
    json: bool,
    /// Print whole node texts.
    #[arg(long)]
    text: bool,
    /// Print the number of rows.
    #[arg(long)]
    count: bool,
    /// Order rows by ?var, @value or @capture.key, descending with a leading -.
    #[arg(long, value_name = "KEY", allow_hyphen_values = true)]
    sort: Option<String>,
    /// Print at most N rows.
    #[arg(long, value_name = "N")]
    limit: Option<usize>,
    #[arg(
        long,
        value_name = "DIR",
        help = "The Model2Vec directory for (similar …) [default: [lint.semantic] model_path]"
    )]
    model: Option<PathBuf>,
}

pub(crate) fn run(args: &Args, config: &Source, out: &mut impl Write) -> Result<Status, Failure> {
    let query = Query::read(&args.pattern, &Schema::core(), args.sort.as_deref())
        .map_err(|error| Failure::explain(&args.pattern, &error))?;
    let trees = input::read(&args.paths, &input::frontends()?)?;
    let mut environment = Environment::new(SystemTime::now());
    if query.is_semantic() {
        let dir = match &args.model {
            Some(dir) => dir.clone(),
            None => config.load()?.model.ok_or_else(|| {
                Failure::new(
                    "(similar …) needs a semantic model; pass --model or set [lint.semantic] model_path",
                )
            })?,
        };
        environment.embed = Some(Arc::new(config::model(&dir)?));
    }
    let corpus = Corpus::new(trees, environment).map_err(|error| Failure::chain(&error))?;
    let mut rows = query
        .run(&corpus.tree, &corpus.environment)
        .map_err(|error| Failure::chain(&error))?;
    let total = rows.len();
    if args.count {
        writeln!(out, "{total}")?;
        return Ok(Status::Clean);
    }
    if let Some(limit) = args.limit {
        rows.truncate(limit);
    }
    let printer = Printer {
        query: &query,
        tree: &corpus.tree,
    };
    if args.json {
        json::write(out, &printer.json(&rows))?;
    } else if rows.is_empty() {
        eprintln!("satz: no matches");
    } else {
        printer.listing(out, &rows, args.text)?;
        if rows.len() < total {
            eprintln!("satz: {} of {total} rows shown", rows.len());
        }
    }
    Ok(Status::Clean)
}

struct Printer<'a> {
    query: &'a Query,
    tree: &'a Tree,
}

impl Printer<'_> {
    /// Rows as JSON: each capture by name, the
    /// unnamed one as `match`, and each variable as `?name`.
    fn json(&self, rows: &[Row]) -> Json {
        let records = rows
            .iter()
            .map(|row| {
                let mut record = Map::new();
                for name in self.query.captures() {
                    let key = if name.is_empty() { "match" } else { name };
                    let value = row
                        .captures
                        .get(name)
                        .map_or(Json::Null, |capture| self.capture_json(capture));
                    record.insert(key.to_owned(), value);
                }
                for (name, value) in &row.vars {
                    record.insert(format!("?{name}"), Json::String(value.clone()));
                }
                Json::Object(record)
            })
            .collect();
        Json::Array(records)
    }

    fn capture_json(&self, capture: &Capture) -> Json {
        match capture {
            Capture::Nodes(hits) | Capture::Collected(hits) => match hits.as_slice() {
                [] => Json::Null,
                [hit] => self.hit_json(hit),
                hits => Json::Array(hits.iter().map(|hit| self.hit_json(hit)).collect()),
            },
            Capture::Value(value) => value_json(value),
            Capture::Values(values) => Json::Array(values.iter().map(value_json).collect()),
            _ => Json::Null,
        }
    }

    fn hit_json(&self, hit: &Hit) -> Json {
        let Some(node) = self.describe(hit) else {
            return Json::Null;
        };
        let mut record = Map::new();
        record.insert("address".to_owned(), Json::String(node.address));
        record.insert("type".to_owned(), Json::String(node.kind.to_owned()));
        record.insert(
            "attrs".to_owned(),
            Json::Object(
                node.attrs
                    .iter()
                    .map(|(k, v)| (k.clone(), Json::String(v.clone())))
                    .collect(),
            ),
        );
        if node.kind != "file" {
            record.insert(
                "lines".to_owned(),
                Json::Array(vec![node.lines.0.into(), node.lines.1.into()]),
            );
            record.insert("text".to_owned(), Json::String(node.text.to_owned()));
        }
        Json::Object(record)
    }

    /// Rows as a listing: the shown variables, then a line
    /// per capture, and a blank line between rows of several lines.
    fn listing(&self, out: &mut impl Write, rows: &[Row], whole: bool) -> std::io::Result<()> {
        let shown = self.query.shown_vars();
        let captures = self.query.captures();
        for (index, row) in rows.iter().enumerate() {
            let vars: Vec<String> = shown
                .iter()
                .filter_map(|name| Some(format!("?{name}={}", row.vars.get(*name)?)))
                .collect();
            if index > 0 && (captures.len() > 1 || !vars.is_empty()) && !whole {
                writeln!(out)?;
            }
            if !vars.is_empty() {
                writeln!(out, "{}", vars.join("  "))?;
            }
            for name in captures {
                let label = if name.is_empty() {
                    String::new()
                } else {
                    format!("@{name}  ")
                };
                match row.captures.get(name) {
                    Some(Capture::Value(value)) => writeln!(out, "{label}{value}")?,
                    Some(Capture::Values(values)) => {
                        let values: Vec<String> = values.iter().map(Value::to_string).collect();
                        writeln!(out, "{label}{}", values.join(", "))?;
                    }
                    Some(Capture::Collected(hits)) if !whole => {
                        let (one, many) = if name.is_empty() {
                            ("match", "matches")
                        } else {
                            ("node", "nodes")
                        };
                        let noun = if hits.len() == 1 { one } else { many };
                        writeln!(out, "{label}{} {noun}", hits.len())?;
                    }
                    Some(Capture::Nodes(hits) | Capture::Collected(hits)) => {
                        for node in hits.iter().filter_map(|hit| self.describe(hit)) {
                            writeln!(out, "{label}{}", node.line())?;
                            if whole && !node.text.is_empty() {
                                writeln!(out, "{}\n", node.text)?;
                            }
                        }
                    }
                    _ => writeln!(out, "{label}-")?,
                }
            }
        }
        Ok(())
    }

    fn describe<'h>(&'h self, hit: &'h Hit) -> Option<Described<'h>> {
        match hit {
            Hit::Node(id) => self.tree.get(*id).map(Described::node),
            Hit::Row(attrs) => Some(Described::row(attrs)),
            _ => None,
        }
    }
}

/// What output shows of a hit.
struct Described<'a> {
    address: String,
    kind: &'a str,
    attrs: &'a BTreeMap<String, String>,
    lines: (u32, u32),
    text: &'a str,
}

impl<'a> Described<'a> {
    fn node(node: NodeRef<'a>) -> Self {
        let data = node.node();
        let lines = data
            .origin
            .position()
            .map_or((0, 0), |at| (at.line(), at.end_line()));
        Self {
            address: address(node),
            kind: &data.kind,
            attrs: &data.attrs,
            lines,
            text: &data.text,
        }
    }

    fn row(attrs: &'a BTreeMap<String, String>) -> Self {
        Self {
            address: String::new(),
            kind: "row",
            attrs,
            lines: (0, 0),
            text: "",
        }
    }

    /// `address  L3-5  type  snippet`.
    fn line(&self) -> String {
        let mut parts = vec![self.address.clone()];
        let (first, last) = self.lines;
        if first > 0 && self.kind != "file" {
            parts.push(if last > first {
                format!("L{first}-{last}")
            } else {
                format!("L{first}")
            });
        }
        parts.push(self.kind.to_owned());
        if let Some(snippet) = self.snippet() {
            parts.push(snippet);
        }
        parts.join("  ")
    }

    /// The first non-blank line of the text, or a section's title.
    fn snippet(&self) -> Option<String> {
        let text = match self.kind {
            "section" => self.attrs.get("title").map_or("", String::as_str),
            _ => self.text,
        };
        let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
        Some(if line.chars().count() > SNIPPET {
            line.chars()
                .take(SNIPPET - 1)
                .chain(iter::once('…'))
                .collect()
        } else {
            line.to_owned()
        })
    }
}

/// The enclosing file's path, with the enclosing section's id after `#`.
fn address(node: NodeRef<'_>) -> String {
    let enclosing = |kind: &str| {
        iter::once(node)
            .chain(node.ancestors())
            .find(|n| n.node().kind == kind)
    };
    let Some(file) = enclosing("file") else {
        return String::new();
    };
    let mut address = file.node().attrs.get("path").cloned().unwrap_or_default();
    if let Some(id) = enclosing("section")
        .and_then(|section| section.node().attrs.get("id"))
        .filter(|id| !id.is_empty())
    {
        address.push('#');
        address.push_str(id);
    }
    address
}

fn value_json(value: &Value) -> Json {
    match value {
        Value::Number(n) => {
            Number::from_f64(*n).map_or_else(|| Json::String(n.to_string()), Json::Number)
        }
        Value::Text(text) => Json::String(text.clone()),
    }
}
