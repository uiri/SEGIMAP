use std::iter::Iterator;

/// This represents an individual item in the list of requested message ids
/// passed by the client
/// The client can pass a number representing a single id, a wildcard
/// (represented by *) or a range which is made up of a start non-range sequence
/// item and an end non-range sequence item separated by a colon
/// A range represents all items with ids between its start and end, inclusive.
#[derive(Clone, PartialEq, Show)]
pub enum SequenceItem {
    Number(uint),
    Range(Box<SequenceItem>, Box<SequenceItem>),
    Wildcard
}

fn parse_item(item: &str) -> Option<SequenceItem> {
    let intseq_opt: Option<uint> = from_str(item);
    match intseq_opt {
        Some(intseq) => {
            // item is a valid number so return that
            Some(Number(intseq))
        }
        None => {
            // item is not a valid number
            // If it is the wildcard value return that
            // Otherwise, return no sequence item
            if item == "*" {
                Some(Wildcard)
            } else {
                None
            }
        }
    }
}

/// Given a string as passed in from the client, create a list of sequence items
/// If the string does not represent a valid list, return None
pub fn parse(sequence_string: &str) -> Option<Vec<SequenceItem>> {
    let mut sequences = sequence_string.split(',');
    let mut sequence_set = Vec::new();
    for sequence in sequences {
        let mut range = sequence.split(':');
        let start = match range.next() {
            None => return None,
            Some(seq) => {
                match parse_item(seq) {
                    None => return None,
                    Some(item) => item
                }
            }
        };
        let stop = match range.next() {
            None => {
                // Nothing after ':'
                // Add the number or wildcard and bail out
                sequence_set.push(start);
                continue;
            }
            Some(seq) => {
                match parse_item(seq) {
                    None => return None,
                    Some(item) => item
                }
            }
        };

        // A valid range only has one colon
        match range.next() {
            Some(_) => return None,
            _ => {}
        }

        sequence_set.push(Range(box start, box stop));
    }
    return Some(sequence_set);
}

/// Create the list of unsigned integers representing valid ids from a list of
/// sequence items. Ideally this would handle wildcards in O(1) rather than O(n)
pub fn iterator(sequence_set: &Vec<SequenceItem>, max_id: uint) -> Vec<uint> {
    // If the number of possible messages is 0, we return an empty vec.
    if max_id == 0 { return Vec::new() }

    let stop = max_id + 1;

    let mut items = Vec::new();
    for item in sequence_set.iter() {
        match item {
            // For a number we just need the value
            &Number(num) => { items.push(num) },
            // For a range we need all the values inside it
            &Range(ref a, ref b) => {
                // Grab the start of the range
                let a = match **a {
                    Number(num) => { num },
                    Wildcard => { max_id }
                    Range(_, _) => {
                        error!("A range of ranges is invalid.");
                        continue;
                    }
                };
                // Grab the end of the range
                let b = match **b {
                    Number(num) => { num },
                    Wildcard => { max_id }
                    Range(_, _) => {
                        error!("A range of ranges is invalid.");
                        continue;
                    }
                };
                // Figure out which way the range points
                let mut min = 0;
                let mut max = 0;
                if a <= b {
                    min = a;
                    max = b;
                } else {
                    min = b;
                    max = a;
                }

                // Bounds checks
                if min > stop { min = stop; }
                if max > stop { max = stop; }

                // Generate the list of values between the min and max
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

pub fn uid_iterator(sequence_set: &Vec<SequenceItem>) -> Vec<uint> {
    let mut items = Vec::new();
    for item in sequence_set.iter() {
        match item {
            &Number(num) => { items.push(num) },
            &Range(ref a, ref b) => {
                let a = match **a {
                    Number(num) => { num },
                    Wildcard => { return Vec::new() }
                    Range(_, _) => {
                        error!("A range of ranges is invalid.");
                        continue;
                    }
                };
                let b = match **b {
                    Number(num) => { num },
                    Wildcard => { return Vec::new() }
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
                return Vec::new()
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
    assert_eq!(iterator(&vec![Number(4324)], 5000), vec![4324]);
    assert_eq!(iterator(&vec![Number(23), Number(44)], 5000), vec![23, 44]);
    assert_eq!(iterator(&vec![Number(6), Number(6), Number(2)], 5000), vec![2, 6]);
}

#[test]
fn test_sequence_past_end() {
    assert_eq!(iterator(&vec![Number(4324)], 100), Vec::new());
    assert_eq!(iterator(&vec![Number(23), Number(44)], 30), vec![23]);
    assert_eq!(iterator(&vec![Number(6), Number(6), Number(2)], 4), vec![2]);
}

#[test]
fn test_sequence_range() {
    assert_eq!(iterator(&vec![Range(Box::new(Number(6)), Box::new(Wildcard))], 10), vec![6, 7, 8, 9, 10]);
    assert_eq!(iterator(&vec![Range(Box::new(Number(1)), Box::new(Number(10)))], 4), vec![1, 2, 3, 4]);
    assert_eq!(iterator(&vec![Range(Box::new(Wildcard), Box::new(Number(8))), Number(9), Number(2), Number(2)], 12), vec![2, 8, 9, 10, 11, 12]);
}

#[test]
fn test_sequence_wildcard() {
    assert_eq!(iterator(&vec![Range(Box::new(Number(10)), Box::new(Wildcard)), Wildcard], 6), vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(iterator(&vec![Wildcard, Number(8)], 3), vec![1, 2, 3]);
}

#[test]
fn test_sequence_complex() {
    assert_eq!(iterator(
            &vec![Number(1), Number(3), Range(Box::new(Number(5)), Box::new(Number(7))), Number(9), Number(12), Range(Box::new(Number(15)), Box::new(Wildcard))], 13),
            vec![1, 3, 5, 6, 7, 9, 12, 13]);
}
