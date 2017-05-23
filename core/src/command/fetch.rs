use std::collections::HashSet;

use command::FetchCommand;
use command::Attribute::BodySection;
use folder::Folder;
use parser::{self, ParserResult};

use message::Flag::Seen;
use super::store::StoreName::Add;

/// Take the rest of the arguments provided by the client and parse them into a
/// `FetchCommand` object with `parser::fetch`.
pub fn fetch(args: Vec<&str>) -> ParserResult<FetchCommand> {
    let mut cmd = "FETCH".to_string();
    for arg in args {
        cmd.push(' ');
        cmd.push_str(arg);
    }

    parser::fetch(cmd.as_bytes())
}

/// Perform the fetch operation on each sequence number indicated and return
/// the response to be sent back to the client.
pub fn fetch_loop(parsed_cmd: &FetchCommand, folder: &mut Folder,
                  sequence_iter: &[usize], tag: &str, uid: bool) -> String {
    for attr in &parsed_cmd.attributes {
        if let BodySection(_, _) = *attr {
            let mut seen_flag_set = HashSet::new();
            seen_flag_set.insert(Seen);
            folder.store(sequence_iter.to_vec(), &Add, true, seen_flag_set,
                         false, tag);
            break;
        }
    }

    let mut res = String::new();
    for i in sequence_iter {
        let index = if !uid {
            *i-1
        } else if let Some(index) = folder.get_index_from_uid(i) {
            *index
        } else {
            continue;
        };
        res.push_str(&folder.fetch(index, &parsed_cmd.attributes)[..]);
    }
    res.push_str(tag);
    res.push_str(" OK ");
    if uid {
        res.push_str("UID ");
    }
    res.push_str("FETCH completed\r\n");
    res
}
