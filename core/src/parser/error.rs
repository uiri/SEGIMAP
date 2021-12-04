use nom;
use std::result::Result as StdResult;
use thiserror::Error;

/// A convenient alias type for results for `parser`.
pub type Result<T> = StdResult<T, Error>;

/// Represents errors which occur while parsing.
#[derive(Debug, PartialEq, Error)]
pub enum Error {
    /// An internal `nom` error.
    #[error(transparent)]
    Nom(#[from] nom::Err),
    /// Incomplete input was fed to the parser.
    #[error("Incomplete input was fed to the parser.")]
    Incomplete,
}
