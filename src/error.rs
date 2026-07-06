//! Error type for the public `imaginu` API.
//!
//! Every fallible entry point on the library boundary returns
//! [`Result<T>`](crate::Result), so a malformed recipe or a failed file
//! operation surfaces as a recoverable [`Error`] — never a panic. This matters
//! because recipe JSON is typically agent- or user-authored: hostile or
//! nonsensical input must not be able to crash a host that embeds imaginu.

use std::fmt;

/// Anything that can go wrong compiling a recipe into an asset.
#[derive(Debug)]
pub enum Error {
    /// The recipe JSON was malformed or did not match the recipe schema.
    Parse(String),
    /// A well-formed recipe could not be compiled: an unknown palette,
    /// invalid geometry parameters, or unresolved bone/animation references.
    Build(String),
    /// A file could not be read or written.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The inner messages already read as complete sentences and match
            // what the CLI prints today, so don't re-prefix them.
            Error::Parse(m) | Error::Build(m) => write!(f, "{m}"),
            Error::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// A [`Result`](std::result::Result) whose error is imaginu's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
