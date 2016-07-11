use std::ascii::AsciiExt;
use std::collections::HashSet;

use folder::Folder;
use message::Flag;
use message::Message;
use message::parse_flag;

use self::StoreName::{Add, Replace, Sub};
use super::sequence_set;

/// Representation of a STORE operation
pub enum StoreName {
    Replace, // replace current flags with new flags
    Add, // add new flags to current flags
    Sub // remove new flags from current flags
}

/// Parse and perform the store operation specified by store_args. Returns the
/// response to the client or None if a BAD response should be sent back to
/// the client
pub fn store(folder: &mut Folder, store_args: Vec<&str>, seq_uid: bool,
                 tag: &str) -> Option<String> {
    if store_args.len() < 3 { return None; }

    // Parse the sequence set argument
    let sequence_set_opt = sequence_set::parse(store_args[0].trim_matches('"'));
    // Grab how to handle the flags. It should be case insensitive.
    let data_name = store_args[1].trim_matches('"').to_ascii_lowercase();

    // Split into "flag" part and "silent" part.
    let mut data_name_parts = (&data_name[..]).split('.');
    let flag_part = data_name_parts.next();
    let silent_part = data_name_parts.next();

    // There shouldn't be any more parts to the data name argument
    match data_name_parts.next() {
        Some(_) => return None,
        _ => {}
    }

    // Grab the flags themselves.
    let data_value = store_args[2].trim_matches('"');

    // Set the silent flag if it is present. If there is something else
    // instead of the word "silent", a BAD response should be sent to the
    // client.
    let silent = match silent_part {
        None => false,
        Some(part) => {
            if part == "silent" {
                true
            } else {
                return None
            }
        }
    };

    // Parse the flag_part into an enum describing what to do with the
    // data_value.
    let flag_name = match parse_storename(flag_part) {
        Some(storename) => storename,
        None => return None
    };

    // Create the Set of flags to be STORE'd from the data_value argument.
    let mut flags: HashSet<Flag> = HashSet::new();
    for flag in data_value.trim_matches('(').trim_matches(')').split(' ') {
        match parse_flag(flag) {
            None => { continue; }
            Some(insert_flag) => { flags.insert(insert_flag); }
        }
    }

    // Perform the STORE operation on each message specified by the
    // sequence set.
    return match sequence_set_opt {
        None => None,
        Some(sequence_set) => {
            let sequence_iter = if seq_uid {
                sequence_set::uid_iterator(&sequence_set)
            } else {
                sequence_set::iterator(&sequence_set, folder.message_count())
            };
            let res = folder.store(sequence_iter, &flag_name, silent, flags,
                                   seq_uid, tag);
            Some(res)
        }
    };
}

/// Takes the argument specifying what to do with the provided flags in a store
/// operation and returns the corresponding enum.
fn parse_storename(storename: Option<&str>) -> Option<StoreName> {
    match storename {
        Some("flags") => Some(Replace),
        Some("+flags") => Some(Add),
        Some("-flags") => Some(Sub),
        _ => None
    }
}

pub fn message(msg: &mut Message, flag_name: &StoreName,
                 new_flags: HashSet<Flag>) -> String {
    match flag_name {
        &Sub => {
            for flag in new_flags.iter() {
                msg.flags.remove(flag);
            }
        }
        &Replace => { msg.flags = new_flags; }
        &Add => {
            for flag in new_flags.into_iter() {
                msg.flags.insert(flag);
            }
        }
    }
    msg.deleted = msg.flags.contains(&Flag::Deleted);
    msg.print_flags()
}
