// This file is made up largely of utility methods which are invoked by the
// session in its interpret method. They are separate because they don't rely
// on the session (or take what they do need as arguments) and/or they are
// called by the session in multiple places.

use std::collections::HashSet;
use std::io::fs;
use std::io::fs::PathExtensions;
use std::os::make_absolute;
use std::str::{from_utf8, StrSlice, CharSplits};
use std::ascii::OwnedAsciiExt;
use regex::Regex;

use parser;
use folder::Folder;
use message::{Flag, Seen, Answered, Deleted, Draft, Flagged};

use command::command::Command;
use command::command::BodySection;
use command::sequence_set;

/// Representation of a STORE operation
pub enum StoreName {
    Replace, // replace current flags with new flags
    Add, // add new flags to current flags
    Sub // remove new flags from current flags
}

pub fn perform_select(maildir: &str, select_args: Vec<&str>, examine: bool,
                      tag: &str) -> (Option<Folder>, String) {
    let err_res = (None, "".to_string());
    if select_args.len() < 1 { return err_res; }
    let mbox_name = regex!("INBOX").replace(select_args[0].trim_chars('"'), "."); // "));
    let maildir_path = Path::new(maildir.as_slice()).join(mbox_name);
    let folder = match Folder::new(maildir_path, examine) {
        None => { return err_res; }
        Some(folder) => folder
    };
    // * <n> EXISTS
    let mut ok_res = format!("* {} EXISTS\r\n", folder.exists);
    // * <n> RECENT
    ok_res.push_str(format!("* {} RECENT\r\n", folder.recent).as_slice());
    // * OK UNSEEN
    ok_res.push_str(folder.unseen().as_slice());
    // * Flags
    // Should match values in enum Flag in message.rs
    ok_res.push_str("* FLAGS (\\Answered \\Deleted \\Draft \\Flagged \\Seen)\r\n");
    // * OK PERMANENTFLAG
    // Should match values in enum Flag in message.rs
    ok_res.push_str("* OK [PERMANENTFLAGS (\\Answered \\Deleted \\Draft \\Flagged \\Seen)] Permanent flags\r\n");
    // * OK UIDNEXT
    // * OK UIDVALIDITY
    ok_res.push_str(format!("{} OK {} SELECT command was successful\r\n", tag,
                            folder.read_status()).as_slice());
    return (Some(folder), ok_res);
}


/// Parse and perform the store operation specified by store_args. Returns the
/// response to the client or None if a BAD response should be sent back to
/// the client
pub fn store(folder: &mut Folder, store_args: Vec<&str>, seq_uid: bool,
                 tag: &str) -> Option<String> {
    if store_args.len() < 3 { return None; }

    // Parse the sequence set argument
    let sequence_set_opt = sequence_set::parse(store_args[0].trim_chars('"'));
    // Grab how to handle the flags. It should be case insensitive.
    let data_name = store_args[1].trim_chars('"').to_string().into_ascii_lower();

    // Split into "flag" part and "silent" part.
    let mut data_name_parts = data_name.as_slice().split('.');
    let flag_part = data_name_parts.next();
    let silent_part = data_name_parts.next();

    // There shouldn't be any more parts to the data name argument
    match data_name_parts.next() {
        Some(_) => return None,
        _ => {}
    }

    // Grab the flags themselves.
    let data_value = store_args[2].trim_chars('"');

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
    for flag in data_value.trim_chars('(').trim_chars(')').split(' ') {
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
            let res = folder.store(sequence_iter, flag_name, silent, flags,
                                   seq_uid, tag);
            Some(res)
        }
    };
}

/// Take the rest of the arguments provided by the client and parse them into a
/// Command object with command::fetch.
pub fn fetch(mut args: CharSplits<char>) -> Result<Command,String> {
    let mut cmd = "FETCH".to_string();
    for arg in args {
        cmd.push(' ');
        cmd.push_str(arg);
    }

    // Parse the command with the PEG parser.
   parser::fetch(cmd.as_slice())
}

/// Perform the fetch operation on each sequence number indicated and return
/// the response to be sent back to the client.
pub fn fetch_loop(parsed_cmd: Command, folder: &mut Folder,
                  sequence_iter: Vec<usize>, tag: &str, uid: bool) -> String {
    for attr in parsed_cmd.attributes.iter() {
        match attr {
            &BodySection(_, _) => {
                let mut seen_flag_set = HashSet::new();
                seen_flag_set.insert(Seen);
                folder.store(sequence_iter.clone(), Add, true, seen_flag_set,
                             false, tag);
                break;
            }
            _ => {}
        }
    }

    let mut res = String::new();
    for i in sequence_iter.iter() {
        let index = if uid {
            match folder.get_index_from_uid(i) {
                Some(index) => *index,
                None => {continue;}
            }
        } else {
            *i-1
        };
        res.push_str(folder.fetch(index, &parsed_cmd.attributes).as_slice());
    }
    res.push_str(tag);
    res.push_str(" OK ");
    if uid {
        res.push_str("UID ");
    }
    res.push_str("FETCH completed\r\n");
    res
}

/// Takes a flag argument and returns the corresponding enum.
fn parse_flag(flag: &str) -> Option<Flag> {
    match flag {
        "\\Deleted" => Some(Deleted),
        "\\Seen" => Some(Seen),
        "\\Draft" => Some(Draft),
        "\\Answered" => Some(Answered),
        "\\Flagged" => Some(Flagged),
        _ => None
    }
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

/// For the given dir, make sure it is a valid mail folder and, if it is,
/// generate the LIST response for it.
fn list_dir(dir: Path, regex: &Regex, maildir_path: &Path) -> Option<String> {
    let dir_string = dir.display().to_string();
    let dir_name = from_utf8(dir.filename().unwrap()).unwrap();

    // These folder names are used to hold mail. Every other folder is
    // valid.
    if  dir_name == "cur" || dir_name == "new" || dir_name == "tmp" {
        return None;
    }

    let abs_dir = make_absolute(&dir);

    // If it doesn't have any mail, then it isn't selectable as a mail
    // folder but it may contain subfolders which hold mail.
    let mut flags = match fs::readdir(&dir.join("cur")) {
        Err(_) => "\\Noselect".to_string(),
        _ => {
            match fs::readdir(&dir.join("new")) {
                Err(_) => "\\Noselect".to_string(),
                // If there is new mail in the folder, we should inform the
                // client. We do this only because we have to perform the
                // check in order to determine selectability. The RFC says
                // not to perform the check if it would slow down the
                // response time.
                Ok(newlisting) => {
                    if newlisting.len() == 0 {
                        "\\Unmarked".to_string()
                    } else {
                        "\\Marked".to_string()
                    }
                }
            }
        }
    };

    // Changing folders in mutt doesn't work properly if we don't indicate
    // whether or not a given folder has subfolders. Mutt has issues
    // selecting folders with subfolders for reading mail, unfortunately.
    match fs::readdir(&dir) {
        Err(_) => { return None; }
        Ok(dir_listing) => {
            let mut children = false;
            for subdir in dir_listing.iter() {
                if dir == *maildir_path {
                    break;
                }
                let subdir_str = from_utf8(subdir.filename().unwrap()).unwrap();
                if subdir_str != "cur" &&
                    subdir_str != "new" &&
                    subdir_str != "tmp" {
                        match fs::readdir(&subdir.join("cur")) {
                            Err(_) => { continue; }
                            _ => {}
                        }
                        match fs::readdir(&subdir.join("new")) {
                            Err(_) => { continue; }
                            _ => {}
                        }
                        children = true;
                        break;
                    }
            }
            if children {
                flags.push_str(" \\HasChildren");
            } else {
                flags.push_str(" \\HasNoChildren");
            }
        }
    }

    let re_path = make_absolute(maildir_path);
    let re_opt = Regex::new(format!("^{}", re_path.display()).as_slice());
    return match re_opt {
        Err(_) =>  None,
        Ok(re) => {
            if !dir.is_dir() || !regex.is_match(dir_string.as_slice()) {
                return None;
            }
            let mut list_str = "* LIST (".to_string();
            list_str.push_str(flags.as_slice());
            list_str.push_str(") \"/\" ");
            list_str.push_str(regex!("INBOX/").replace
                              (re.replace(abs_dir.display().to_string()
                                          .as_slice(), "INBOX").as_slice(),
                               "").as_slice());
            Some(list_str)
        }
    };
}

/// Go through the logged in user's maildir and list every folder matching
/// the given regular expression. Returns a list of LIST responses.
pub fn list(maildir: &str, regex: Regex) -> Vec<String> {
    let maildir_path = Path::new(maildir);
    let mut responses = Vec::new();
    match list_dir(maildir_path.clone(), &regex, &maildir_path) {
        Some(list_response) => {
            responses.push(list_response);
        }
        _ => {}
    }
    match fs::walk_dir(&maildir_path) {
        Err(_) => Vec::new(),
        Ok(mut dir_list) => {
            for dir in dir_list {
                match list_dir(dir, &regex, &maildir_path) {
                    Some(list_response) => {
                        responses.push(list_response);
                    }
                    _ => {}
                }
            }
            responses
        }
    }
}
