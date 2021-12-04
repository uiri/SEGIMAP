use std::io;
use std::result::Result as StdResult;
use thiserror::Error;

/// A convenient alias type for results for `mime`.
pub type Result<T> = StdResult<T, Error>;

/// Represents errors which occur during MIME parsing.
#[derive(Debug, Error)]
pub enum Error {
    /// An internal `std::io` error.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An error occurs when a `Content-Type` is unspecified for a body part.
    #[error("Missing `Content-Type` for body part.")]
    MissingContentType,
    /// An error which occurs when the parser failed to determine the MULTIPART
    /// boundary.
    #[error("Failed to parse MULTIPART boundary.")]
    ParseMultipartBoundary,
}
