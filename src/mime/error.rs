use std::io;

/// An enum of all error kinds.
#[derive(Debug)]
pub enum ErrorKind {
    InternalIoError(io::Error),
    MessageDecodeError,
}

/// Represents a SEGIMAP error.
#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub desc: &'static str,
    pub detail: Option<String>
}

/// Generic result type.
pub type MimeResult<T> = Result<T, Error>;

impl Error {
    pub fn new(kind: ErrorKind, desc: &'static str) -> Error {
        Error {
            kind: kind,
            desc: desc,
            detail: None
        }
    }
}
