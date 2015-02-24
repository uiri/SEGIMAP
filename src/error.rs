use std::io::IoError;

use rustc_serialize::json::DecoderError;

/// An enum of all error kinds.
#[derive(Show)]
pub enum ErrorKind {
    InternalIoError(IoError),
    MessageDecodeError,
    ImapStateError,
    SerializationError(DecoderError)
}

/// Represents a SEGIMAP error.
#[derive(Show)]
pub struct Error {
    pub kind: ErrorKind,
    pub desc: &'static str,
    pub detail: Option<String>
}

/// Generic result type.
pub type ImapResult<T> = Result<T, Error>;

impl Error {
    pub fn new(kind: ErrorKind, desc: &'static str) -> Error {
        Error {
            kind: kind,
            desc: desc,
            detail: None
        }
    }
}
