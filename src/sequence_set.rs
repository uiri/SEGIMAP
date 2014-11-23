pub use self::sequence_set::sequence_set;

peg_file! sequence_set("sequence_set.rustpeg")

#[deriving(PartialEq, Show)]
enum SequenceItem {
    Number(uint),
    Range(Box<SequenceItem>, Box<SequenceItem>),
    All
}

#[deriving(PartialEq, Show)]
pub struct SequenceSet {
    sequence_item: SequenceItem,
    sequence_sets: Vec<SequenceSet>
}

#[cfg(test)]
mod tests {
    use super::sequence_set::sequence_set;
    use super::{All, Number, Range, SequenceSet};

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
        let expected = SequenceSet {
            sequence_item: Number(4324),
            sequence_sets: Vec::new()
        };
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_sequence_all() {
        let seq = sequence_set("*");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = SequenceSet {
            sequence_item: All,
            sequence_sets: Vec::new()
        };
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_sequence_ranges() {
        let seq = sequence_set("98:100");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = SequenceSet {
            sequence_item: Range(box Number(98), box Number(100)),
            sequence_sets: Vec::new()
        };
        assert_eq!(seq, expected);

        assert!(sequence_set("1:5").is_ok());
        assert!(sequence_set("21:44").is_ok());
    }

    #[test]
    fn test_sequence_range_all() {
        let seq = sequence_set("31:*");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = SequenceSet {
            sequence_item: Range(box Number(31), box All),
            sequence_sets: Vec::new()
        };
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_sequence_set() {
        let seq = sequence_set("1231,1342,12,98:104,16");
        assert!(seq.is_ok());
        let seq = seq.unwrap();
        let expected = SequenceSet {
            sequence_item: Number(1231),
            sequence_sets: vec![
                SequenceSet {
                    sequence_item: Number(1342),
                    sequence_sets: vec![
                        SequenceSet {
                            sequence_item: Number(12),
                            sequence_sets: vec![
                                SequenceSet {
                                    sequence_item: Range(box Number(98), box Number(104)),
                                    sequence_sets: vec![
                                        SequenceSet {
                                            sequence_item: Number(16),
                                            sequence_sets: Vec::new()
                                        }
                                    ]
                                }
                            ]
                        }
                    ]
                }
            ]
        };
        assert_eq!(seq, expected);
    }
}
