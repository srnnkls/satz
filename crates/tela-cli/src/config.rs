//! `tela.toml`: the `[lint]` table, read into [`Settings`].

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tela::lint::{InlineRule, RuleConfig, Semantic, Settings};
use tela::query::{Table, Value};
use tela::{RuleId, Severity};

use crate::failure::Failure;

const DEFAULT: &str = "tela.toml";

/// Where the config comes from: the file `--config` names, which must exist,
/// or `tela.toml` in the current directory, if there is one.
#[derive(Debug)]
pub(crate) struct Source {
    named: Option<PathBuf>,
}

impl Source {
    pub(crate) fn new(named: Option<PathBuf>) -> Self {
        Self { named }
    }

    /// The lint settings the config file sets, or the defaults without one.
    pub(crate) fn settings(&self) -> Result<Settings, Failure> {
        let path = self.named.as_deref().unwrap_or(Path::new(DEFAULT));
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound && self.named.is_none() => {
                return Ok(Settings::default());
            }
            Err(error) => return Err(Failure::from(error).context(path.display())),
        };
        parse(&text).map_err(|failure| failure.context(path.display()))
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    lint: Lint,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Lint {
    rules: Vec<Rule>,
    config: BTreeMap<String, toml::Table>,
    disable: Vec<String>,
    disable_key: Option<String>,
    semantic: SemanticTable,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    id: String,
    query: String,
    message: String,
    severity: Option<String>,
    at: Option<String>,
    related: Option<String>,
    #[serde(default)]
    params: toml::Table,
    #[serde(default)]
    data: toml::Table,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SemanticTable {
    enabled: bool,
    model_path: Option<PathBuf>,
}

/// The settings `text` configures: TOML scalars become params, booleans as
/// `true` or `false` text, and tables and arrays become data tables.
fn parse(text: &str) -> Result<Settings, Failure> {
    let file: File =
        toml::from_str(text).map_err(|error| Failure::new(error.to_string().trim_end()))?;
    let lint = file.lint;
    let defaults = Settings::default();
    let mut config = BTreeMap::new();
    for (id, table) in lint.config {
        let rule = rule_id(&id)?;
        let overrides = rule_config(table).map_err(|f| f.context(format!("[lint.config.{id}]")))?;
        config.insert(rule, overrides);
    }
    Ok(Settings {
        rules: lint
            .rules
            .into_iter()
            .map(Rule::into_inline)
            .collect::<Result<_, _>>()?,
        config,
        disable: lint
            .disable
            .iter()
            .map(|id| rule_id(id))
            .collect::<Result<_, _>>()?,
        disable_key: lint.disable_key.unwrap_or(defaults.disable_key),
        semantic: Semantic {
            enabled: lint.semantic.enabled,
            model_path: lint.semantic.model_path,
        },
    })
}

impl Rule {
    fn into_inline(self) -> Result<InlineRule, Failure> {
        let context = format!("[[lint.rules]] {}", self.id);
        let rule = || -> Result<InlineRule, Failure> {
            let mut params = BTreeMap::new();
            for (key, value) in self.params {
                let value = scalar(&value)
                    .ok_or_else(|| Failure::new(format!("param {key} must be a scalar")))?;
                params.insert(key, value);
            }
            let mut data = BTreeMap::new();
            for (key, value) in self.data {
                let table = table(&value).ok_or_else(|| {
                    Failure::new(format!("data {key} must be a table or an array of scalars"))
                })?;
                data.insert(key, table);
            }
            Ok(InlineRule {
                id: rule_id(&self.id)?,
                query: self.query,
                message: self.message,
                severity: self.severity.as_deref().map(severity).transpose()?,
                at: self.at,
                related: self.related,
                params,
                data,
            })
        };
        rule().map_err(|failure| failure.context(context))
    }
}

fn rule_config(rule: toml::Table) -> Result<RuleConfig, Failure> {
    let mut config = RuleConfig::default();
    for (key, value) in rule {
        if key == "severity" {
            let name = value
                .as_str()
                .ok_or_else(|| Failure::new("severity must be text"))?;
            config.severity = Some(severity(name)?);
        } else if let Some(param) = scalar(&value) {
            config.params.insert(key, param);
        } else {
            let table = table(&value).ok_or_else(|| {
                Failure::new(format!("data {key} must be a table or an array of scalars"))
            })?;
            config.data.insert(key, table);
        }
    }
    Ok(config)
}

fn rule_id(id: &str) -> Result<RuleId, Failure> {
    RuleId::new(id).map_err(|error| Failure::chain(&error))
}

fn severity(name: &str) -> Result<Severity, Failure> {
    name.parse().map_err(|error| Failure::chain(&error))
}

fn scalar(value: &toml::Value) -> Option<Value> {
    match value {
        toml::Value::Integer(n) => Some(Value::Number(integer(*n))),
        toml::Value::Float(n) => Some(Value::Number(*n)),
        toml::Value::Array(_) | toml::Value::Table(_) => None,
        other => text(other).map(Value::Text),
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "query numbers are f64, as henia's are"
)]
fn integer(n: i64) -> f64 {
    n as f64
}

fn table(value: &toml::Value) -> Option<Table> {
    match value {
        toml::Value::Table(rows) => rows
            .iter()
            .map(|(key, value)| Some((key.clone(), text(value)?)))
            .collect::<Option<_>>()
            .map(Table::Map),
        toml::Value::Array(rows) => rows
            .iter()
            .map(text)
            .collect::<Option<_>>()
            .map(Table::List),
        _ => None,
    }
}

fn text(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(text) => Some(text.clone()),
        toml::Value::Integer(n) => Some(n.to_string()),
        toml::Value::Float(n) => Some(n.to_string()),
        toml::Value::Boolean(flag) => Some(flag.to_string()),
        toml::Value::Datetime(datetime) => Some(datetime.to_string()),
        toml::Value::Array(_) | toml::Value::Table(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_config_splits_params_and_data() {
        let settings = parse(
            r#"
            [lint]
            disable = ["duplicate-heading"]
            disable_key = "henia.lint.disable"

            [lint.config.similar-content]
            severity = "error"
            similarity = 0.3
            strict = true

            [lint.config.outdated-reference.outdated]
            zeta = "z"
            alpha = "a"
            "#,
        )
        .unwrap();
        assert_eq!(settings.disable_key, "henia.lint.disable");
        assert!(settings.disable.contains("duplicate-heading"));
        let similar = &settings.config["similar-content"];
        assert_eq!(similar.severity, Some(Severity::Error));
        assert_eq!(similar.params["similarity"], Value::Number(0.3));
        assert_eq!(similar.params["strict"], Value::Text("true".into()));
        assert_eq!(
            settings.config["outdated-reference"].data["outdated"],
            Table::Map(vec![
                ("zeta".into(), "z".into()),
                ("alpha".into(), "a".into())
            ])
        );
    }

    #[test]
    fn defaults_without_lint_table() {
        let settings = parse("").unwrap();
        assert_eq!(settings.disable_key, "tela.lint.disable");
        assert!(!settings.semantic.enabled);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let failure = parse("[lint]\nstage = \"source\"\n").unwrap_err();
        assert!(
            failure.to_string().contains("unknown field `stage`"),
            "{failure}"
        );
    }

    #[test]
    fn inline_rules_type_params_and_data() {
        let settings = parse(
            r#"
            [[lint.rules]]
            id = "short"
            query = "(paragraph :words (< $min)) @p"
            message = "short"
            severity = "warning"
            params = { min = 3 }
            data = { words = ["a", "b"] }
            "#,
        )
        .unwrap();
        let rule = &settings.rules[0];
        assert_eq!(rule.params["min"], Value::Number(3.0));
        assert_eq!(
            rule.data["words"],
            Table::List(vec!["a".into(), "b".into()])
        );
        assert_eq!(rule.severity, Some(Severity::Warning));
    }
}
