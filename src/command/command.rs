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
    Body(BodySection, Option<(uint, uint)>),
    BodyPeek(BodySection, Option<(uint, uint)>),
    BodyStructure,
    UID
}

// TODO: Remove the suffix from this enum when enum namespacing is available.
#[deriving(PartialEq, Show)]
pub enum RFC822Attribute {
    AllRFC822,
    HeaderRFC822,
    SizeRFC822,
    TextRFC822
}

// TODO: Remove the suffix from this enum when enum namespacing is available.
#[deriving(PartialEq, Show)]
pub enum BodySection {
    AllSection,
    MsgtextSection(Msgtext),
    PartSection(Vec<uint>, Option<Msgtext>)
}

#[deriving(PartialEq, Show)]
pub enum Msgtext {
    HeaderMsgtext,
    HeaderFieldsMsgtext(Vec<String>),
    HeaderFieldsNotMsgtext(Vec<String>),
    TextMsgtext, // This is for the msgtext "TEXT" field
    MimeMsgtext
}

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
