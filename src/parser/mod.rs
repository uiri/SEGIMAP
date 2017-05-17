use command::Command;

mod grammar;

pub fn fetch(input: &[u8]) -> Result<Command, ()> {
    use nom::IResult::{Done, Error, Incomplete};

    match self::grammar::fetch(input) {
        Done(_, v) => Ok(v),
        // TODO: handle the error and incomplete cases properly (possibly via a
        // custom enum?)
        Error(_) | Incomplete(_) => Err(()),
    }
}
