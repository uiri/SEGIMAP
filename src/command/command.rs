use command::sequence_set::SequenceItem;

/// Only the Fetch command is complicated enough to require dedicated command
/// parsing
#[derive(PartialEq, Debug)]
pub enum CommandType {
    Fetch
}

/// The different Attributes which a Fetch command may request.
#[derive(PartialEq, Debug)]
pub enum Attribute {
    Body,
    BodyPeek(BodySectionType, Option<(usize, usize)>),
    BodySection(BodySectionType, Option<(usize, usize)>),
    BodyStructure,
    Envelope,
    Flags,
    InternalDate,
    RFC822(RFC822Attribute),
    UID
}

/// Attributes defined as part of any electronic mail message
#[derive(PartialEq, Debug)]
pub enum RFC822Attribute {
    AllRFC822,
    HeaderRFC822,
    SizeRFC822,
    TextRFC822
}

#[derive(PartialEq, Debug)]
pub enum BodySectionType {
    AllSection,
    MsgtextSection(Msgtext),
    PartSection(Vec<usize>, Option<Msgtext>)
}

#[derive(PartialEq, Debug)]
pub enum Msgtext {
    HeaderMsgtext,
    HeaderFieldsMsgtext(Vec<String>),
    HeaderFieldsNotMsgtext(Vec<String>),
    TextMsgtext,
    MimeMsgtext
}

/// This represents a Fetch command;
/// It has a list of message ids (either UIDs or indexes into the folder's list
/// of messages)
/// It has a list of message attributes which are being requested.
#[derive(PartialEq, Debug)]
pub struct Command {
    command_type: CommandType,
    pub sequence_set: Vec<SequenceItem>,
    pub attributes: Vec<Attribute>
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
