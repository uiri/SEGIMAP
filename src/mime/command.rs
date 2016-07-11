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
