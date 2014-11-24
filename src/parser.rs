pub use self::grammar::{fetch, sequence_set};

peg_file! grammar("grammar.rustpeg")

#[cfg(test)]
mod tests {
    use super::{fetch, sequence_set};
    use command::command::{
        Body,
        BodyStructure,
        Command,
        Envelope,
        Fetch,
        Flags,
        InternalDate,
        RFC822,
        UID
    };
    use command::command::{
        AllRFC822,
        HeaderRFC822,
        SizeRFC822,
        TextRFC822
    };
    use command::command::{
        AllBody,
        HeaderBody,
        HeaderFieldsBody,
        HeaderFieldsNotBody,
        TextBody,
        NumberBody
    };
    use command::sequence_set::{
        Number,
        Range,
        Wildcard
    };

    #[test]
    fn test_invalid_sequences() {
        assert!(sequence_set("").is_err());
        assert!(sequence_set("a").is_err());
        assert!(sequence_set("0").is_err());
        assert!(sequence_set("a:*").is_err());
        assert!(sequence_set(":*").is_err());
        assert!(sequence_set("1:").is_err());
        assert!(sequence_set("1:0").is_err());
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
        let expected = vec![Range(box Number(98), box Number(100))];
        assert_eq!(seq, expected);

        assert!(sequence_set("1:5").is_ok());
        assert!(sequence_set("21:44").is_ok());
    }

    #[test]
    fn test_sequence_range_all() {
        let seq = sequence_set("31:*");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = vec![Range(box Number(31), box Wildcard)];
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_sequence_set() {
        let seq = sequence_set("1231,1342,12,98:104,16");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = vec![Number(1231), Number(1342), Number(12), Range(box Number(98), box Number(104)), Number(16)];
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_fetch_all() {
        let cmd = fetch("FETCH 1:5 ALL");
        assert!(cmd.is_ok());
        let cmd = cmd.unwrap();
        let expected = Command::new(
                Fetch,
                vec![Range(box Number(1), box Number(5))],
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
        let cmd = fetch("FETCH 2:7 FULL");
        assert!(cmd.is_ok());
        let cmd = cmd.unwrap();
        let expected = Command::new(
                Fetch,
                vec![Range(box Number(2), box Number(7))],
                vec![Flags, InternalDate, RFC822(SizeRFC822), Envelope, Body(AllBody)]);
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
                vec![Body(AllBody)]));
        assert_eq!(fetch("FETCH * BODYSTRUCTURE").unwrap(), Command::new(
                Fetch,
                vec![Wildcard],
                vec![BodyStructure]));
    }

    #[test]
    fn test_complex_fetch() {
        let cmd = fetch("FETCH * (FLAGS BODY[HEADER.FIELDS (DATE FROM)])");
        println!("CMD: {}", cmd);
        assert!(cmd.is_ok());
        let cmd = cmd.unwrap();
        let expected = Command::new(
                Fetch,
                vec![Wildcard],
                Vec::new()); // TODO: Fill in the flags.
        assert_eq!(cmd, expected);
    }
}
