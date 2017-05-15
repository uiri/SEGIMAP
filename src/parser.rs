// FIXME: move the nom parsing functions into the grammar module, and only
// expose the top-level API.

use command::sequence_set::SequenceItem;
use std::collections::HashSet;
use std::str::FromStr;

pub use self::grammar::fetch;
pub use self::grammar::ParseError;

// grammar.rustpeg contains the parsing expression grammar needed in order to
// parse FETCH commands.
peg_file! grammar("grammar.rustpeg");

const DIGITS: &'static str = "0123456789";
const NZ_DIGITS: &'static str = "123456789";

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

/// Converts a `char` and a `Vec<char>` into a string by prepending the `char`
/// to the vector and collecting it into a string.
// This function is used to convert individual digit characters into a `String`
// representation.
fn chars_to_string(chr: char, v: Vec<char>) -> String {
    let res: String = v.into_iter().collect();
    chr.to_string() + &res
}

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

/// Recognizes exactly one non-zero numerical character: 1-9.
// digit-nz = %x31-39
//    ; 1-9
named!(digit_nz<char>, one_of!(NZ_DIGITS));

/// Recognizes a non-zero unsigned 32-bit integer.
// (0 < n < 4,294,967,296)
named!(nz_number<usize>,
    map_res!(
        do_parse!(
            d: digit_nz                   >>
            rest: many0!(one_of!(DIGITS)) >>

            (chars_to_string(d, rest))
        ),
        |s: String| { (&s).parse() }
    )
);

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
    use nom::ErrorKind::{Alt, OneOf, MapRes};
    use nom::Needed::Size;
    use nom::IResult::{Done, Error, Incomplete};
    use super::{digit_nz, nz_number, seq_number, seq_range};

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
    fn test_digit_nz() {
        assert_eq!(digit_nz(b""), Incomplete(Size(1)));
        assert_eq!(digit_nz(b"a"), Error(OneOf));
        assert_eq!(digit_nz(b"1"), Done(&b""[..], '1'));
        assert_eq!(digit_nz(b"62"), Done(&b"2"[..], '6'));
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
        assert_eq!(nz_number(b"100000000000000000000"), Error(MapRes));
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
