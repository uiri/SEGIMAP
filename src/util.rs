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

use folder::Folder;

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

    let ok_res = folder.select_response(tag);
    return (Some(folder), ok_res);
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
