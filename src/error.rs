use std::io::IoError;

use serialize::json::DecoderError;

/// An enum of all error kinds.
#[deriving(Show)]
pub enum ErrorKind {
    InternalIoError(IoError),
    NoSuchMessageError,
    MessageDecodeError,
    ImapStateError,
    SerializationError(DecoderError)
}

/// Represents a SEGIMAP error.
#[deriving(Show)]
pub struct Error {
    pub kind: ErrorKind,
    pub desc: &'static str,
    pub detail: Option<String>
}

/// Generic result type.
pub type ImapResult<T> = Result<T, Error>;

impl Error {
    pub fn simple(kind: ErrorKind, desc: &'static str) -> Error {
        Error {
            kind: kind,
            desc: desc,
            detail: None
        }
    }
}
