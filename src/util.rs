// This file is made up largely of utility methods which are invoked by the
// session in its interpret method. They are separate because they don't rely
// on the session (or take what they do need as arguments) and/or they are
// called by the session in multiple places.

use std::ascii::AsciiExt;
use std::collections::HashSet;
use std::env::current_dir;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str::Split;
use regex;
use regex::Regex;
use walkdir::WalkDir;

use parser;
use folder::Folder;
use message::Flag;
use message::Flag::{Seen, Answered, Deleted, Draft, Flagged};

use command::command::Command;
use command::command::Attribute::BodySection;
use command::sequence_set;

use self::StoreName::{Add, Replace, Sub};

/// Representation of a STORE operation
pub enum StoreName {
    Replace, // replace current flags with new flags
    Add, // add new flags to current flags
    Sub // remove new flags from current flags
}

fn make_absolute(dir: &Path) -> String {
    let mut abs_path = current_dir().unwrap();
    abs_path.push(dir);
    return abs_path.as_path().display().to_string();
}

pub fn inbox_re() -> Regex { Regex::new("INBOX").unwrap() }

pub fn perform_select(maildir: &str, select_args: Vec<&str>, examine: bool,
                      tag: &str) -> (Option<Folder>, String) {
    let err_res = (None, "".to_string());
    if select_args.len() < 1 { return err_res; }
    let mbox_name = inbox_re().replace(select_args[0].trim_matches('"'), "."); // "));
    let mut maildir_path = PathBuf::new();
    maildir_path.push(maildir);
    maildir_path.push(mbox_name);
    let folder =  match Folder::new(maildir_path, examine) {
        None => { return err_res; }
        Some(folder) => folder.clone()
    };
    // * <n> EXISTS
    let mut ok_res = format!("* {} EXISTS\r\n", folder.exists);
    // * <n> RECENT
    ok_res.push_str(&(format!("* {} RECENT\r\n", folder.recent))[..]);
    // * OK UNSEEN
    ok_res.push_str(&folder.unseen()[..]);
    // * Flags
    // Should match values in enum Flag in message.rs
    ok_res.push_str("* FLAGS (\\Answered \\Deleted \\Draft \\Flagged \\Seen)\r\n");
    // * OK PERMANENTFLAG
    // Should match values in enum Flag in message.rs
    ok_res.push_str("* OK [PERMANENTFLAGS (\\Answered \\Deleted \\Draft \\Flagged \\Seen)] Permanent flags\r\n");
    // * OK UIDNEXT
    // * OK UIDVALIDITY
    ok_res.push_str(&(format!("{} OK {} SELECT command was successful\r\n", tag,
                            folder.read_status()))[..]);
    return (Some(folder), ok_res);
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

/// Take the rest of the arguments provided by the client and parse them into a
/// Command object with command::fetch.
pub fn fetch(args: Split<char>) -> Result<Command, parser::ParseError> {
    let mut cmd = "FETCH".to_string();
    for arg in args {
        cmd.push(' ');
        cmd.push_str(arg);
    }

    // Parse the command with the PEG parser.
   parser::fetch(&cmd[..])
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
                folder.store(sequence_iter.clone(), &Add, true, seen_flag_set,
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
fn list_dir(dir: &Path, regex: &Regex, maildir_path: &Path) -> Option<String> {
    let dir_string = dir.display().to_string();
    let dir_name = dir.file_name().unwrap().to_str().unwrap();

    // These folder names are used to hold mail. Every other folder is
    // valid.
    if  dir_name == "cur" || dir_name == "new" || dir_name == "tmp" {
        return None;
    }

    let abs_dir = make_absolute(dir);

    // If it doesn't have any mail, then it isn't selectable as a mail
    // folder but it may contain subfolders which hold mail.
    let mut flags = match fs::read_dir(&dir.join("cur")) {
        Err(_) => "\\Noselect".to_string(),
        _ => {
            match fs::read_dir(&dir.join("new")) {
                Err(_) => "\\Noselect".to_string(),
                // If there is new mail in the folder, we should inform the
                // client. We do this only because we have to perform the
                // check in order to determine selectability. The RFC says
                // not to perform the check if it would slow down the
                // response time.
                Ok(newlisting) => {
                    if newlisting.count() == 0 {
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
    match fs::read_dir(&dir) {
        Err(_) => { return None; }
        Ok(dir_listing) => {
            let mut children = false;
            for subdir_entry in dir_listing {
                match subdir_entry {
                    Ok(subdir) => {
                        if *dir == *maildir_path {
                            break;
                        }
                        let subdir_path = subdir.path();
                        let subdir_str = subdir_path.as_path().file_name().unwrap().to_str().unwrap();
                        if subdir_str != "cur" &&
                            subdir_str != "new" &&
                            subdir_str != "tmp" {
                                match fs::read_dir(&subdir.path().join("cur")) {
                                    Err(_) => { continue; }
                                    _ => {}
                                }
                                match fs::read_dir(&subdir.path().join("new")) {
                                    Err(_) => { continue; }
                                    _ => {}
                                }
                                children = true;
                                break;
                            }
                    },
                    _ => {}
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
    let re_opt = Regex::new(&(format!("^{}", re_path))[..]);
    return match re_opt {
        Err(_) =>  None,
        Ok(re) => {
            if !fs::metadata(dir).unwrap().is_dir() || !regex.is_match(&dir_string[..]) {
                return None;
            }
            let mut list_str = "* LIST (".to_string();
            list_str.push_str(&flags[..]);
            list_str.push_str(") \"/\" ");
            list_str.push_str(&(inbox_re().replace
                              (&re.replace(&abs_dir[..], "INBOX")[..],
                               ""))[..]);
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
    for dir_res in WalkDir::new(&maildir_path) {
        match dir_res {
            Ok(dir) => {
                match list_dir(dir.path(), &regex, &maildir_path) {
                    Some(list_response) => {
                        responses.push(list_response);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    responses
}
