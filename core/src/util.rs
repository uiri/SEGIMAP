// This file is made up largely of utility methods which are invoked by the
// session in its interpret method. They are separate because they don't rely
// on the session (or take what they do need as arguments) and/or they are
// called by the session in multiple places.

use std::env::current_dir;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use regex::Regex;
use walkdir::WalkDir;

use crate::folder::Folder;

#[macro_export]
macro_rules! path_filename_to_str(
    ($p:ident) => ({
        use std::ffi::OsStr;
        $p.file_name().unwrap_or_else(|| OsStr::new("")).to_str().unwrap_or_else(|| "")
    });
);

fn make_absolute(dir: &Path) -> String {
    match current_dir() {
        Err(_) => dir.display().to_string(),
        Ok(absp) => {
            let mut abs_path = absp.clone();
            abs_path.push(dir);
            abs_path.display().to_string()
        }
    }
}

pub fn perform_select(maildir: &str, select_args: &[&str], examine: bool,
                      tag: &str) -> (Option<Folder>, String) {
    let err_res = (None, "".to_string());
    if select_args.len() < 1 { return err_res; }
    let mbox_name = select_args[0].trim_matches('"').replace("INBOX", ".");
    let mut maildir_path = PathBuf::new();
    maildir_path.push(maildir);
    maildir_path.push(mbox_name);
    let folder = match Folder::new(maildir_path, examine) {
        None => { return err_res; }
        Some(folder) => folder.clone()
    };

    let ok_res = folder.select_response(tag);
    (Some(folder), ok_res)
}

/// For the given dir, make sure it is a valid mail folder and, if it is,
/// generate the LIST response for it.
fn list_dir(dir: &Path, regex: &Regex, maildir_path: &Path) -> Option<String> {
    let dir_string = dir.display().to_string();
    let dir_name = path_filename_to_str!(dir);

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
                if let Ok(subdir) = subdir_entry {
                    if *dir == *maildir_path {
                        break;
                    }
                    let subdir_path = subdir.path();
                    let subdir_str = path_filename_to_str!(subdir_path);
                    if subdir_str != "cur" &&
                        subdir_str != "new" &&
                        subdir_str != "tmp" {
                            if fs::read_dir(&subdir.path().join("cur")).is_err() {
                                continue;
                            }
                            if fs::read_dir(&subdir.path().join("new")).is_err() {
                                continue;
                            }
                            children = true;
                            break;
                        }
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
    match fs::metadata(dir) {
        Err(_) => return None,
        Ok(md) =>
            if !md.is_dir() {
                return None;
            }
    };

    if !regex.is_match(&dir_string[..]) {
        return None;
    }
    let mut list_str = "* LIST (".to_string();
    list_str.push_str(&flags[..]);
    list_str.push_str(") \"/\" ");
    let list_dir_string = if abs_dir.starts_with(&re_path[..]) {
        abs_dir.replacen(&re_path[..], "", 1)
    } else {
        abs_dir
    };
    list_str.push_str(&(list_dir_string.replace("INBOX", ""))[..]);
    Some(list_str)
}

/// Go through the logged in user's maildir and list every folder matching
/// the given regular expression. Returns a list of LIST responses.
pub fn list(maildir: &str, regex: &Regex) -> Vec<String> {
    let maildir_path = Path::new(maildir);
    let mut responses = Vec::new();
    if let Some(list_response) = list_dir(maildir_path, regex, maildir_path) {
        responses.push(list_response);
    }
    for dir_res in WalkDir::new(&maildir_path) {
        if let Ok(dir) = dir_res {
            if let Some(list_response) = list_dir(dir.path(), regex, maildir_path) {
                responses.push(list_response);
            }
        }
    }
    responses
}
