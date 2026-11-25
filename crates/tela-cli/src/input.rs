//! The files a command reads, parsed into trees by their frontends.
//!
//! A directory walk reads the files a frontend claims by extension, skipping
//! hidden entries, `.git`, `node_modules`, `vendor` and what Git ignores; a
//! file named on the command line goes through the fallback frontend when
//! none claims it. Each file is read once, however the paths overlap.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use ignore::WalkBuilder;
use tela::markdown::Markdown;
use tela::plain::Plain;
use tela::{Frontend, Frontends, Tree};

use crate::failure::{self, Failure};

const SKIPPED_DIRS: [&str; 3] = [".git", "node_modules", "vendor"];

/// Markdown for `md`, plain text for named files no frontend claims.
pub(crate) fn frontends() -> Result<Frontends, Failure> {
    Frontends::new(
        vec![Arc::new(Markdown::new()), Arc::new(Plain)],
        Some("plain"),
    )
    .map_err(|error| Failure::chain(&error))
}

/// The trees of the files under `roots`, or under the current directory
/// without roots, each file node carrying its `path`. A file that cannot be
/// read is left out and reported on stderr.
pub(crate) fn read(roots: &[PathBuf], frontends: &Frontends) -> Result<Vec<Tree>, Failure> {
    let roots = if roots.is_empty() {
        &[PathBuf::from(".")][..]
    } else {
        roots
    };
    let missing: Vec<String> = roots
        .iter()
        .filter_map(|root| {
            fs::metadata(root)
                .err()
                .map(|error| format!("{}: {}", root.display(), failure::cause(&error)))
        })
        .collect();
    if !missing.is_empty() {
        return Err(Failure::new(missing.join("\ntela: ")));
    }
    let mut seen = HashSet::new();
    let mut trees = Vec::new();
    for root in roots {
        for (path, frontend) in files(root, frontends) {
            let identity = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if seen.insert(identity) {
                trees.extend(parse(&path, frontend));
            }
        }
    }
    Ok(trees)
}

/// The files `root` names: itself, or the claimed files below it in path
/// order.
fn files<'f>(root: &Path, frontends: &'f Frontends) -> Vec<(PathBuf, &'f dyn Frontend)> {
    if !root.is_dir() {
        return frontends
            .for_named(root)
            .map(|frontend| (clean(root), frontend))
            .into_iter()
            .collect();
    }
    let walk = WalkBuilder::new(root)
        .hidden(true)
        .ignore(false)
        .follow_links(true)
        .sort_by_file_name(Ord::cmp)
        .filter_entry(|entry| {
            entry.depth() == 0
                || !entry.file_type().is_some_and(|t| t.is_dir())
                || !SKIPPED_DIRS.iter().any(|dir| entry.file_name() == *dir)
        })
        .build();
    let mut found = Vec::new();
    for entry in walk {
        match entry {
            Ok(entry) if entry.file_type().is_some_and(|t| t.is_file()) => {
                if let Some(frontend) = frontends.by_path(entry.path()) {
                    found.push((clean(entry.path()), frontend));
                }
            }
            Ok(_) => {}
            Err(error) => omitted(&error),
        }
    }
    found
}

fn parse(path: &Path, frontend: &dyn Frontend) -> Option<Tree> {
    let display = path.to_string_lossy();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("tela: {display}: omitted: {}", failure::cause(&error));
            return None;
        }
    };
    let mut tree = frontend.parse(&display, &String::from_utf8_lossy(&bytes));
    let root = tree.root().id();
    if let Some(file) = tree.node_mut(root) {
        file.attrs.insert("path".to_owned(), display.into_owned());
    }
    Some(tree)
}

fn omitted(error: &ignore::Error) {
    match (location(error), cause(error)) {
        (Some(path), Some(cause)) => {
            eprintln!(
                "tela: {}: omitted: {}",
                clean(path).display(),
                failure::cause(cause)
            );
        }
        _ => eprintln!("tela: {error}"),
    }
}

fn location(error: &ignore::Error) -> Option<&Path> {
    match error {
        ignore::Error::WithPath { path, .. } => Some(path),
        ignore::Error::WithDepth { err, .. } | ignore::Error::WithLineNumber { err, .. } => {
            location(err)
        }
        _ => None,
    }
}

fn cause(error: &ignore::Error) -> Option<&std::io::Error> {
    match error {
        ignore::Error::Io(error) => Some(error),
        ignore::Error::WithPath { err, .. }
        | ignore::Error::WithDepth { err, .. }
        | ignore::Error::WithLineNumber { err, .. } => cause(err),
        _ => None,
    }
}

/// `path` without `.` components and with `dir/..` pairs folded, as Go's
/// `filepath.Clean` writes it.
fn clean(path: &Path) -> PathBuf {
    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(cleaned.components().next_back(), Some(Component::Normal(_))) =>
            {
                cleaned.pop();
            }
            other => cleaned.push(other),
        }
    }
    if cleaned.as_os_str().is_empty() {
        cleaned.push(".");
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_drops_dots_and_folds_parents() {
        for (path, expected) in [
            ("./skills/a.md", "skills/a.md"),
            ("docs/../docs/./guide.md", "docs/guide.md"),
            ("../up.md", "../up.md"),
            ("/abs/./x.md", "/abs/x.md"),
            (".", "."),
        ] {
            assert_eq!(clean(Path::new(path)), Path::new(expected), "{path}");
        }
    }
}
