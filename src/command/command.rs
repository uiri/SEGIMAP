use command::sequence_set::SequenceItem;

#[deriving(PartialEq, Show)]
pub enum CommandType {
    Fetch
}

// TODO: Sort these in alphabetical order.
#[deriving(PartialEq, Show)]
pub enum Attribute {
    Envelope,
    Flags,
    InternalDate,
    RFC822(RFC822Attribute),
    Body,
    BodyStructure,
    UID,
    /*
    BODY section ("<" number "." nz_number ">")?,
    BODYPEEK section ("<" number "." nz_number ">")?
    */
}

#[deriving(PartialEq, Show)]
pub enum RFC822Attribute {
    All,
    Header,
    Size,
    Text
}

/*#[deriving(PartialEq, Show)]
pub enum BodyAttribute {
    BodyAll,
    Size,
    Text,
    Plain
}*/

#[deriving(PartialEq, Show)]
pub struct Command {
    command_type: CommandType,
    sequence_set: Vec<SequenceItem>,
    attributes: Vec<Attribute>
}

impl Command {
    pub fn new(
            command_type: CommandType,
            sequence_set: Vec<SequenceItem>,
            attributes: Vec<Attribute>) -> Command {
        Command {
            command_type: command_type,
            sequence_set: sequence_set,
            attributes: attributes
        }
    }
}
