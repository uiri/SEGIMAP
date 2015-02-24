pub use self::grammar::{fetch, sequence_set};

// grammar.rustpeg contains the parsing expression grammar needed in order to
// parse FETCH commands.
peg_file! grammar("grammar.rustpeg");

// Tests for the parsed FETCH commands follow
#[cfg(test)]
mod tests {
    use super::{fetch, sequence_set};
    use command::command::{
        Body,
        BodyPeek,
        BodySection,
        BodyStructure,
        Command,
        Envelope,
        Fetch,
        Flags,
        InternalDate,
        RFC822,
        UID
    };
    use command::command::Attribute::{
        AllSection,
        MsgtextSection,
        PartSection
    };
    use command::command::MsgText::{
        HeaderMsgtext,
        HeaderFieldsMsgtext,
        HeaderFieldsNotMsgtext,
        MimeMsgtext,
        TextMsgtext
    };
    use command::command::RFC822Attribute::{
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
