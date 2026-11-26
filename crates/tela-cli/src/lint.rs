//! `tela lint`: tela's std rules, the lint modules of the user's and the
//! project's layers and `tela.toml`'s inline rules over files, and
//! `tela lint test`, which runs every rule's examples.
//!
//! The user's layer is `$XDG_CONFIG_HOME/tela/lint`, or `~/.config/tela/lint`;
//! the project's is `.tela/lint` in the nearest directory holding `.git`, or
//! else in the current directory.

use std::env;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Serialize, Serializer};
use tela::lint::{
    CompileError, Config, Corpus, Diagnostic, Expect, Layer, Outcome, Plan, RuleId, Score,
    Selection, Settings, Severity, Spec,
};
use tela::markdown::Markdown;
use tela::query::Environment;
use tela::{Frontend, Origin};

use crate::config::{self, Source};
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

pub(crate) fn run(args: &Args, source: &Source, out: &mut impl Write) -> Result<Status, Failure> {
    let config::Config {
        mut settings,
        model,
    } = source.load()?;
    for id in &args.disable {
        let id = RuleId::new(id.as_str()).map_err(|error| Failure::chain(&error))?;
        settings.disable.insert(id);
    }
    let environment = environment(settings.semantic, model.as_deref())?;
    let mut spec = Spec::default();
    config::DEFAULT.clone_into(&mut spec.labels.inline);
    let config = Config::new(layers()?, spec, settings).map_err(|error| Failure::chain(&error))?;
    let plan = Plan::compile(&config).map_err(|error| compiled(&error, config.settings()))?;
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

/// The environment rules run in, with the semantic model `dir` holds when
/// semantic rules are enabled.
fn environment(semantic: bool, dir: Option<&Path>) -> Result<Environment, Failure> {
    let mut environment = Environment::new(SystemTime::now());
    if semantic {
        let dir =
            dir.ok_or_else(|| Failure::new("semantic rules are enabled but no model_path is set"))?;
        environment.embed = Some(Arc::new(config::model(dir)?));
    }
    Ok(environment)
}

/// The module layers that exist: the user's, then the project's.
fn layers() -> Result<Vec<Layer>, Failure> {
    let user = env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::home_dir().map(|home| home.join(".config")))
        .map(|dir| dir.join("tela/lint"));
    let cwd = env::current_dir()?;
    let project = cwd
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(&cwd)
        .join(".tela/lint");
    user.into_iter()
        .chain([project])
        .filter(|dir| dir.is_dir())
        .map(|dir| Layer::read_dir(None, dir).map_err(|error| Failure::chain(&error)))
        .collect()
}

/// A compile error; an inline rule's query error points into its query.
fn compiled(error: &CompileError, settings: &Settings) -> Failure {
    if let CompileError::Query {
        module,
        rule,
        source,
    } = error
        && let Some(inline) = settings.rules.iter().find(|inline| &inline.id == rule)
    {
        return Failure::explain(&inline.query, source)
            .context(format!("module {module:?}, rule {rule}"));
    }
    Failure::chain(error)
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
        let at = format!("{}:{}", example.module, example.line);
        let found = match plan.run_example(example, &corpus) {
            Ok(Outcome::Pass) => continue,
            Ok(Outcome::Fail { found }) => found,
            Ok(_) => 0,
            Err(error) => {
                failed += 1;
                writeln!(out, "{at}: {}", Failure::chain(&error))?;
                continue;
            }
        };
        failed += 1;
        match example.expect {
            Expect::Matches => writeln!(
                out,
                "{at}: {} should report this example, but did not",
                example.rule
            )?,
            Expect::Passes => writeln!(
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

/// A diagnostic as `--format json` writes it.
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
            Some(Score::Jaccard { shared, .. }) => (Some("jaccard"), shared, None),
            Some(Score::Containment { shared, .. }) => (Some("containment"), shared, None),
            Some(Score::Cosine { model, .. }) => (Some("cosine"), &[], Some(model.as_str())),
            _ => (None, &[], None),
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
