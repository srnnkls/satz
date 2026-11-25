//! `tela lint`: tela's std rules and `tela.toml`'s inline rules over files,
//! and `tela lint test`, which runs every rule's examples.

use std::fmt;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Serialize, Serializer};
use tela::lint::{Config, Corpus, Expect, Outcome, Score, Selection, Settings, Spec};
use tela::markdown::Markdown;
use tela::query::Environment;
use tela::similarity::Model2Vec;
use tela::{Diagnostic, Frontend, Origin, Plan, RuleId, Severity};

use crate::config::Source;
use crate::failure::Failure;
use crate::{Status, input, json};

#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    /// Files and directories to lint [default: the current directory]
    paths: Vec<PathBuf>,
    /// Diagnostic output.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Exit unsuccessfully on warnings as well as errors.
    #[arg(long)]
    strict: bool,
    /// Rules to disable (comma-separated, repeatable).
    #[arg(long, value_name = "RULE", value_delimiter = ',')]
    disable: Vec<String>,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Run every rule's Matches and Passes examples.
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Format {
    Text,
    Json,
}

pub(crate) fn run(args: &Args, config: &Source, out: &mut impl Write) -> Result<Status, Failure> {
    let mut settings = config.settings()?;
    for id in &args.disable {
        let id = RuleId::new(id.as_str()).map_err(|error| Failure::chain(&error))?;
        settings.disable.insert(id);
    }
    let environment = environment(&settings)?;
    let config = Config::new(Vec::new(), Spec::default(), settings)
        .map_err(|error| Failure::chain(&error))?;
    let plan = Plan::compile(&config).map_err(|error| Failure::chain(&error))?;
    if let Some(Command::Test) = &args.command {
        return test(&plan, &environment, out);
    }
    let trees = input::read(&args.paths, &input::frontends()?)?;
    let corpus = Corpus::new(trees, environment).map_err(|error| Failure::chain(&error))?;
    let diagnostics = plan
        .run(&corpus, &Selection::All)
        .map_err(|error| Failure::chain(&error))?;
    match args.format {
        Format::Json => json::write(
            out,
            &diagnostics.iter().map(Record::new).collect::<Vec<_>>(),
        )?,
        Format::Text => {
            for diagnostic in &diagnostics {
                let (line, column) = location(diagnostic.origin());
                writeln!(
                    out,
                    "{}:{line}:{column}: {} [{}] {}",
                    diagnostic.origin().file(),
                    diagnostic.severity(),
                    diagnostic.rule(),
                    diagnostic.message()
                )?;
            }
        }
    }
    let failing = diagnostics
        .iter()
        .any(|d| args.strict || d.severity() == Severity::Error);
    Ok(if failing {
        Status::Findings
    } else {
        Status::Clean
    })
}

/// The environment rules run in, with the semantic model when the settings
/// enable semantic rules.
fn environment(settings: &Settings) -> Result<Environment, Failure> {
    let mut environment = Environment::new(SystemTime::now());
    if settings.semantic.enabled
        && let Some(dir) = &settings.semantic.model_path
    {
        let model = Model2Vec::load(dir).map_err(|error| Failure::chain(&error))?;
        environment.embed = Some(Arc::new(model));
    }
    Ok(environment)
}

/// Runs each example over a corpus of its Markdown, printing the failures.
fn test(plan: &Plan, environment: &Environment, out: &mut impl Write) -> Result<Status, Failure> {
    let markdown = Markdown::new();
    let examples = plan.examples();
    let mut failed = 0;
    for example in examples {
        let mut tree = markdown.parse("example.md", &example.text);
        let root = tree.root().id();
        if let Some(file) = tree.node_mut(root) {
            file.attrs
                .insert("path".to_owned(), "example.md".to_owned());
        }
        let corpus =
            Corpus::new([tree], environment.clone()).map_err(|error| Failure::chain(&error))?;
        let Outcome::Fail { found, problem } = plan.run_example(example, &corpus) else {
            continue;
        };
        failed += 1;
        let at = format!("{}:{}", example.module, example.line);
        match (problem, example.expect) {
            (Some(problem), _) => writeln!(out, "{at}: {} {problem}", example.rule)?,
            (None, Expect::Matches) => writeln!(
                out,
                "{at}: {} should report this example, but did not",
                example.rule
            )?,
            (None, Expect::Passes) => writeln!(
                out,
                "{at}: {} should pass this example, but reported {found} diagnostics",
                example.rule
            )?,
        }
    }
    if failed > 0 {
        eprintln!("tela: {failed} of {} lint examples failed", examples.len());
        return Ok(Status::Findings);
    }
    writeln!(out, "{} lint examples passed", examples.len())?;
    Ok(Status::Clean)
}

/// The line and column of `origin`; a whole-file origin points at 1:1.
fn location(origin: &Origin) -> (u32, u32) {
    origin
        .position()
        .map_or((1, 1), |at| (at.line(), at.column()))
}

/// A diagnostic as henia's `--format json` writes it, without `stage`.
#[derive(Serialize)]
struct Record<'a> {
    path: &'a str,
    line: u32,
    column: u32,
    #[serde(serialize_with = "display")]
    severity: Severity,
    rule: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    related: Vec<Location<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    similarity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<&'static str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    shared_phrases: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
}

fn display<S: Serializer>(value: &impl fmt::Display, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

#[derive(Serialize)]
struct Location<'a> {
    path: &'a str,
    line: u32,
    column: u32,
}

impl<'a> Location<'a> {
    fn new(origin: &'a Origin) -> Self {
        let (line, column) = location(origin);
        Self {
            path: origin.file(),
            line,
            column,
        }
    }
}

impl<'a> Record<'a> {
    fn new(diagnostic: &'a Diagnostic) -> Self {
        let origin = Location::new(diagnostic.origin());
        let (method, shared_phrases, model): (_, &[String], _) = match diagnostic.score() {
            None => (None, &[], None),
            Some(Score::Jaccard { shared, .. }) => (Some("jaccard"), shared, None),
            Some(Score::Containment { shared, .. }) => (Some("containment"), shared, None),
            Some(Score::Cosine { model, .. }) => (Some("cosine"), &[], Some(model.as_str())),
        };
        Self {
            path: origin.path,
            line: origin.line,
            column: origin.column,
            severity: diagnostic.severity(),
            rule: diagnostic.rule().as_str(),
            message: diagnostic.message(),
            related: diagnostic.related().iter().map(Location::new).collect(),
            similarity: diagnostic.score().map(Score::value),
            method,
            shared_phrases,
            model,
        }
    }
}
