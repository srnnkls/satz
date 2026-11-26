//! Compiling a configuration into a plan: rule groups from modules and
//! inline rules, with the user's settings applied and checked.

use std::collections::BTreeMap;

use super::load::{self, Clause, Group, Loader};
use super::{CompileError, Config, Example, Plan, RuleConfig, RuleId, RuleMeta};
use crate::query::{ParamKind, Table, TableKind, Value, suggest};

/// A rule's clauses with its effective params and data.
#[derive(Debug, Clone)]
pub(super) struct Compiled {
    pub(super) clauses: Vec<Clause>,
    pub(super) params: BTreeMap<String, Value>,
    pub(super) data: BTreeMap<String, Table>,
}

/// Params and data typed as a rule declares them.
struct Typed {
    params: BTreeMap<String, Value>,
    data: BTreeMap<String, Table>,
}

impl Plan {
    /// Compiles satz's std modules, the layers' modules and the inline rules,
    /// then applies the per-rule config.
    ///
    /// Modules are named by their path without `.md`, under their layer's
    /// package; std modules under the spec's std label. A rule's params
    /// that no module or config sets are read from the environment when the
    /// plan runs.
    ///
    /// # Errors
    /// Returns [`CompileError`] when a module is defined twice, is malformed
    /// or declares an unknown frontmatter key, a query does not read, a rule
    /// id is defined twice, or the settings name a rule, param or table that
    /// does not exist or give a value of the wrong type.
    pub fn compile(config: &Config) -> Result<Self, CompileError> {
        let (spec, settings) = (config.spec(), config.settings());
        let mut groups = Loader::new(config.layers(), spec)?.groups()?;
        for rule in &settings.rules {
            groups.push(load::inline(rule, spec)?);
        }
        unique(&groups)?;
        for (id, rule) in &settings.config {
            let index = position(&groups, id)?;
            groups[index].configure(rule)?;
        }
        for id in &settings.disable {
            position(&groups, id)?;
        }
        let mut examples = Vec::new();
        for group in &groups {
            group.check_numbers()?;
            examples.extend(group.examples()?);
        }
        let rules = groups.iter().map(Group::meta).collect();
        let compiled = groups
            .into_iter()
            .map(|group| Compiled {
                clauses: group.clauses,
                params: group.params,
                data: group.data,
            })
            .collect();
        Ok(Self {
            rules,
            examples,
            compiled,
            disabled: settings.disable.clone(),
            disable_key: settings.disable_key.clone(),
            semantic: settings.semantic,
        })
    }
}

fn unique(groups: &[Group]) -> Result<(), CompileError> {
    let mut first: BTreeMap<&RuleId, &str> = BTreeMap::new();
    for group in groups {
        if let Some(module) = first.insert(&group.id, &group.module) {
            return Err(CompileError::DuplicateRule {
                rule: group.id.clone(),
                first: module.to_owned(),
                second: group.module.clone(),
            });
        }
    }
    Ok(())
}

fn position(groups: &[Group], id: &RuleId) -> Result<usize, CompileError> {
    groups.iter().position(|g| &g.id == id).ok_or_else(|| {
        let ids: Vec<&str> = groups.iter().map(|g| g.id.as_str()).collect();
        CompileError::UnknownRule {
            rule: id.clone(),
            hint: suggest(id.as_str(), &ids),
        }
    })
}

impl Group {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: self.id.clone(),
            module: self.module.clone(),
            severity: self.clauses[0].rule.severity,
            meta: self.meta.clone(),
        }
    }

    fn configure(&mut self, config: &RuleConfig) -> Result<(), CompileError> {
        if let Some(severity) = config.severity {
            for clause in &mut self.clauses {
                clause.rule.severity = severity;
            }
        }
        let typed = self.typed(&config.params, &config.data)?;
        self.params.extend(typed.params);
        self.data.extend(typed.data);
        Ok(())
    }

    /// Checks that every param a clause compares as a number holds one.
    fn check_numbers(&self) -> Result<(), CompileError> {
        for clause in &self.clauses {
            for (name, kind) in clause.rule.params() {
                if kind == ParamKind::Number
                    && matches!(self.params.get(&name), Some(Value::Text(_)))
                {
                    return Err(self.param_type(&name, kind));
                }
            }
        }
        Ok(())
    }

    fn examples(&self) -> Result<Vec<Example>, CompileError> {
        self.samples
            .iter()
            .map(|sample| {
                let mut params = BTreeMap::new();
                let mut data: BTreeMap<String, Table> = BTreeMap::new();
                for (key, value) in &sample.settings {
                    match key.split_once('.') {
                        Some((table, row)) => {
                            let rows = data
                                .entry(table.to_owned())
                                .or_insert_with(|| Table::Map(Vec::new()));
                            if let Table::Map(rows) = rows {
                                rows.push((row.to_owned(), value.clone()));
                            }
                        }
                        None => {
                            params.insert(key.clone(), Value::Text(value.clone()));
                        }
                    }
                }
                let Typed { params, data } = self.typed(&params, &data)?;
                Ok(Example {
                    rule: self.id.clone(),
                    module: self.module.clone(),
                    line: sample.line,
                    expect: sample.expect,
                    text: sample.text.clone(),
                    params,
                    data,
                })
            })
            .collect()
    }

    /// `params` and `data` typed as the rule declares them.
    fn typed(
        &self,
        params: &BTreeMap<String, Value>,
        data: &BTreeMap<String, Table>,
    ) -> Result<Typed, CompileError> {
        let params = params
            .iter()
            .map(|(name, value)| {
                let typed = match (self.params.get(name), value) {
                    (None, _) => return Err(self.unknown(name)),
                    (Some(Value::Number(_)), Value::Number(n)) => Value::Number(*n),
                    (Some(Value::Number(_)), Value::Text(text)) => text
                        .parse()
                        .map(Value::Number)
                        .map_err(|_| self.param_type(name, ParamKind::Number))?,
                    (Some(Value::Text(_)), Value::Text(text)) => Value::Text(text.clone()),
                    (Some(Value::Text(_)), Value::Number(_)) => {
                        return Err(self.param_type(name, ParamKind::Text));
                    }
                };
                Ok((name.clone(), typed))
            })
            .collect::<Result<_, _>>()?;
        let data = data
            .iter()
            .map(|(name, table)| match self.data.get(name) {
                None => Err(self.unknown(name)),
                Some(declared) if declared.kind() != table.kind() => {
                    Err(self.table_shape(name, declared.kind()))
                }
                Some(_) => Ok((name.clone(), table.clone())),
            })
            .collect::<Result<_, _>>()?;
        Ok(Typed { params, data })
    }

    /// The error for a setting the rule lacks under that kind: a table set
    /// as a param, a param set as a table, or neither.
    fn unknown(&self, key: &str) -> CompileError {
        if let Some(table) = self.data.get(key) {
            return self.table_shape(key, table.kind());
        }
        match self.params.get(key) {
            Some(Value::Number(_)) => self.param_type(key, ParamKind::Number),
            Some(Value::Text(_)) => self.param_type(key, ParamKind::Text),
            None => {
                let known: Vec<&str> = self
                    .params
                    .keys()
                    .chain(self.data.keys())
                    .map(String::as_str)
                    .collect();
                let hint = if known.is_empty() {
                    "it takes none".to_owned()
                } else {
                    suggest(key, &known)
                };
                CompileError::UnknownSetting {
                    rule: self.id.clone(),
                    key: key.to_owned(),
                    hint,
                }
            }
        }
    }

    fn param_type(&self, param: &str, expected: ParamKind) -> CompileError {
        CompileError::ParamType {
            rule: self.id.clone(),
            param: param.to_owned(),
            expected,
        }
    }

    fn table_shape(&self, table: &str, expected: TableKind) -> CompileError {
        CompileError::TableShape {
            rule: self.id.clone(),
            table: table.to_owned(),
            expected,
        }
    }
}
