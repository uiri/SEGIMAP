use std::iter::Iterator;

#[deriving(Clone, PartialEq, Show)]
pub enum SequenceItem {
    Number(uint),
    Range(Box<SequenceItem>, Box<SequenceItem>),
    Wildcard
}

pub fn parse(sequence_string: &str) -> Option<Vec<SequenceItem>> {
    let mut sequences = sequence_string.split(',');
    let mut sequence_set = Vec::new();
    for sequence in sequences {
        let mut range = sequence.split(':');
        let start = range.next();
        let stop = range.next();
        match range.next() {
            Some(_) => return None,
            _ => {}
        }
        match stop {
            None => {
                match start {
                    None => return None,
                    Some(seq) => {
                        let intseq_opt: Option<uint> = from_str(seq);
                        match intseq_opt {
                            Some(intseq) => {
                                sequence_set.push(Number(intseq));
                            }
                            None => {
                                if sequence == "*" {
                                    sequence_set.push(Wildcard);
                                }
                            }
                        }
                    }
                }
            }
            Some(end) => {
                match start {
                    None => return None,
                    Some(begin) => {
                        let range_one = parse(begin);
                        let range_two = parse(end);
                        let mut range_val = Vec::new();
                        match range_one {
                            Some(range_start_vec) => {
                                if range_start_vec.len() != 1 {
                                    return None;
                                }
                                range_val = range_start_vec;
                                match range_two {
                                    Some(range_stop_vec) => {
                                        if range_stop_vec.len() != 1 {
                                            return None;
                                        }
                                        range_val.push(range_stop_vec[0].clone());
                                        sequence_set.push(Range(box range_val[0].clone(), box range_val[1].clone()));
                                    }
                                    None => return None
                                }
                            },
                            None => return None
                        }
                    }
                }
            }
        }
    }
    return Some(sequence_set);
}

// TODO: Find a way to handle sequences in O(1) as currently, the memory usage
// of the vec returned by this function scales at O(n).
pub fn iterator(sequence_set: Vec<SequenceItem>, max_id: uint) -> Vec<uint> {
    // If the number of possible messages is 0, we return an empty vec.
    if max_id == 0 { return Vec::new() }

    let stop = max_id + 1;

    let mut items = Vec::new();
    for item in sequence_set.iter() {
        match item {
            &Number(num) => { items.push(num) },
            &Range(ref a, ref b) => {
                let a = match **a {
                    Number(num) => { num },
                    Wildcard => { max_id }
                    Range(_, _) => {
                        error!("A range of ranges is invalid.");
                        continue;
                    }
                };
                let b = match **b {
                    Number(num) => { num },
                    Wildcard => { max_id }
                    Range(_, _) => {
                        error!("A range of ranges is invalid.");
                        continue;
                    }
                };
                let mut min = 0;
                let mut max = 0;
                if a <= b {
                    min = a;
                    max = b;
                } else {
                    min = b;
                    max = a;
                }
                if min > stop { min = stop; }
                if max > stop { max = stop; }
                let seq_range: Vec<uint> = range(min, max + 1).collect();
                items.push_all(seq_range.as_slice());
            },
            &Wildcard => {
                // If the sequence set contains the wildcard operator, it spans
                // the entire possible range of messages.
                return range(1, stop).collect()
            }
        }
    }

    // Sort and remove duplicates.
    items.sort();
    items.dedup();
    // Remove all elements that are greater than the maximum.
    let items: Vec<uint> = items.into_iter().filter(|&x| x <= max_id).collect();
    return items;
}

// TODO: Find a way to handle sequences in O(1) as currently, the memory usage
// of the vec returned by this function scales at O(n).
pub fn uid_iterator(sequence_set: Vec<SequenceItem>) -> Vec<uint> {
    let mut items = Vec::new();
    for item in sequence_set.iter() {
        match item {
            &Number(num) => { items.push(num) },
            &Range(ref a, ref b) => {
                let a = match **a {
                    Number(num) => { num },
                    Wildcard => { return Vec::new() } // TODO: implement
                    Range(_, _) => {
                        error!("A range of ranges is invalid.");
                        continue;
                    }
                };
                let b = match **b {
                    Number(num) => { num },
                    Wildcard => { return Vec::new() } // TODO: implement
                    Range(_, _) => {
                        error!("A range of ranges is invalid.");
                        continue;
                    }
                };
                let mut min = 0;
                let mut max = 0;
                if a <= b {
                    min = a;
                    max = b;
                } else {
                    min = b;
                    max = a;
                }
                //if min > stop { min = stop; }
                //if max > stop { max = stop; }
                let seq_range: Vec<uint> = range(min, max + 1).collect();
                items.push_all(seq_range.as_slice());
            },
            &Wildcard => {
                return Vec::new() // TODO: implement
            }
        }
    }

    // Sort and remove duplicates.
    items.sort();
    items.dedup();
    // Remove all elements that are greater than the maximum.
    //let items: Vec<uint> = items.into_iter().filter(|&x| x <= max_id).collect();
    return items;
}

#[test]
fn test_sequence_num() {
    assert_eq!(iterator(vec![Number(4324)], 5000), vec![4324]);
    assert_eq!(iterator(vec![Number(23), Number(44)], 5000), vec![23, 44]);
    assert_eq!(iterator(vec![Number(6), Number(6), Number(2)], 5000), vec![2, 6]);
}

#[test]
fn test_sequence_past_end() {
    assert_eq!(iterator(vec![Number(4324)], 100), Vec::new());
    assert_eq!(iterator(vec![Number(23), Number(44)], 30), vec![23]);
    assert_eq!(iterator(vec![Number(6), Number(6), Number(2)], 4), vec![2]);
}

#[test]
fn test_sequence_range() {
    assert_eq!(iterator(vec![Range(box Number(6), box Wildcard)], 10), vec![6, 7, 8, 9, 10]);
    assert_eq!(iterator(vec![Range(box Number(1), box Number(10))], 4), vec![1, 2, 3, 4]);
    assert_eq!(iterator(vec![Range(box Wildcard, box Number(8)), Number(9), Number(2), Number(2)], 12), vec![2, 8, 9, 10, 11, 12]);
}

#[test]
fn test_sequence_wildcard() {
    assert_eq!(iterator(vec![Range(box Number(10), box Wildcard), Wildcard], 6), vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(iterator(vec![Wildcard, Number(8)], 3), vec![1, 2, 3]);
}

#[test]
fn test_sequence_complex() {
    assert_eq!(iterator(
            vec![Number(1), Number(3), Range(box Number(5), box Number(7)), Number(9), Number(12), Range(box Number(15), box Wildcard)], 13),
            vec![1, 3, 5, 6, 7, 9, 12, 13]);
}
