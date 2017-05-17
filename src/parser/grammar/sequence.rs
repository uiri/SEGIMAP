use command::sequence_set::SequenceItem;
use parser::grammar::nz_number;

/* Sequence item and set rules */

named!(pub sequence_set<Vec<SequenceItem>>,
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

#[cfg(test)]
mod tests {
    use command::sequence_set::SequenceItem::{
        Number,
        Range,
        Wildcard
    };
    use nom::ErrorKind::Alt;
    use nom::Needed::Size;
    use nom::IResult::{Done, Error, Incomplete};
    use super::{
        sequence_set,
        seq_number,
        seq_range,
    };

    #[test]
    fn test_sequence_set() {
        assert_eq!(sequence_set(b""), Incomplete(Size(1)));
        assert_eq!(sequence_set(b"a"), Error(Alt));
        assert_eq!(sequence_set(b"0"), Error(Alt));
        assert_eq!(sequence_set(b"a:*"), Error(Alt));
        assert_eq!(sequence_set(b":*"), Error(Alt));
        assert_eq!(sequence_set(b"*"), Done(&b""[..], vec![Wildcard]));
        assert_eq!(sequence_set(b"1"), Done(&b""[..], vec![Number(1)]));
        // TODO: is this handled correctly by the parser?
        // Previously these were treated as an error case, but now nom parses it
        // as a `seq-number` with trailing text, instead of as an invalid
        // `seq-range`.
        assert_eq!(sequence_set(b"1:"), Done(&b":"[..], vec![Number(1)]));
        assert_eq!(sequence_set(b"4,5,6,"), Incomplete(Size(7)));
        // TODO: determine if this should parse correctly as `(":0", 1)`, or
        // return an error because "1:0" is not a valid range (i.e., are we OK
        // treating this as a `seq-number` with trailing text?).
        assert_eq!(sequence_set(b"1:0"), Done(&b":0"[..], vec![Number(1)]));
        assert_eq!(sequence_set(b"0:1"), Error(Alt));
        assert_eq!(sequence_set(b"1:1"), Done(&b""[..], vec![
            Range(Box::new(Number(1)), Box::new(Number(1)))
        ]));
        assert_eq!(sequence_set(b"2:4a"), Done(&b"a"[..], vec![
            Range(Box::new(Number(2)), Box::new(Number(4)))
        ]));
        assert_eq!(sequence_set(b"*:3, 4:4"), Done(&b", 4:4"[..], vec![
            Range(Box::new(Wildcard), Box::new(Number(3)))
        ]));
        assert_eq!(sequence_set(b"*:3,4:4"), Done(&b""[..], vec![
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
}
