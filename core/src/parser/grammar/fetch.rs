use command::Attribute::{
    self,
    Body,
    BodyPeek,
    BodySection,
    BodyStructure,
    Envelope,
    Flags,
    InternalDate,
    RFC822,
    UID
};
use command::FetchCommand;
use command::RFC822Attribute::{AllRFC822, HeaderRFC822, SizeRFC822, TextRFC822};
use mime::BodySectionType::{self, AllSection, MsgtextSection, PartSection};
use mime::Msgtext::{
    self,
    HeaderFieldsMsgtext,
    HeaderFieldsNotMsgtext,
    HeaderMsgtext,
    MimeMsgtext,
    TextMsgtext,
};
use parser::grammar::{astring, number, nz_number, whitespace};
use parser::grammar::sequence::sequence_set;
use std::ascii::AsciiExt;
use std::str;

named!(pub fetch<FetchCommand>,
    do_parse!(
        tag_no_case!("FETCH") >>
        whitespace >>
        set: sequence_set >>
        whitespace >>
        attrs: alt!(
            delimited!(
                tag!("("),
                do_parse!(
                    a: fetch_att                                >>
                    b: many0!(preceded!(whitespace, fetch_att)) >>

                    ({
                        let mut attrs = vec![a];
                        for attr in b {
                            attrs.push(attr);
                        }
                        attrs
                    })
                ),
                tag!(")")
            ) |
            map!(fetch_att, |attr| { vec![attr] }) |
            map!(tag_no_case!("ALL"), |_| { vec![Flags, InternalDate, RFC822(SizeRFC822), Envelope] }) |
            map!(tag_no_case!("FULL"), |_| { vec![Flags, InternalDate, RFC822(SizeRFC822), Envelope, Body] }) |
            map!(tag_no_case!("FAST"), |_| { vec![Flags, InternalDate, RFC822(SizeRFC822)] })
        ) >>

        ({ FetchCommand::new(set, attrs) })
    )
);

named!(fetch_att<Attribute>,
    alt!(
        complete!(tag_no_case!("ENVELOPE")) => { |_| { Envelope } } |
        complete!(tag_no_case!("FLAGS")) => { |_| { Flags } } |
        complete!(tag_no_case!("INTERNALDATE")) => { |_| { InternalDate } } |
        do_parse!(
            tag_no_case!("RFC822")                            >>
            sub_attr: opt!(alt!(
                tag!(".HEADER") => { |_| { HeaderRFC822 } } |
                tag!(".SIZE") => { |_| { SizeRFC822 } } |
                tag!(".TEST") => { |_| { TextRFC822 } }
            ))                                                >>

            ({ RFC822(sub_attr.unwrap_or(AllRFC822)) })
        ) |
        complete!(tag_no_case!("UID")) => { |_| { UID } } |
        preceded!(
            tag_no_case!("BODY"),
            alt!(
                do_parse!(
                    tag_no_case!(".PEEK")     >>
                    section: section          >>
                    octets: opt!(octet_range) >>

                    ({ BodyPeek(section, octets) })
                ) |
                do_parse!(
                    section: section          >>
                    octets: opt!(octet_range) >>

                    ({ BodySection(section, octets) })
                ) |
                do_parse!(
                    sub_attr: opt!(tag!("STRUCTURE")) >>

                    ({
                        if sub_attr.is_some() {
                            BodyStructure
                        } else {
                            Body
                        }
                    })
                )
            )
        )
    )
);

named!(octet_range<(usize, usize)>,
    delimited!(
        tag!("<"),
        do_parse!(
            first_octet: number   >>
            tag!(".")             >>
            last_octet: nz_number >>

            ((first_octet, last_octet))
        ),
        tag!(">")
    )
);

/* Section parsing */

named!(section<BodySectionType>,
    delimited!(
        tag!("["),
        map!(
            opt!(section_spec),
            |v: Option<BodySectionType>| { v.unwrap_or(AllSection) }
        ),
        tag!("]")
    )
);

named!(section_spec<BodySectionType>,
    alt!(
        section_msgtext => { |v| { MsgtextSection(v) } } |
        do_parse!(
            a: section_part                             >>
            b: opt!(preceded!(tag!("."), section_text)) >>

            ({ PartSection(a, b) })
        )
    )
);

// Top-level or MESSAGE/RFC822 part
named!(section_msgtext<Msgtext>,
    alt!(
        complete!(do_parse!(
            tag_no_case!("HEADER.FIELDS")   >>
            not: opt!(tag_no_case!(".NOT")) >>
            whitespace                      >>
            headers: header_list            >>

            ({
                if not.is_some() {
                    HeaderFieldsNotMsgtext(headers)
                } else {
                    HeaderFieldsMsgtext(headers)
                }
            })
        )) |
        complete!(tag_no_case!("HEADER")) => { |_| { HeaderMsgtext } } |
        tag_no_case!("TEXT") => { |_| { TextMsgtext } }
    )
);

named!(header_list<Vec<String>>,
    delimited!(
        tag!("("),
        separated_nonempty_list!(whitespace, header_fld_name),
        tag!(")")
    )
);

named!(header_fld_name<String>,
    map!(
        map_res!(astring, str::from_utf8),
        AsciiExt::to_ascii_uppercase
    )
);

// Body part nesting
named!(section_part<Vec<usize>>,
    separated_nonempty_list!(tag!("."), nz_number)
);

// Text other than actual body part (headers, etc.)
named!(section_text<Msgtext>,
    alt!(
        section_msgtext |
        tag_no_case!("MIME") => { |_| { MimeMsgtext } }
    )
);

#[cfg(test)]
mod tests {
    use command::Attribute::{
        Body,
        BodyPeek,
        BodySection,
        BodyStructure,
        Envelope,
        Flags,
        InternalDate,
        RFC822,
    };
    use command::FetchCommand;
    use mime::BodySectionType::{
        AllSection,
        MsgtextSection,
        PartSection
    };
    use mime::Msgtext::{
        HeaderMsgtext,
        HeaderFieldsMsgtext,
        HeaderFieldsNotMsgtext,
        MimeMsgtext,
        TextMsgtext
    };
    use command::RFC822Attribute::{
        AllRFC822,
        HeaderRFC822,
        SizeRFC822,
    };
    use command::sequence_set::SequenceItem::{
        Number,
        Range,
        Wildcard
    };
    use nom::ErrorKind::{Alt, OneOf, Tag};
    use nom::Needed::Size;
    use nom::IResult::{Done, Error, Incomplete};
    use super::{
        fetch,
        fetch_att,
        header_fld_name,
        header_list,
        octet_range,
        section,
        section_msgtext,
        section_part,
        section_spec,
        section_text,
    };

    #[test]
    fn test_fetch() {
        assert_eq!(fetch(b""), Incomplete(Size(5)));
        assert_eq!(fetch(b"FETCH *:3,6 (FLAGS RFC822)"), Done(&b""[..],
            FetchCommand::new(vec![Range(Box::new(Wildcard), Box::new(Number(3))), Number(6)], vec![Flags, RFC822(AllRFC822)])
        ));
        assert_eq!(fetch(b"FETCH * FLAGS"), Done(&b""[..],
            FetchCommand::new(vec![Wildcard], vec![Flags])
        ));
        assert_eq!(fetch(b"FETCH * ALL"), Done(&b""[..],
            FetchCommand::new(vec![Wildcard], vec![Flags, InternalDate, RFC822(SizeRFC822), Envelope])
        ));
        assert_eq!(fetch(b"FETCH * FULL"), Done(&b""[..],
            FetchCommand::new(vec![Wildcard], vec![Flags, InternalDate, RFC822(SizeRFC822), Envelope, Body])
        ));
        assert_eq!(fetch(b"FETCH * FAST"), Done(&b""[..],
            FetchCommand::new(vec![Wildcard], vec![Flags, InternalDate, RFC822(SizeRFC822)])
        ));
        assert_eq!(
            fetch(b"FETCH 4,5:3,* (FLAGS RFC822 BODY.PEEK[43.65.HEADER.FIELDS.NOT (abc \"def\" {2}\r\nde)]<4.2>)"),
            Done(&b""[..],
                FetchCommand::new(
                    vec![Number(4), Range(Box::new(Number(5)), Box::new(Number(3))), Wildcard],
                    vec![
                        Flags,
                        RFC822(AllRFC822),
                        BodyPeek(
                            PartSection(
                                vec![43, 65],
                                Some(HeaderFieldsNotMsgtext(vec![
                                    "ABC".to_owned(),
                                    "DEF".to_owned(),
                                    "DE".to_owned()
                                ]))
                            ),
                            Some((4, 2))
                        )
                    ]
                )
            )
        );
    }

    #[test]
    fn test_fetch_case_insensitivity() {
        assert_eq!(
            fetch(b"FETCH * (FLAGS BODY[4.2.2.2.HEADER.FIELDS.NOT (DATE FROM)]<400.10000>)"),
            fetch(b"fetch * (flags body[4.2.2.2.header.fields.not (date from)]<400.10000>)")
        );
    }

    #[test]
    fn test_fetch_att() {
        assert_eq!(fetch_att(b""), Incomplete(Size(6)));
        assert_eq!(fetch_att(b"envelope"), Done(&b""[..], Envelope));
        assert_eq!(fetch_att(b"FLAGS"), Done(&b""[..], Flags));
        assert_eq!(fetch_att(b"RFC822 "), Done(&b" "[..], RFC822(AllRFC822)));
        assert_eq!(fetch_att(b"RFC822.HEADER"), Done(&b""[..], RFC822(HeaderRFC822)));
        assert_eq!(fetch_att(b"BODY "), Done(&b" "[..],
            Body
        ));
        assert_eq!(fetch_att(b"BODYSTRUCTURE"), Done(&b""[..],
            BodyStructure
        ));
        assert_eq!(fetch_att(b"BODY.PEEK[] "), Done(&b" "[..],
            BodyPeek(AllSection, None)
        ));
        assert_eq!(fetch_att(b"BODY.PEEK[]<1.2>"), Done(&b""[..],
            BodyPeek(AllSection, Some((1, 2)))
        ));
        assert_eq!(fetch_att(b"BODY[TEXT]<1.2>"), Done(&b""[..],
            BodySection(MsgtextSection(TextMsgtext), Some((1, 2)))
        ));
    }

    #[test]
    fn test_octet_range() {
        assert_eq!(octet_range(b""), Incomplete(Size(1)));
        assert_eq!(octet_range(b"<0.0>"), Error(OneOf));
        assert_eq!(octet_range(b"<100.200>"), Done(&b""[..], (100, 200)));
    }

    #[test]
    fn test_section() {
        assert_eq!(section(b""), Incomplete(Size(1)));
        assert_eq!(section(b"[]"), Done(&b""[..], AllSection));
        assert_eq!(section(b"[1.2.3.HEADER.FIELDS (abc def)]"),
            Done(
                &b""[..],
                PartSection(
                    vec![1, 2, 3],
                    Some(HeaderFieldsMsgtext(vec!["ABC".to_string(), "DEF".to_string()]))
                )
            )
        );
    }

    #[test]
    fn test_section_spec() {
        assert_eq!(section_spec(b""), Incomplete(Size(4)));
        assert_eq!(section_spec(b"invalid"), Error(Alt));
        assert_eq!(section_spec(b"HEADER"), Done(&b""[..], MsgtextSection(HeaderMsgtext)));
        assert_eq!(section_spec(b"1.2.3.MIME"), Done(&b""[..], PartSection(vec![1, 2, 3], Some(MimeMsgtext))));
        assert_eq!(section_spec(b"1.2.3.HEADER.FIELDS (abc def)"),
            Done(
                &b""[..],
                PartSection(
                    vec![1, 2, 3],
                    Some(HeaderFieldsMsgtext(vec!["ABC".to_string(), "DEF".to_string()]))
                )
            )
        );
    }

    #[test]
    fn test_section_msgtext() {
        assert_eq!(section_msgtext(b""), Incomplete(Size(4)));
        assert_eq!(section_msgtext(b"invalid"), Error(Alt));
        assert_eq!(section_msgtext(b"header"), Done(&b""[..], HeaderMsgtext));
        assert_eq!(section_msgtext(b"HEADER"), Done(&b""[..], HeaderMsgtext));
        assert_eq!(section_msgtext(b"text"), Done(&b""[..], TextMsgtext));
        assert_eq!(section_msgtext(b"HEADER.FIELDS (abc def)"),
            Done(
                &b""[..],
                HeaderFieldsMsgtext(vec!["ABC".to_string(), "DEF".to_string()])
            )
        );
        assert_eq!(section_msgtext(b"HEADER.FIELDS.NOT (abc def)"),
            Done(
                &b""[..],
                HeaderFieldsNotMsgtext(vec!["ABC".to_string(), "DEF".to_string()])
            )
        );
    }

    #[test]
    fn test_header_list() {
        assert_eq!(header_list(b""), Incomplete(Size(1)));
        assert_eq!(header_list(b"(abc\ndef)"), Error(Tag));
        assert_eq!(header_list(b"(abc)\ndef"), Done(&b"\ndef"[..], vec!["ABC".to_string()]));
        assert_eq!(header_list(b"(abc def ghi jkl)"),
            Done(&b""[..], vec!["ABC".to_string(), "DEF".to_string(), "GHI".to_string(), "JKL".to_string()])
        );
        assert_eq!(header_list(b"({3}\r\ndef)"),
            Done(&b""[..], vec!["DEF".to_string()])
        );
    }

    #[test]
    fn test_header_fld_name() {
        assert_eq!(header_fld_name(b""), Incomplete(Size(1)));
        assert_eq!(header_fld_name(b"abc123\ndef456"), Done(&b"\ndef456"[..], "ABC123".to_string()));
        assert_eq!(header_fld_name(b"{3}\r\ndef"), Done(&b""[..], "DEF".to_string()));
    }

    #[test]
    fn test_section_part() {
        assert_eq!(section_part(b""), Incomplete(Size(1)));
        assert_eq!(section_part(b"0"), Error(OneOf));
        assert_eq!(section_part(b"1"), Incomplete(Size(2)));
        assert_eq!(section_part(b"1 "), Done(&b" "[..], vec![1]));
        assert_eq!(section_part(b"1.2.3 "), Done(&b" "[..], vec![1, 2, 3]));
    }

    #[test]
    fn test_section_text() {
        assert_eq!(section_text(b""), Incomplete(Size(4)));
        assert_eq!(section_text(b"MIME"), Done(&b""[..], MimeMsgtext));
        assert_eq!(section_text(b"invalid"), Error(Alt));
        assert_eq!(section_text(b"HEADER.FIELDS (abc def)"),
            Done(
                &b""[..],
                HeaderFieldsMsgtext(vec!["ABC".to_string(), "DEF".to_string()])
            )
        );
    }
}

#[cfg(all(feature = "unstable", test))]
mod bench {
    extern crate test;

    use command::Attribute::{BodyPeek, Flags, RFC822};
    use command::FetchCommand;
    use command::RFC822Attribute::AllRFC822;
    use command::sequence_set::SequenceItem::{Number, Range, Wildcard};
    use mime::BodySectionType::PartSection;
    use mime::Msgtext::HeaderFieldsNotMsgtext;
    use nom::IResult::Done;
    use self::test::Bencher;
    use super::fetch;

    #[bench]
    fn bench_fetch(b: &mut Bencher) {
        const FETCH_STR: &'static str = "FETCH 4,5:3,* (FLAGS RFC822 BODY.PEEK[43.65.HEADER.FIELDS.NOT (a \"abc\")]<4.2>)";

        b.iter(|| {
            assert_eq!(fetch(FETCH_STR.as_bytes()), Done(&b""[..],
                FetchCommand::new(
                    vec![Number(4), Range(Box::new(Number(5)), Box::new(Number(3))), Wildcard],
                    vec![
                        Flags,
                        RFC822(AllRFC822),
                        BodyPeek(
                            PartSection(
                                vec![43, 65],
                                Some(HeaderFieldsNotMsgtext(vec![
                                    "A".to_owned(),
                                    "ABC".to_owned(),
                                ]))
                            ),
                            Some((4, 2))
                        )
                    ]
                )
            ));
        });
    }
}
