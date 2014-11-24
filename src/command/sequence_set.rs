#[deriving(Clone, PartialEq, Show)]
pub enum SequenceItem {
    Number(uint),
    Range(Box<SequenceItem>, Box<SequenceItem>),
    Wildcard
}
