use command::Command;

mod error;
mod grammar;

pub use self::error::Error as ParserError;
pub use self::error::Result as ParserResult;

pub fn fetch(input: &[u8]) -> ParserResult<Command> {
    use nom::IResult::{Done, Error, Incomplete};

    match self::grammar::fetch(input) {
        Done(_, v) => Ok(v),
        Incomplete(_) => Err(ParserError::Incomplete),
        Error(err) => Err(err).map_err(ParserError::from),
    }
}
