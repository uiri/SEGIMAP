use mime;
use serde_json::Error as JsonError;
use std::io;
use std::result::Result as StdResult;
use thiserror::Error;
use toml::ser::Error as TomlError;

/// A convenient alias type for results for `segimap`.
pub type ImapResult<T> = StdResult<T, Error>;

/// Represents errors which occur during SEGIMAP operation.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Not in selected state.")]
    InvalidImapState,
    /// An internal `std::io` error.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An internal `serde_json` error which occurs when serializing or
    /// deserializing JSON data.
    #[error(transparent)]
    Json(#[from] JsonError),
    /// An error which occurs when attempting to read the UID for a message.
    #[error("An error occured while decoding the UID for a message.")]
    MessageUidDecode,
    /// An error which occurs when a Maildir message has a bad filename
    #[error("An error occured while parsing message information from its filename")]
    MessageBadFilename,
    /// An internal `mime` error.
    #[error(transparent)]
    Mime(#[from] mime::Error),
    /// An internal `toml` error which occurs when serializing or deserializing
    /// TOML data.
    #[error(transparent)]
    Toml(#[from] TomlError),
}
