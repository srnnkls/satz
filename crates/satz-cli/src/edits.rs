//! Edits on disk: applied in place, or shown as a unified diff.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;

use satz::edit::{self, Change};

use crate::failure::Failure;

/// Writes `changes` to their files, or prints their diff; returns how many
/// files change.
pub(crate) fn emit(
    changes: Vec<Change>,
    write: bool,
    out: &mut impl Write,
) -> Result<usize, Failure> {
    let mut by_file: BTreeMap<String, Vec<Change>> = BTreeMap::new();
    for change in changes {
        by_file
            .entry(change.file().to_owned())
            .or_default()
            .push(change);
    }
    let mut edited = Vec::with_capacity(by_file.len());
    for (file, changes) in &by_file {
        let text = fs::read_to_string(file).map_err(|error| Failure::from(error).context(file))?;
        if write {
            let text = edit::apply(&text, changes).map_err(|error| Failure::chain(&error))?;
            edited.push((file, text));
        } else {
            let diff = edit::diff(file, &text, changes).map_err(|error| Failure::chain(&error))?;
            out.write_all(diff.as_bytes())?;
        }
    }
    for (file, text) in edited {
        fs::write(file, text).map_err(|error| Failure::from(error).context(file))?;
        writeln!(out, "edited {file}")?;
    }
    Ok(by_file.len())
}
