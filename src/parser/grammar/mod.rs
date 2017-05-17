use nom::{crlf, Slice};
use std::str;

pub use self::fetch::fetch;

mod fetch;
mod sequence;

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

#[cfg(test)]
mod tests {
    use nom::ErrorKind::{Alt, Char, Count, OneOf, TakeWhile1, MapOpt, Tag};
    use nom::Needed::Size;
    use nom::IResult::{Done, Error, Incomplete};
    use super::{
        astring,
        digit_nz,
        literal,
        number,
        nz_number,
        quoted,
        string,
        whitespace
    };

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
}
