//! Why a command stopped: a usage, query, config or I/O error, printed on
//! stderr with exit status 2.

use std::error::Error;
use std::fmt;
use std::io;

/// A command's failure, with its causes joined into one message.
#[derive(Debug)]
pub(crate) struct Failure(String);

impl Failure {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// `error` and its sources, outermost first.
    pub(crate) fn chain(error: &dyn Error) -> Self {
        let mut message = error.to_string();
        let mut source = error.source();
        while let Some(cause) = source {
            message.push_str(": ");
            message.push_str(&cause.to_string());
            source = cause.source();
        }
        Self(message)
    }

    /// The failure with `context` before its message.
    pub(crate) fn context(self, context: impl fmt::Display) -> Self {
        Self(format!("{context}: {}", self.0))
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self(cause(&error))
    }
}

/// An I/O error as the operating system words it, without Rust's
/// `(os error N)` suffix: `permission denied`.
pub(crate) fn cause(error: &io::Error) -> String {
    let Some(code) = error.raw_os_error() else {
        return error.to_string();
    };
    let message = error.to_string();
    let message = message
        .strip_suffix(&format!(" (os error {code})"))
        .unwrap_or(&message);
    let mut chars = message.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}
