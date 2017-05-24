pub mod sequence_set;
pub mod store;
pub mod fetch;

use command::sequence_set::SequenceItem;

use mime::BodySectionType;

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

/// This represents a Fetch command;
/// It has a list of message ids (either UIDs or indexes into the folder's list
/// of messages)
/// It has a list of message attributes which are being requested.
#[derive(PartialEq, Debug)]
pub struct FetchCommand {
    pub sequence_set: Vec<SequenceItem>,
    pub attributes: Vec<Attribute>
}

impl FetchCommand {
    pub fn new(sequence_set: Vec<SequenceItem>, attributes: Vec<Attribute>)
               -> FetchCommand {
        FetchCommand {
            sequence_set: sequence_set,
            attributes: attributes
        }
    }
}
