// FIXME: move the nom parsing functions into the grammar module, and only
// expose the top-level API.

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
use command::Command;
use command::CommandType::Fetch;
use command::RFC822Attribute::{AllRFC822, HeaderRFC822, SizeRFC822, TextRFC822};
use command::sequence_set::SequenceItem;
use nom::{crlf, Slice};
use mime::BodySectionType::{self, AllSection, MsgtextSection, PartSection};
use mime::Msgtext::{
    self,
    HeaderFieldsMsgtext,
    HeaderFieldsNotMsgtext,
    HeaderMsgtext,
    MimeMsgtext,
    TextMsgtext,
};
use std::ascii::AsciiExt;
use std::collections::HashSet;
use std::str;

pub use self::grammar::fetch;
pub use self::grammar::ParseError;

// grammar.rustpeg contains the parsing expression grammar needed in order to
// parse FETCH commands.
peg_file! grammar("grammar.rustpeg");

const DIGITS: &'static str = "0123456789";
const NZ_DIGITS: &'static str = "123456789";

fn is_astring_char(chr: u8) -> bool {
    // TODO: perhaps take `char` instead, avoiding this cast?
    let chr = chr as char;
    is_atom_char(chr) || is_resp_specials(chr)
}

// any CHAR except atom-specials
fn is_atom_char(chr: char) -> bool {
    !is_atom_specials(chr)
}

fn is_atom_specials(chr: char) -> bool {
    chr == '(' || chr == ')' || chr == '{' || is_sp(chr) || is_ctl(chr) ||
        is_list_wildcards(chr) || is_quoted_specials(chr) ||
        is_resp_specials(chr)
}

fn is_sp(chr: char) -> bool {
    chr == ' '
}

fn is_ctl(chr: char) -> bool {
    (chr >= '\x00' && chr <= '\x1F') || chr == '\x7F'
}

fn is_list_wildcards(chr: char) -> bool {
    chr == '%' || chr == '*'
}

fn is_quoted_specials(chr: char) -> bool {
    is_dquote(chr) || chr == '\\'
}

fn is_dquote(chr: char) -> bool {
    chr == '"'
}

fn is_resp_specials(chr: char) -> bool {
    chr == ']'
}

// an ASCII digit (%x30-%x39)
fn is_digit(chr: u8) -> bool {
    // TODO: perhaps take `char` instead, avoiding this cast?
    let chr = chr as char;
    chr >= '0' && chr <= '9'
}

// any TEXT_CHAR except quoted_specials
fn is_quoted_char(chr: u8) -> bool {
    // TODO: perhaps take `char` instead, avoiding this cast?
    let chr = chr as char;
    !is_quoted_specials(chr) && is_text_char(chr)
}

// any CHAR except CR and LF
fn is_text_char(chr: char) -> bool {
    !is_eol_char(chr)
}

// a CR or LF CHAR
fn is_eol_char(chr: char) -> bool {
    chr == '\r' || chr == '\n'
}

// FIXME: remove the wrapper functions once the migration to nom has been
// completed.
pub fn sequence_set_wrapper(parse_str: &str) -> Result<Vec<SequenceItem>, ParseError> {
    use nom::IResult::{Done, Error, Incomplete};

    match sequence_set(parse_str.as_bytes()) {
        Done(_, v) => Ok(v),
        Error(_) |
            Incomplete(_) => Err(ParseError {
                column: 0,
                expected: HashSet::new(),
                line: 0,
                offset: 0,
            }),
    }
}

// TODO: remove `_nom` suffix
named!(fetch_nom<Command>,
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
                        for attr in b.into_iter() {
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

        ({ Command::new(Fetch, set, attrs) })
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

// TODO: confirm this only needs to work for ASCII
named!(header_fld_name<String>,
    map!(
        map_res!(astring, str::from_utf8),
        AsciiExt::to_ascii_uppercase
    )
);

// Body part nesting
// NOTE: currently returns `Incomplete` if the provided string does not contain
// a non-matching byte. This is useful for streaming parsers which may be
// awaiting further input.
// TODO: decide if this should be streaming-compatible. If not, add a
// `complete!` invocation.
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

/* String parsing */

named!(astring<&[u8], &[u8]>, alt!(take_while1!(is_astring_char) | string));

named!(string<&[u8], &[u8]>, alt!(quoted | literal));

named!(quoted<&[u8], &[u8]>,
    delimited!(
        tag!("\""),
        recognize!(
            many0!(
                alt!(
                    take_while1!(is_quoted_char) |
                    preceded!(tag!("\\"), alt!(tag!("\"") | tag!("\\")))
                )
            )
        ),
        tag!("\"")
    )
);

named!(literal<&[u8], &[u8]>,
    do_parse!(
        // The "number" is used to indicate the number of octets.
        number: terminated!(delimited!(tag!("{"), number, tag!("}")), crlf) >>
        v: recognize!(
            count!(
                do_parse!(
                    // any OCTET except NUL ('%x00')
                    not!(char!('\x00')) >>
                    chr: take!(1) >>

                    (chr)
                ),
                number
            )
        ) >>

        (v)
    )
);

/* Sequence item and set rules */

named!(sequence_set<Vec<SequenceItem>>,
    do_parse!(
        a: alt!(
            complete!(seq_range) |
            seq_number
        )                                             >>
        b: many0!(preceded!(tag!(","), sequence_set)) >>

        ({
            let mut seq = vec![a];
            // TODO: implement this with iterator combinators instead.
            for set in b.into_iter() {
                for elem in set.into_iter() {
                    seq.push(elem);
                }
            }
            seq
        })
    )
);

named!(seq_range<SequenceItem>,
    do_parse!(
        a: seq_number >>
        tag!(":")     >>
        b: seq_number >>

        (SequenceItem::Range(Box::new(a), Box::new(b)))
    )
);

named!(seq_number<SequenceItem>,
    alt!(
        nz_number => { |num: usize| SequenceItem::Number(num) } |
        tag!("*") => { |_| SequenceItem::Wildcard }
    )
);

/* RFC 3501 Boilerplate */

/// Recognizes an non-zero unsigned 32-bit integer.
// (0 < n < 4,294,967,296)
named!(number<usize>, flat_map!(take_while1!(is_digit), parse_to!(usize)));

/// Recognizes a non-zero unsigned 32-bit integer.
// (0 < n < 4,294,967,296)
named!(nz_number<usize>,
    flat_map!(
        recognize!(
            tuple!(
                digit_nz,
                many0!(one_of!(DIGITS))
            )
        ),
        parse_to!(usize)
    )
);

/// Recognizes exactly one non-zero numerical character: 1-9.
// digit-nz = %x31-39
//    ; 1-9
named!(digit_nz<char>, one_of!(NZ_DIGITS));

/// Recognizes exactly one ASCII whitespace.
named!(whitespace<char>, char!(' '));

// Tests for the parsed FETCH commands follow
#[cfg(test)]
mod tests {
    use super::sequence_set_wrapper as sequence_set;
    use super::grammar::fetch;
    use command::Attribute::{
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
    use command::Command;
    use command::CommandType::Fetch;
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
        TextRFC822
    };
    use command::sequence_set::SequenceItem::{
        Number,
        Range,
        Wildcard
    };
    use nom::ErrorKind::{Alt, Char, Count, OneOf, TakeWhile1, MapOpt, Tag};
    use nom::Needed::Size;
    use nom::IResult::{Done, Error, Incomplete};
    use super::{
        astring,
        digit_nz,
        fetch_att,
        fetch_nom,
        header_fld_name,
        header_list,
        literal,
        number,
        nz_number,
        octet_range,
        quoted,
        section,
        section_msgtext,
        section_part,
        section_spec,
        section_text,
        seq_number,
        seq_range,
        string,
        whitespace
    };
    use test::Bencher;

    const FETCH_STR: &'static str = "FETCH 4,5:3,* (FLAGS RFC822 BODY.PEEK[43.65.HEADER.FIELDS.NOT (a \"abc\")]<4.2>)";

    #[bench]
    fn bench_fetch_nom(b: &mut Bencher) {
        b.iter(|| {
            assert_eq!(fetch_nom(FETCH_STR.as_bytes()), Done(&b""[..],
                Command::new(
                    Fetch,
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

    #[bench]
    fn bench_fetch_peg(b: &mut Bencher) {
        b.iter(|| {
            assert_eq!(fetch(FETCH_STR), Ok(
                Command::new(
                    Fetch,
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

    #[test]
    fn test_fetch_nom() {
        assert_eq!(fetch_nom(b""), Incomplete(Size(5)));
        assert_eq!(fetch_nom(b"FETCH *:3,6 (FLAGS RFC822)"), Done(&b""[..],
            Command::new(Fetch, vec![Range(Box::new(Wildcard), Box::new(Number(3))), Number(6)], vec![Flags, RFC822(AllRFC822)])
        ));
        assert_eq!(fetch_nom(b"FETCH * FLAGS"), Done(&b""[..],
            Command::new(Fetch, vec![Wildcard], vec![Flags])
        ));
        assert_eq!(fetch_nom(b"FETCH * ALL"), Done(&b""[..],
            Command::new(Fetch, vec![Wildcard], vec![Flags, InternalDate, RFC822(SizeRFC822), Envelope])
        ));
        assert_eq!(fetch_nom(b"FETCH * FULL"), Done(&b""[..],
            Command::new(Fetch, vec![Wildcard], vec![Flags, InternalDate, RFC822(SizeRFC822), Envelope, Body])
        ));
        assert_eq!(fetch_nom(b"FETCH * FAST"), Done(&b""[..],
            Command::new(Fetch, vec![Wildcard], vec![Flags, InternalDate, RFC822(SizeRFC822)])
        ));
        assert_eq!(
            fetch_nom(b"FETCH 4,5:3,* (FLAGS RFC822 BODY.PEEK[43.65.HEADER.FIELDS.NOT (abc \"def\" {2}\r\nde)]<4.2>)"),
            Done(&b""[..],
                Command::new(
                    Fetch,
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

    #[test]
    fn test_astring() {
        assert_eq!(astring(b""), Incomplete(Size(1)));
        assert_eq!(astring(b"("), Error(Alt));
        assert_eq!(astring(b"]"), Done(&b""[..], &b"]"[..]));
        assert_eq!(astring(b"a"), Done(&b""[..], &b"a"[..]));
        assert_eq!(astring(b"\"test\"abc"), Done(&b"abc"[..], &b"test"[..]));
        assert_eq!(astring(b"\"test\""), Done(&b""[..], &b"test"[..]));
        assert_eq!(astring(b"{3}\r\nabc\x00"), Done(&b"\x00"[..], &b"abc"[..]));
    }

    #[test]
    fn test_string() {
        assert_eq!(string(b"\"test\""), Done(&b""[..], &b"test"[..]));
        assert_eq!(string(b"{2}\r\nab\x00"), Done(&b"\x00"[..], &b"ab"[..]));
    }

    #[test]
    fn test_quoted() {
        assert_eq!(quoted(b""), Incomplete(Size(1)));
        assert_eq!(quoted(b"\"\""), Done(&b""[..], &b""[..]));
        assert_eq!(quoted(b"\"a\""), Done(&b""[..], &b"a"[..]));
        assert_eq!(quoted(b"\"\\\""), Incomplete(Size(4)));
        assert_eq!(quoted(b"\"\\\"\""), Done(&b""[..], &b"\\\""[..]));
        assert_eq!(quoted(b"\"\\\\\""), Done(&b""[..], &b"\\\\"[..]));
        assert_eq!(quoted(b"\"\r\""), Error(Tag));
        assert_eq!(quoted(b"\"\t\""), Done(&b""[..], &b"\t"[..]));
    }

    #[test]
    fn test_literal() {
        assert_eq!(literal(b""), Incomplete(Size(1)));
        assert_eq!(literal(b"{1}\r\nabc"), Done(&b"bc"[..], &b"a"[..]));
        assert_eq!(literal(b"{0}\r\n"), Done(&b""[..], &b""[..]));
        assert_eq!(literal(b"{1}\r\na"), Done(&b""[..], &b"a"[..]));
        assert_eq!(literal(b"{2}\r\na"), Incomplete(Size(7)));
        assert_eq!(literal(b"{2}\r\na\x00a"), Error(Count));
    }

    #[test]
    // TODO; remove the `_nom` suffix from all the functions.
    fn test_sequence_set_nom() {
        use super::sequence_set as sequence_set_nom;

        assert_eq!(sequence_set_nom(b""), Incomplete(Size(1)));
        assert_eq!(sequence_set_nom(b"a"), Error(Alt));
        assert_eq!(sequence_set_nom(b"0"), Error(Alt));
        assert_eq!(sequence_set_nom(b"*"), Done(&b""[..], vec![Wildcard]));
        assert_eq!(sequence_set_nom(b"1"), Done(&b""[..], vec![Number(1)]));
        // TODO: determine if this should parse correctly as `(":0", 1)`, or
        // return an error because "1:0" is not a valid range (i.e., are we OK
        // treating this as a `seq-number` with trailing text?).
        assert_eq!(sequence_set_nom(b"1:0"), Done(&b":0"[..], vec![Number(1)]));
        assert_eq!(sequence_set_nom(b"1:1"), Done(&b""[..], vec![
            Range(Box::new(Number(1)), Box::new(Number(1)))
        ]));
        assert_eq!(sequence_set_nom(b"2:4a"), Done(&b"a"[..], vec![
            Range(Box::new(Number(2)), Box::new(Number(4)))
        ]));
        assert_eq!(sequence_set_nom(b"*:3, 4:4"), Done(&b", 4:4"[..], vec![
            Range(Box::new(Wildcard), Box::new(Number(3)))
        ]));
        assert_eq!(sequence_set_nom(b"*:3,4:4"), Done(&b""[..], vec![
            Range(Box::new(Wildcard), Box::new(Number(3))),
            Range(Box::new(Number(4)), Box::new(Number(4)))
        ]));
    }

    #[test]
    fn test_seq_range() {
        assert_eq!(seq_range(b""), Incomplete(Size(1)));
        assert_eq!(seq_range(b"a"), Error(Alt));
        assert_eq!(seq_range(b"0"), Error(Alt));
        assert_eq!(seq_range(b"1:1"), Done(&b""[..], Range(Box::new(Number(1)), Box::new(Number(1)))));
        assert_eq!(seq_range(b"2:4a"), Done(&b"a"[..], Range(Box::new(Number(2)), Box::new(Number(4)))));
        assert_eq!(seq_range(b"*:3"), Done(&b""[..], Range(Box::new(Wildcard), Box::new(Number(3)))));
    }

    #[test]
    fn test_seq_number() {
        assert_eq!(seq_number(b""), Incomplete(Size(1)));
        assert_eq!(seq_number(b"a"), Error(Alt));
        assert_eq!(seq_number(b"0"), Error(Alt));
        assert_eq!(seq_number(b"100"), Done(&b""[..], Number(100)));
        assert_eq!(seq_number(b"*a"), Done(&b"a"[..], Wildcard));
    }

    #[test]
    fn test_number() {
        assert_eq!(number(b""), Incomplete(Size(1)));
        assert_eq!(number(b"a"), Error(TakeWhile1));
        assert_eq!(number(b"0"), Done(&b""[..], 0));
        assert_eq!(number(b"1"), Done(&b""[..], 1));
        assert_eq!(number(b"10"), Done(&b""[..], 10));
        assert_eq!(number(b"10a"), Done(&b"a"[..], 10));
        assert_eq!(number(b"4294967296"), Done(&b""[..], 4294967296));
        assert_eq!(number(b"100000000000000000000"), Error(MapOpt));
    }

    #[test]
    fn test_nz_number() {
        assert_eq!(nz_number(b""), Incomplete(Size(1)));
        assert_eq!(nz_number(b"a"), Error(OneOf));
        assert_eq!(nz_number(b"0"), Error(OneOf));
        assert_eq!(nz_number(b"1"), Done(&b""[..], 1));
        assert_eq!(nz_number(b"10"), Done(&b""[..], 10));
        assert_eq!(nz_number(b"10a"), Done(&b"a"[..], 10));
        assert_eq!(nz_number(b"4294967296"), Done(&b""[..], 4294967296));
        assert_eq!(nz_number(b"100000000000000000000"), Error(MapOpt));
    }

    #[test]
    fn test_digit_nz() {
        assert_eq!(digit_nz(b""), Incomplete(Size(1)));
        assert_eq!(digit_nz(b"a"), Error(OneOf));
        assert_eq!(digit_nz(b"1"), Done(&b""[..], '1'));
        assert_eq!(digit_nz(b"62"), Done(&b"2"[..], '6'));
    }

    #[test]
    fn test_whitespace() {
        assert_eq!(whitespace(b""), Incomplete(Size(1)));
        assert_eq!(whitespace(b"a"), Error(Char));
        assert_eq!(whitespace(b" "), Done(&b""[..], ' '));
        assert_eq!(whitespace(b"\t"), Error(Char));
    }

    #[test]
    fn test_invalid_sequences() {
        assert!(sequence_set("").is_err());
        assert!(sequence_set("a").is_err());
        assert!(sequence_set("0").is_err());
        assert!(sequence_set("a:*").is_err());
        assert!(sequence_set(":*").is_err());
        // TODO: temporarily disabled because previously these were treated as
        // an error case, but now nom parses it as a `seq-number` with trailing
        // text, instead of as an invalid `seq-range`.
        //assert!(sequence_set("1:").is_err());
        //assert!(sequence_set("1:0").is_err());
        assert!(sequence_set("0:1").is_err());
        assert!(sequence_set("4,5,6,").is_err());
    }

    #[test]
    fn test_sequence_num() {
        let seq = sequence_set("4324");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = vec![Number(4324)];
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_sequence_all() {
        let seq = sequence_set("*");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = vec![Wildcard];
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_sequence_ranges() {
        let seq = sequence_set("98:100");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = vec![Range(Box::new(Number(98)), Box::new(Number(100)))];
        assert_eq!(seq, expected);

        assert!(sequence_set("1:5").is_ok());
        assert!(sequence_set("21:44").is_ok());
    }

    #[test]
    fn test_sequence_range_all() {
        let seq = sequence_set("31:*");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = vec![Range(Box::new(Number(31)), Box::new(Wildcard))];
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_sequence_set() {
        let seq = sequence_set("1231,1342,12,98:104,16");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = vec![Number(1231), Number(1342), Number(12),
                            Range(Box::new(Number(98)), Box::new(Number(104))), Number(16)];
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_fetch_all() {
        let cmd = fetch("FETCH 1:5 ALL");
        assert!(cmd.is_ok());
        let cmd = cmd.unwrap();
        let expected = Command::new(
                Fetch,
                vec![Range(Box::new(Number(1)), Box::new(Number(5)))],
                vec![Flags, InternalDate, RFC822(SizeRFC822), Envelope]);
        assert_eq!(cmd, expected);
    }

    #[test]
    fn test_fetch_fast() {
        let cmd = fetch("FETCH 3,5 FAST");
        assert!(cmd.is_ok());
        let cmd = cmd.unwrap();
        let expected = Command::new(
                Fetch,
                vec![Number(3), Number(5)],
                vec![Flags, InternalDate, RFC822(SizeRFC822)]);
        assert_eq!(cmd, expected);
    }

    #[test]
    fn test_fetch_full() {
        let cmd = fetch("FeTCH 2:7 FULL");
        assert!(cmd.is_ok());
        let cmd = cmd.unwrap();
        let expected = Command::new(
                Fetch,
                vec![Range(Box::new(Number(2)), Box::new(Number(7)))],
                vec![Flags, InternalDate, RFC822(SizeRFC822), Envelope, Body]);
        assert_eq!(cmd, expected);
    }

    #[test]
    fn test_fetch_simple() {
        assert_eq!(fetch("FETCH * ENVELOPE").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![Envelope]));
        assert_eq!(fetch("FETCH * FLAGS").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![Flags]));
        assert_eq!(fetch("FETCH * INTERNALDATE").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![InternalDate]));
        assert_eq!(fetch("FETCH * UID").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![UID]));
    }

    #[test]
    fn test_fetch_rfc822() {
        assert_eq!(fetch("FETCH * RFC822").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![RFC822(AllRFC822)]));
        assert_eq!(fetch("FETCH * RFC822.HEADER").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![RFC822(HeaderRFC822)]));
        assert_eq!(fetch("FETCH * RFC822.SIZE").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![RFC822(SizeRFC822)]));
        assert_eq!(fetch("FETCH * RFC822.TEXT").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![RFC822(TextRFC822)]));
    }

    #[test]
    fn test_fetch_body() {
        assert_eq!(fetch("FETCH * BODY").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![Body]));
    }

    #[test]
    fn test_fetch_bodystructure() {
        assert_eq!(fetch("FETCH * BODYSTRUCTURE").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![BodyStructure]));
    }

    #[test]
    fn test_fetch_body_octets() {
        assert_eq!(fetch("FETCH 1,2 BODY[]<0.1>").unwrap(), Command::new(
                Fetch,
                vec![Number(1), Number(2)],
                vec![BodySection(AllSection, Some((0, 1)))]));
        assert_eq!(fetch("FETCH *:4 BODY[]<400.10000>").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(AllSection, Some((400, 10000)))]));
    }

    #[test]
    fn test_fetch_body_section() {
        assert_eq!(fetch("FETCH 1,2 BODY[]").unwrap(), Command::new(
                Fetch,
                vec![Number(1), Number(2)],
                vec![BodySection(AllSection, None)]));
        assert_eq!(fetch("FETCH *:4 BODY[HEADER]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(MsgtextSection(HeaderMsgtext), None)]));
        assert_eq!(fetch("FETCH *:4 BODY[TEXT]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(MsgtextSection(TextMsgtext), None)]));
        assert_eq!(fetch("FETCH *:4 BODY[1]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(PartSection(vec![1], None), None)]));
        assert_eq!(fetch("FETCH *:4 BODY[3]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(PartSection(vec![3], None), None)]));
        assert_eq!(fetch("FETCH *:4 BODY[3.HEADER]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(PartSection(vec![3], Some(HeaderMsgtext)),
                                 None)]));
        assert_eq!(fetch("FETCH *:4 BODY[3.TEXT]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(PartSection(vec![3], Some(TextMsgtext)),
                                 None)]));
        assert_eq!(fetch("FETCH *:4 BODY[3.1]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(PartSection(vec![3, 1], None), None)]));
        assert_eq!(fetch("FETCH *:4 BODY[3.2]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(PartSection(vec![3, 2], None), None)]));
        assert_eq!(fetch("FETCH *:4 BODY[4.1.MIME]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(PartSection(vec![4, 1], Some(MimeMsgtext)),
                                 None)]));
        assert_eq!(fetch("FETCH *:4 BODY[4.2.HEADER]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodySection(PartSection(vec![4, 2], Some(HeaderMsgtext)),
                                 None)]));
        assert_eq!(fetch("FETCH *:4 BODY[4.2.2.2.HEADER.FIELDS (DATE FROM)]")
                    .unwrap(), Command::new(
                        Fetch,
                        vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                        vec![BodySection
                             (PartSection
                              (vec![4, 2, 2, 2], Some
                               (HeaderFieldsMsgtext
                                (vec!["DATE".to_string(), "FROM".to_string()]))
                               ),
                              None)]));
    }

    #[test]
    fn test_fetch_bodypeek() {
        assert_eq!(fetch("FETCH * BODY.PEEK[]").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![BodyPeek(AllSection, None)]));
    }

    #[test]
    fn test_fetch_bodypeek_octets() {
        assert_eq!(fetch("FETCH 1,2 BODY.PEEK[]<0.1>").unwrap(), Command::new(
                Fetch,
                vec![Number(1), Number(2)],
                vec![BodyPeek(AllSection, Some((0, 1)))]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[]<400.10000>").unwrap(),
                   Command::new(
                       Fetch,
                       vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                       vec![BodyPeek(AllSection, Some((400, 10000)))]));
    }

    #[test]
    fn test_fetch_bodypeek_section() {
        assert_eq!(fetch("FETCH 1,2 BODY.PEEK[]").unwrap(), Command::new(
                Fetch,
                vec![Number(1), Number(2)],
                vec![BodyPeek(AllSection, None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[HEADER]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodyPeek(MsgtextSection(HeaderMsgtext), None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[TEXT]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodyPeek(MsgtextSection(TextMsgtext), None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[1]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodyPeek(PartSection(vec![1], None), None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[3]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodyPeek(PartSection(vec![3], None), None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[3.HEADER]").unwrap(),
                   Command::new(
                       Fetch,
                       vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                       vec![BodyPeek(PartSection(vec![3], Some(HeaderMsgtext)),
                                     None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[3.TEXT]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodyPeek(PartSection(vec![3], Some(TextMsgtext)), None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[3.1]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodyPeek(PartSection(vec![3, 1], None), None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[3.2]").unwrap(), Command::new(
                Fetch,
                vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                vec![BodyPeek(PartSection(vec![3, 2], None), None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[4.1.MIME]").unwrap(),
                   Command::new(
                       Fetch,
                       vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                       vec![BodyPeek(PartSection(vec![4, 1], Some(MimeMsgtext)),
                                     None)]));
        assert_eq!(fetch("FETCH *:4 BODY.PEEK[4.2.HEADER]").unwrap(),
                   Command::new(
                       Fetch,
                       vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                       vec![BodyPeek(PartSection(vec![4, 2], Some
                                                 (HeaderMsgtext)), None)]));
        assert_eq!(fetch
                   ("FETCH *:4 BODY.PEEK[4.2.2.2.HEADER.FIELDS (DATE FROM)]")
                    .unwrap(),
                   Command::new(
                       Fetch,
                       vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                       vec![BodyPeek(PartSection(vec![4, 2, 2, 2], Some
                                                 (HeaderFieldsMsgtext
                                                  (vec!["DATE".to_string(),
                                                        "FROM".to_string()]))),
                                     None)]));
    }

    #[test]
    fn test_fetch_case_sensitivity() {
        assert_eq!(fetch("fetch * (flags body[4.2.2.2.header.fields.not (date from)]<400.10000>)").unwrap(),
                   Command::new(
                       Fetch,
                       vec![Wildcard],
                       vec![Flags, BodySection(PartSection
                                               (vec![4, 2, 2, 2], Some
                                                (HeaderFieldsNotMsgtext
                                                 (vec!["DATE".to_string(),
                                                       "FROM".to_string()]))),
                                               Some((400, 10000)))]));
        assert_eq!(
            fetch("FETCH * (FLAGS BODY[4.2.2.2.HEADER.FIELDS.NOT (DATE FROM)]<400.10000>)")
                .unwrap(),
            fetch("fetch * (flags body[4.2.2.2.header.fields.not (date from)]<400.10000>)")
                .unwrap());
    }

    #[test]
    fn test_fetch_complex() {
        assert_eq!(fetch
                   ("FETCH *:4 BODY[4.2.2.2.HEADER.FIELDS (DATE FROM)]<400.10000>")
                   .unwrap(),
                   Command::new(
                       Fetch,
                       vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                       vec![BodySection(PartSection
                                        (vec![4, 2, 2, 2], Some
                                         (HeaderFieldsMsgtext
                                          (vec!["DATE".to_string(),
                                                "FROM".to_string()]))),
                                        Some((400, 10000)))]));
        assert_eq!(fetch
                   ("FETCH *:4 BODY.PEEK[4.2.2.2.HEADER.FIELDS (DATE FROM)]<400.10000>")
                    .unwrap(),
                   Command::new(
                       Fetch,
                       vec![Range(Box::new(Wildcard), Box::new(Number(4)))],
                       vec![BodyPeek(PartSection(vec![4, 2, 2, 2],
                                                 Some(HeaderFieldsMsgtext
                                                      (vec!["DATE".to_string(),
                                                            "FROM".to_string()]
                                                       ))),
                                     Some((400, 10000)))]));
        assert_eq!(fetch("FETCH * (FLAGS BODY[HEADER.FIELDS (DATE FROM)])")
                    .unwrap(),
                   Command::new(
                       Fetch,
                       vec![Wildcard],
                       vec![Flags, BodySection(MsgtextSection
                                               (HeaderFieldsMsgtext
                                                (vec!["DATE".to_string(),
                                                      "FROM".to_string()])),
                                               None)]));
        assert_eq!(fetch
                   ("FETCH * (FLAGS BODY[4.2.2.2.HEADER.FIELDS.NOT (DATE FROM)]<400.10000>)")
                    .unwrap(),
                   Command::new(
                       Fetch,
                       vec![Wildcard],
                       vec![Flags, BodySection
                            (PartSection(vec![4, 2, 2, 2], Some
                                         (HeaderFieldsNotMsgtext
                                          (vec!["DATE".to_string(),
                                                "FROM".to_string()]))),
                             Some((400, 10000)))]));
    }
}
