use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::result::Result as StdResult;

/// A convenient alias type for results for `mime`.
pub type Result<T> = StdResult<T, Error>;

/// Represents errors which occur during MIME parsing.
#[derive(Debug)]
pub enum Error {
    /// An internal `std::io` error.
    Io(io::Error),
    /// An error occurs when a `Content-Type` is unspecified for a body part.
    MissingContentType,
    /// An error which occurs when the parser failed to determine the MULTIPART
    /// boundary.
    ParseMultipartBoundary,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match *self {
            MissingContentType |
                ParseMultipartBoundary => write!(f, "{}", StdError::description(self)),
            Io(ref e) => e.fmt(f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;

        match *self {
            MissingContentType => "Missing `Content-Type` for body part.",
            ParseMultipartBoundary => "Failed to parse MULTIPART boundary.",
            Io(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use self::Error::*;

        match *self {
            ParseMultipartBoundary |
                MissingContentType => None,
            Io(ref e) => e.cause(),
        }
    }
}

// Implement `PartialEq` manually, since `std::io::Error` does not implement it.
impl PartialEq<Error> for Error {
    fn eq(&self, other: &Error) -> bool {
        use self::Error::*;

        match (self, other) {
            (&Io(_), &Io(_)) |
                (&MissingContentType, &MissingContentType) |
                (&ParseMultipartBoundary, &ParseMultipartBoundary) => true,
            _ => false,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Error {
        Error::Io(error)
    }
}
