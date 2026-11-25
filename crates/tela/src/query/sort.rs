//! Sort keys: `?var`, `@value` or `@capture.key`, descending with a leading
//! `-`.

use std::collections::BTreeMap;

use super::check::Mention;
use super::lex::is_name;
use super::read::suggest;
use super::{Error, ErrorKind, Query};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SortKey {
    pub(super) text: String,
    pub(super) descending: bool,
    pub(super) by: SortBy,
    pub(super) pos: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SortBy {
    Var(String),
    Capture(String),
    Key(String, String),
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().enumerate().all(|(i, c)| is_name(c, i == 0))
}

impl SortKey {
    /// Parses `text`, reporting errors at `pos`.
    pub(super) fn parse(text: &str, pos: usize) -> Result<Self, Error> {
        let (descending, rest) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let by = if let Some(var) = rest.strip_prefix('?').filter(|v| valid_name(v)) {
            Some(SortBy::Var(var.to_owned()))
        } else if let Some(capture) = rest.strip_prefix('@') {
            match capture.rfind('.') {
                Some(dot)
                    if dot > 0
                        && valid_name(&capture[..dot])
                        && valid_name(&capture[dot + 1..]) =>
                {
                    Some(SortBy::Key(
                        capture[..dot].to_owned(),
                        capture[dot + 1..].to_owned(),
                    ))
                }
                _ => valid_name(capture).then(|| SortBy::Capture(capture.to_owned())),
            }
        } else {
            None
        };
        let by = by.ok_or_else(|| {
            Error::syntax(
                pos,
                format!("--sort {text} is neither ?var, @value nor @capture.key"),
            )
            .with_hint("e.g. --sort -@n, --sort ?l or --sort @s.words")
        })?;
        Ok(Self {
            text: text.to_owned(),
            descending,
            by,
            pos,
        })
    }

    pub(super) fn var(&self) -> Option<&str> {
        match &self.by {
            SortBy::Var(var) => Some(var),
            _ => None,
        }
    }
}

impl Query {
    pub(super) fn check_sorts(&self, mentions: &[Mention]) -> Result<(), Error> {
        for sort in &self.sorts {
            let text = &sort.text;
            let unbound = |message: String| Error::new(ErrorKind::Unbound, sort.pos, message);
            let (capture, key) = match &sort.by {
                SortBy::Capture(capture) => (capture, None),
                SortBy::Key(capture, key) => (capture, Some(key)),
                SortBy::Var(var) => {
                    self.check_sort_var(sort, var, mentions)?;
                    continue;
                }
            };
            if !self.captures.contains(capture) {
                let candidates: Vec<&str> = self.captures.iter().map(String::as_str).collect();
                return Err(unbound(format!(
                    "--sort {text} names @{capture}, which no pattern captures"
                ))
                .with_hint(suggest(capture, &candidates)));
            }
            let value = self.values.contains(capture)
                || self.group.as_ref().is_some_and(|g| g.is_output(capture));
            if value && key.is_some() {
                return Err(Error::syntax(
                    sort.pos,
                    format!("--sort {text} reads a key, but @{capture} captures a value"),
                )
                .with_hint(format!("sort by the value itself: --sort @{capture}")));
            }
            if !value && key.is_none() {
                return Err(Error::syntax(
                    sort.pos,
                    format!("--sort {text} names @{capture}, which captures nodes"),
                )
                .with_hint(format!(
                    "sort by a key of its nodes, as in --sort @{capture}.words"
                )));
            }
        }
        Ok(())
    }

    fn check_sort_var(&self, sort: &SortKey, var: &str, mentions: &[Mention]) -> Result<(), Error> {
        let text = &sort.text;
        let bound = mentions
            .iter()
            .any(|m| m.name == var && m.binds() && !m.negated);
        let unbound = |message: String| Error::new(ErrorKind::Unbound, sort.pos, message);
        let Some(group) = &self.group else {
            if bound {
                return Ok(());
            }
            return Err(unbound(format!(
                "--sort {text} names ?{var}, which no pattern binds"
            ))
            .with_hint(format!(
                "bind it with :key ?{var} in a pattern of the query"
            )));
        };
        if group.is_output(var) {
            let minus = if sort.descending { "-" } else { "" };
            return Err(Error::syntax(
                sort.pos,
                format!("--sort {text} names ?{var}, but aggregates are captures"),
            )
            .with_hint(format!("sort by --sort {minus}@{var}")));
        }
        if group.keys.iter().any(|k| k.input.var() == Some(var)) {
            return Ok(());
        }
        if bound {
            return Err(unbound(format!(
                "--sort {text} names ?{var}, which (group ...) drops"
            ))
            .with_hint(format!(
                "key the group on ?{var}, or sort by a key or aggregate of the group"
            )));
        }
        Err(unbound(format!(
            "--sort {text} names ?{var}, which no pattern or aggregate binds"
        ))
        .with_hint("sort by a key or aggregate of the group"))
    }

    /// Counts the variables sort keys and message placeholders use.
    pub(super) fn output_uses(&self, uses: &mut BTreeMap<String, usize>) {
        let names = self
            .sorts
            .iter()
            .filter_map(SortKey::var)
            .chain(self.printed.iter().map(String::as_str));
        for name in names {
            *uses.entry(name.to_owned()).or_default() += 1;
        }
    }
}
