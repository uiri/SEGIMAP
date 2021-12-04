use nom;
use std::error::Error as StdError;
use std::fmt;
use std::result::Result as StdResult;

/// A convenient alias type for results for `parser`.
pub type Result<T> = StdResult<T, Error>;

/// Represents errors which occur while parsing.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// An internal `nom` error.
    Nom(nom::Err),
    /// Incomplete input was fed to the parser.
    Incomplete,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match *self {
            Incomplete => write!(f, "{}", StdError::description(self)),
            Nom(ref e) => e.fmt(f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;

        match *self {
            Incomplete => "Incomplete input was fed to the parser.",
            Nom(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&dyn StdError> {
        use self::Error::*;

        match *self {
            Incomplete => None,
            Nom(ref e) => e.source(),
        }
    }
}

impl From<nom::Err> for Error {
    fn from(error: nom::Err) -> Error {
        Error::Nom(error)
    }
}
