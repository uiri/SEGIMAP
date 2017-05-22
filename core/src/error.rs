use mime;
use serde_json::Error as JsonError;
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::result::Result as StdResult;
use toml::ser::Error as TomlError;

/// A convenient alias type for results for `segimap`.
pub type ImapResult<T> = StdResult<T, Error>;

/// Represents errors which occur during SEGIMAP operation.
#[derive(Debug)]
pub enum Error {
    InvalidImapState,
    /// An internal `std::io` error.
    Io(io::Error),
    /// An internal `serde_json` error which occurs when serializing or
    /// deserializing JSON data.
    Json(JsonError),
    /// An error which occurs when attempting to read the UID for a message.
    MessageUidDecode,
    /// An error which occurs when a Maildir message has a bad filename
    MessageBadFilename,
    /// An internal `mime` error.
    Mime(mime::Error),
    /// An internal `toml` error which occurs when serializing or deserializing
    /// TOML data.
    Toml(TomlError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match *self {
            InvalidImapState | MessageUidDecode | MessageBadFilename => write!(f, "{}", StdError::description(self)),
            Io(ref e) => e.fmt(f),
            Json(ref e) => e.fmt(f),
            Mime(ref e) => e.fmt(f),
            Toml(ref e) => e.fmt(f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;

        match *self {
            InvalidImapState => "Not in selected state.",
            MessageUidDecode => "An error occured while decoding the UID for a message.",
            MessageBadFilename => "An error occured while parsing message information from its filename",
            Io(ref e) => e.description(),
            Json(ref e) => e.description(),
            Mime(ref e) => e.description(),
            Toml(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use self::Error::*;

        match *self {
            InvalidImapState | MessageUidDecode | MessageBadFilename => None,
            Io(ref e) => e.cause(),
            Json(ref e) => e.cause(),
            Mime(ref e) => e.cause(),
            Toml(ref e) => e.cause(),
        }
    }
}

// Implement `PartialEq` manually, since `std::io::Error` does not implement it.
impl PartialEq<Error> for Error {
    fn eq(&self, other: &Error) -> bool {
        use self::Error::*;

        match (self, other) {
            (&InvalidImapState, &InvalidImapState) |
                (&Io(_), &Io(_)) |
                (&Json(_), &Json(_)) |
                (&Mime(_), &Mime(_)) |
                (&Toml(_), &Toml(_)) => true,
            _ => false,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Error {
        Error::Io(error)
    }
}

impl From<JsonError> for Error {
    fn from(error: JsonError) -> Error {
        Error::Json(error)
    }
}

impl From<mime::Error> for Error {
    fn from(error: mime::Error) -> Error {
        Error::Mime(error)
    }
}

impl From<TomlError> for Error {
    fn from(error: TomlError) -> Error {
        Error::Toml(error)
    }
}
