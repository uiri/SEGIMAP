use std::collections::HashSet;
use std::io::{Buffer, BufferedStream, FilePermission, fs, TcpStream};
use std::io::fs::PathExtensions;
use std::os::make_absolute;
use std::str::{from_utf8, StrSlice, CharSplits};
use std::ascii::OwnedAsciiExt;
use std::sync::Arc;
use regex::Regex;

pub use folder::Folder;
pub use server::Server;

use command::command::{Command, UID};
use command::sequence_set;
use command::sequence_set::{
    Number,
    Range,
    Wildcard
};
use error::{Error,ImapStateError};
use login::LoginData;
use parser::fetch;

use message;
use message::{Flag, Add, Seen};
use command::command::BodySection;

macro_rules! return_on_err(
    ($inp:expr) => {
        match $inp {
            Err(_) => { return; }
            _ => {}
        }
    }
)

macro_rules! opendirlisting(
    ($inp:expr, $listing:ident, $err:ident, $next:expr) => {
        match fs::readdir($inp) {
            Err(_) => return $err,
            Ok($listing) => {
                $next
            }
        }
    }
)

static GREET: &'static [u8] = b"* OK Server ready.\r\n";

pub struct Session {
    stream: BufferedStream<TcpStream>,
    serv: Arc<Server>,
    logout: bool,
    maildir: Option<String>,
    folder: Option<Folder>
}

impl Session {
    pub fn new(stream: BufferedStream<TcpStream>, serv: Arc<Server>) -> Session {
        Session {
            stream: stream,
            serv: serv,
            logout: false,
            maildir: None,
            folder: None
        }
    }

    pub fn handle(&mut self) {
        return_on_err!(self.stream.write(GREET));
        return_on_err!(self.stream.flush());
        loop {
            match self.stream.read_line() {
                Ok(command) => {
                    if command.len() == 0 {
                        return;
                    }
                    let res = self.interpret(command.as_slice());
                    warn!("Response:\n{}", res);
                    return_on_err!(self.stream.write(res.as_bytes()));
                    return_on_err!(self.stream.flush());
                    if self.logout {
                        return;
                    }
                }
                Err(_) => { return; }
            }
        }
    }

    fn interpret(&mut self, command: &str) -> String {
        let mut args = command.trim().split(' ');
        let tag = args.next().unwrap();
        let mut bad_res = tag.to_string();
        bad_res.push_str(" BAD Invalid command\r\n");
        match args.next() {
            Some(cmd) => {
                warn!("Cmd: {}", command.trim());
                match cmd.to_string().into_ascii_lower().as_slice() {
                    "noop" => {
                        let mut res = tag.to_string();
                        res.push_str(" OK NOOP\r\n");
                        return res;
                    }
                    "capability" => {
                        let mut res = "* CAPABILITY IMAP4rev1 CHILDREN\r\n".to_string();
                        res.push_str(tag);
                        res.push_str(" OK Capability successful\r\n");
                        return res;
                    }
                    "login" => {
                        let login_args: Vec<&str> = args.collect();
                        if login_args.len() < 2 { return bad_res; }
                        let email = login_args[0].trim_chars('"');
                        let password = login_args[1].trim_chars('"');
                        let mut no_res  = tag.to_string();
                        no_res.push_str(" NO invalid username or password\r\n");
                        match LoginData::new(email.to_string(), password.to_string()) {
                            Some(login_data) => {
                                self.maildir = match self.serv.users.find(&login_data.email) {
                                    Some(user) => {
                                        if user.auth_data.verify_auth(login_data.password) {
                                            Some(user.maildir.clone())
                                        } else {
                                            None
                                        }
                                    }
                                    None => None
                                }
                            }
                            None => { return no_res; }
                        }
                        match self.maildir {
                            Some(_) => {
                                let mut res = tag.to_string();
                                res.push_str(" OK logged in successfully as ");
                                res.push_str(email);
                                res.push_str("\r\n");
                                return res;
                            }
                            None => { return no_res; }
                        }
                    }
                    "logout" => {
                        self.logout = true;
                        match self.folder {
                            Some(ref folder) => {
                                folder.expunge();
                            }
                            _ => {}
                        }
                        let mut res = "* BYE Server logging out\r\n".to_string();
                        res.push_str(tag);
                        res.push_str(" OK Server logged out\r\n");
                        return res;
                    }
                    // Examine and Select should be nearly identical...
                    "select" => {
                        return self.perform_select(args.collect(), false, bad_res, tag);
                    }
                    "examine" => {
                        return self.perform_select(args.collect(), true, bad_res, tag);
                    }
                    "create" => {
                        let create_args: Vec<&str> = args.collect();
                        if create_args.len() < 1 { return bad_res; }
                        let mailbox_name = create_args[0].trim_chars('"'); // ");
                        let mbox_name = regex!("INBOX").replace(mailbox_name, "");
                        match self.maildir {
                            None => return bad_res,
                            Some(ref maildir) => {
                                let mut no_res = tag.to_string();
                                no_res.push_str(" NO Could not create folder.\r\n");
                                let maildir_path = Path::new(maildir.as_slice()).join(mbox_name);
                                let newmaildir_path = maildir_path.join("new");
                                let curmaildir_path = maildir_path.join("cur");
                                let file_perms = FilePermission::from_bits_truncate(0o755);
                                match fs::mkdir_recursive(&newmaildir_path, file_perms) {
                                    Err(_) => return no_res,
                                    _ => {}
                                }
                                match fs::mkdir_recursive(&curmaildir_path, file_perms) {
                                    Err(_) => return no_res,
                                    _ => {}
                                }
                                let mut ok_res = tag.to_string();
                                ok_res.push_str(" OK CREATE successful.\r\n");
                                return ok_res;
                            }
                        }
                    }
                    // rename
                    "delete" => {
                        let delete_args: Vec<&str> = args.collect();
                        if delete_args.len() < 1 { return bad_res; }
                        let mailbox_name = delete_args[0].trim_chars('"'); // ");
                        let mbox_name = regex!("INBOX").replace(mailbox_name, "");
                        match self.maildir {
                            None => return bad_res,
                            Some(ref maildir) => {
                                let mut no_res = tag.to_string();
                                no_res.push_str(" NO Invalid folder.\r\n");
                                let maildir_path = Path::new(maildir.as_slice()).join(mbox_name);
                                let newmaildir_path = maildir_path.join("new");
                                let curmaildir_path = maildir_path.join("cur");
                                opendirlisting!(&newmaildir_path, newlist, no_res,
                                    opendirlisting!(&curmaildir_path, curlist, no_res,
                                   {
                                       for file in newlist.iter() {
                                           match fs::unlink(file) {
                                               Err(_) => return no_res,
                                               _ => {}
                                           }
                                       }
                                       for file in curlist.iter() {
                                           match fs::unlink(file) {
                                               Err(_) => return no_res,
                                               _ => {}
                                           }
                                       }
                                       match fs::rmdir(&newmaildir_path) {
                                           Err(_) => return no_res,
                                           _ => {}
                                       }
                                       match fs::rmdir(&curmaildir_path) {
                                           Err(_) => return no_res,
                                           _ => {}
                                       }
                                       let mut ok_res = tag.to_string();
                                       ok_res.push_str(" OK DELETE successsful.\r\n");
                                       return ok_res;
                                   })
                                );
                            }
                        }
                    }
                    "list" => {
                        let list_args: Vec<&str> = args.collect();
                        if list_args.len() < 2 { return bad_res; }
                        let reference = list_args[0].trim_chars('"');
                        let mailbox_name = list_args[1].trim_chars('"');
                        match self.maildir {
                            None => { return bad_res; }
                            Some(ref maildir) => {
                                if mailbox_name.len() == 0 {
                                    return format!("* LIST (\\Noselect) \"/\" \"{}\"\r\n{} OK List successful\r\n", reference, tag);
                                }
                                let mailbox_name = mailbox_name.replace("*", ".*").replace("%", "[^/]*");
                                let maildir_path = Path::new(maildir.as_slice());
                                let re_opt = Regex::new(format!("{}/?{}/?{}$", from_utf8(maildir_path.filename().unwrap()).unwrap(), reference, mailbox_name.replace("INBOX", "")).as_slice());
                                match re_opt {
                                    Err(_) => { return bad_res;}
                                    Ok(re) => {
                                        let list_responses = self.list(re);
                                        let mut res_iter = list_responses.iter();
                                        let mut ok_res = String::new();
                                        for list_response in res_iter {
                                            ok_res.push_str(list_response.as_slice());
                                            ok_res.push_str("\r\n");
                                        }
                                        ok_res.push_str(tag);
                                        ok_res.push_str(" OK list successful\r\n");
                                        return ok_res;
                                    }
                                }
                            }
                        }
                    }
                    "check" => {
                        match self.expunge() {
                            _ => {}
                        }
                        match self.folder {
                            None => return bad_res,
                            Some(ref mut folder) => {
                                folder.check();
                                let mut ok_res = tag.to_string();
                                ok_res.push_str(" OK Check completed\r\n");
                                return ok_res;
                            }
                        }
                    }
                    "close" => {
                        return match self.expunge() {
                            Err(_) => bad_res,
                            Ok(_) => {
                                match self.folder {
                                    Some(ref mut folder) => {
                                        folder.check();
                                    }
                                    _ => {}
                                }
                                self.folder = None;
                                format!("{} OK close completed\r\n", tag)
                            }
                        };
                    }
                    "expunge" => {
                        match self.expunge() {
                            Err(_) => { return bad_res; }
                            Ok(v) => {
                                let mut ok_res = String::new();
                                for i in v.iter() {
                                    ok_res.push_str("* ");
                                    ok_res.push_str(i.to_string().as_slice());
                                    ok_res.push_str(" EXPUNGE\r\n");
                                }
                                ok_res.push_str(tag);
                                ok_res.push_str(" OK expunge completed\r\n");
                                return ok_res;
                            }
                        }
                    }
                    "fetch" => {
                        let folder = match self.folder {
                            Some(ref mut folder) => folder,
                            None => return bad_res
                        };

                        let parsed_cmd = match collate_and_parse_fetch(args) {
                            Ok(cmd) => cmd,
                            _ => return bad_res
                        };
                        // Retrieve the current folder, if it exists.
                        /*
                         * Verify that the requested sequence set is valid.
                         *
                         * Per FRC 3501 seq-number definition:
                         * "The server should respond with a tagged BAD
                         * response to a command that uses a message
                         * sequence number greater than the number of
                         * messages in the selected mailbox. This
                         * includes "*" if the selected mailbox is empty."
                         */
                        let sequence_iter = sequence_set::iterator(&parsed_cmd.sequence_set, folder.message_count());
                        if sequence_iter.len() == 0 { return bad_res }
                        return normal_fetch_iter_loop(parsed_cmd, folder, sequence_iter, tag, false);
                    },
                    "uid" => {
                        match args.next() {
                            Some(uidcmd) => {
                                match uidcmd.to_string().into_ascii_lower().as_slice() {
                                    "fetch" => {
                                        // Retrieve the current folder, if it exists.
                                        let folder = match self.folder {
                                            Some(ref mut folder) => folder,
                                            None => return bad_res
                                        };
                                        // Parse the command with the PEG parser.
                                        let mut parsed_cmd = match collate_and_parse_fetch(args) {
                                            Ok(cmd) => cmd,
                                            _ => return bad_res
                                        };
                                        parsed_cmd.attributes.push(UID);

                                        // SPECIAL CASE FOR RANGES WITH WILDCARDS
                                        return match parsed_cmd.sequence_set[0] {
                                            Range(box Number(n), box Wildcard) => {
                                                if folder.message_count() == 0 { return bad_res }
                                                let start = match folder.get_index_from_uid(&n) {
                                                    Some(start) => *start,
                                                    None => {
                                                        if n == 1 {
                                                            0u
                                                        } else {
                                                            return bad_res;
                                                        }
                                                    }
                                                };
                                                let mut res = String::new();
                                                for index in range(start, folder.message_count()) {
                                                    res.push_str(folder.fetch(index+1, &parsed_cmd.attributes).as_slice());
                                                }
                                                res.push_str(tag);
                                                res.push_str(" OK UID FETCH completed\r\n");
                                                res
                                            }
                                            _ => {
                                                /*
                                                 * Verify that the requested sequence set is valid.
                                                 *
                                                 * Per FRC 3501 seq-number definition:
                                                 * "The server should respond with a tagged BAD
                                                 * response to a command that uses a message
                                                 * sequence number greater than the number of
                                                 * messages in the selected mailbox. This
                                                 * includes "*" if the selected mailbox is empty."
                                                 */
                                                let sequence_iter = sequence_set::uid_iterator(&parsed_cmd.sequence_set);
                                                if sequence_iter.len() == 0 { return bad_res; }
                                                normal_fetch_iter_loop(parsed_cmd, folder, sequence_iter, tag, true)
                                            }
                                        };
                                    }
                                    "store" => {
                                        match self.store(args.collect(), true, tag) {
                                            Some(res) => return res,
                                            _ => return bad_res
                                        }
                                    }
                                    _ => { return bad_res; }
                                }
                            }
                            None => { return bad_res; }
                        }
                    },
                    "store" => {
                        match self.store(args.collect(), false, tag) {
                            Some(res) => return res,
                            _ => return bad_res
                        }
                    }
                    _ => { return bad_res; }
                }
            }
            None => {}
        }
        bad_res
    }

    // should generate list of sequence numbers that were deleted
    fn expunge(&self) -> Result<Vec<uint>, Error> {
        match self.folder {
            None => { Err(Error::new(ImapStateError, "Not in selected state")) }
            Some(ref folder) => {
                Ok(folder.expunge())
            }
        }
    }

    fn select_mailbox(&mut self, mailbox_name: &str, examine: bool) {
        let mbox_name = regex!("INBOX").replace(mailbox_name, ".");
        match self.maildir {
            None => {}
            Some(ref maildir) => {
                let maildir_path = Path::new(maildir.as_slice()).join(mbox_name);
                // TODO: recursively grab parent...
                self.folder = Folder::new(maildir_path, examine)
                    // TODO: Insert new folder into folder service
                    // folder_service.insert(mailbox_name.to_string(), box *folder);
            }
        }
    }

    fn perform_select(&mut self, select_args: Vec<&str>, examine: bool, bad_res: String, tag: &str) -> String {
        if select_args.len() < 1 { return bad_res; }
        let mailbox_name = select_args[0].trim_chars('"'); // "
        match self.maildir {
            None => { return bad_res; }
            _ => {}
        }
        self.select_mailbox(mailbox_name, examine);
        match self.folder {
            None => {
                return format!("{} NO error finding mailbox\r\n", tag);
            }
            Some(ref folder) => {
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
                ok_res.push_str(format!("{} OK {} SELECT command was successful\r\n", tag, folder.read_status()).as_slice());
                return ok_res;
            }
        }
    }

    fn list_dir(&self, dir: Path, regex: &Regex, maildir_path: &Path) -> Option<String> {
        let dir_string = format!("{}", dir.display());
        let dir_name = from_utf8(dir.filename().unwrap()).unwrap();
        if  dir_name == "cur" || dir_name == "new" || dir_name == "tmp" {
            return None;
        }
        let abs_dir = make_absolute(&dir);
        let mut flags = match fs::readdir(&dir.join("cur")) {
            Err(_) => "\\Noselect".to_string(),
            _ => {
                match fs::readdir(&dir.join("new")) {
                    Err(_) => "\\Noselect".to_string(),
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
        flags = match fs::readdir(&dir) {
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
                    format!("{} \\HasChildren", flags)
                } else {
                    format!("{} \\HasNoChildren", flags)
                }
            }
        };
        let re_path = make_absolute(maildir_path);
        let re_opt = Regex::new(format!("^{}", re_path.display()).as_slice());
        match re_opt {
            Err(_) => { return None; }
            Ok(re) => {
                let list_str = format!("* LIST ({}) \"/\" {}", flags, regex!("INBOX/").replace(re.replace(format!("{}", abs_dir.display()).as_slice(), "INBOX").as_slice(), ""));
                if dir.is_dir() && regex.is_match(dir_string.as_slice()) {
                    return Some(list_str);
                }
                return None;
            }
        }
    }

    fn list(&self, regex: Regex) -> Vec<String> {
        match self.maildir {
            None => Vec::new(),
            Some(ref maildir) => {
                let maildir_path = Path::new(maildir.as_slice());
                let mut responses = Vec::new();
                match self.list_dir(maildir_path.clone(), &regex, &maildir_path) {
                    Some(list_response) => {
                        responses.push(list_response);
                    }
                    _ => {}
                }
                match fs::walk_dir(&maildir_path) {
                    Err(_) => Vec::new(),
                    Ok(mut dir_list) => {
                        for dir in dir_list {
                            match self.list_dir(dir, &regex, &maildir_path) {
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
        }
    }

    fn store(&mut self, store_args: Vec<&str>, seq_uid: bool, tag: &str) -> Option<String> {
        if store_args.len() < 3 { return None; }
        let sequence_set_opt = sequence_set::parse(store_args[0].trim_chars('"'));
        let data_name = store_args[1].trim_chars('"');
        let data_value = store_args[2].trim_chars('"'); // "));
        let data_name_lower = data_name.to_string().into_ascii_lower();
        let mut data_name_parts = data_name_lower.as_slice().split('.');
        let flag_part = data_name_parts.next();
        let silent_part = data_name_parts.next();
        match data_name_parts.next() {
            Some(_) => return None,
            _ => {}
        }
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
        let flag_name = match message::parse_storename(flag_part) {
            Some(storename) => storename,
            None => return None
        };
        let mut flags: HashSet<Flag> = HashSet::new();
        for flag in data_value.trim_chars('(').trim_chars(')').split(' ') {
            match message::parse_flag(flag) {
                None => { continue; }
                Some(insert_flag) => { flags.insert(insert_flag); }
            }
        }
        match self.folder {
            None => return None,
            Some(ref mut folder) => {
                match sequence_set_opt {
                    None => return None,
                    Some(sequence_set) => {
                        let sequence_iter = sequence_set::uid_iterator(&sequence_set);
                        return Some(format!("{}{} OK STORE complete\r\n", folder.store(sequence_iter, flag_name, silent, flags, seq_uid), tag));
                    }
                }
            }
        }
    }

}

fn collate_and_parse_fetch(mut args: CharSplits<char>) -> Result<Command,String> {
    let mut cmd = "FETCH".to_string();
    for arg in args {
        cmd.push(' ');
        cmd.push_str(arg);
    }

    // Parse the command with the PEG parser.
   fetch(cmd.as_slice())
}

fn normal_fetch_iter_loop(parsed_cmd: Command, folder: &mut Folder, sequence_iter: Vec<uint>, tag: &str, uid: bool) -> String {
    for attr in parsed_cmd.attributes.iter() {
        match attr {
            &BodySection(_, _) => {
                let mut seen_flag_set = HashSet::new();
                seen_flag_set.insert(Seen);
                folder.store(sequence_iter.clone(), Add, true, seen_flag_set, false);
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
