use rustc_serialize::json::DecoderError;
use std::io;

use mime;

/// An enum of all error kinds.
#[derive(Debug)]
pub enum ErrorKind {
    InternalIoError(io::Error),
    MimeError(mime::Error),
    MessageDecodeError,
    ImapStateError,
    SerializationError(DecoderError)
}

/// Represents a SEGIMAP error.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    desc: &'static str,
    detail: Option<String>
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
