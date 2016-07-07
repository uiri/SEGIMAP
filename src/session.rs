use std::ascii::AsciiExt;
use std::fs;
use std::io::{BufRead, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use bufstream::BufStream;
use regex::Regex;

pub use folder::Folder;
pub use server::Server;

use command::command::Attribute::UID;
use command::sequence_set;
use command::sequence_set::SequenceItem::{
    Number,
    Range,
    Wildcard
};
use error::Error;
use error::ErrorKind::ImapStateError;
use login::LoginData;
use util;

// Just bail if there is some error.
// Used when performing operations on a TCP Stream generally
macro_rules! return_on_err(
    ($inp:expr) => {
        match $inp {
            Err(_) => { return; }
            _ => {}
        }
    }
);

// Used to grab every file for removal while performing DELETE on a folder.
macro_rules! opendirlisting(
    ($inp:expr, $listing:ident, $err:ident, $next:expr) => {
        match fs::read_dir($inp) {
            Err(_) => return $err,
            Ok($listing) => {
                $next
            }
        }
    }
);

// Standard IMAP greeting
static GREET: &'static [u8] = b"* OK Server ready.\r\n";

/// Representation of a session
pub struct Session {
    /// The TCP connection
    stream: BufStream<TcpStream>,
    /// Shared wrapper for config and user data
    serv: Arc<Server>,
    /// Whether to logout and close the connection after interpreting the
    /// latest client command
    logout: bool,
    /// If None, not logged in. If Some(String), the String represents the
    /// logged in user's maildir
    maildir: Option<String>,
    /// If None, no folder selected. Otherwise, contains the currently selected
    /// folder.
    folder: Option<Folder>
}

impl Session {
    pub fn new(stream: BufStream<TcpStream>,
               serv: Arc<Server>) -> Session {
        Session {
            stream: stream,
            serv: serv,
            logout: false,
            maildir: None,
            folder: None
        }
    }

    /// Handles client commands as they come in on the stream and writes
    /// responeses back to the stream.
    pub fn handle(&mut self) {
        // Provide the client with an IMAP greeting.
        return_on_err!(self.stream.write(GREET));
        return_on_err!(self.stream.flush());
        loop {
            let mut command = String::new();
            match self.stream.read_line(&mut command) {
                Ok(_) => {
                    // If the command is empty, exit.
                    // Exitting will close the stream for us.
                    if command.len() == 0 {
                        return;
                    }

                    // Interpret the command and generate a response
                    let res = self.interpret(&command[..]);

                    // Log the response
                    warn!("Response:\n{}", res);

                    return_on_err!(self.stream.write(res.as_bytes()));
                    return_on_err!(self.stream.flush());

                    // Exit if the client is logging out, per RFC 3501
                    if self.logout {
                        return;
                    }
                }

                // If there is an error on the stream, exit.
                Err(_) => { return; }
            }
        }
    }

    /// Interprets a client command and generates a String response
    fn interpret(&mut self, command: &str) -> String {
        let mut args = command.trim().split(' ');

        // The client will need the tag in the response in order to match up
        // the response to the command it issued because the client does not
        // have to wait on our response in order to issue new commands.
        let tag = args.next().unwrap();
        let mut bad_res = tag.to_string();
        bad_res.push_str(" BAD Invalid command\r\n");

        // The argument after the tag specified the command issued.
        // Additional arguments are arguments for that specific command.
        match args.next() {
            Some(cmd) => {
                warn!("Cmd: {}", command.trim());
                match &cmd.to_ascii_lowercase()[..] {
                    "noop" => {
                        let mut res = tag.to_string();
                        res.push_str(" OK NOOP\r\n");
                        return res;
                    }

                    // Inform the client of the supported IMAP version and
                    // extension(s)
                    "capability" => {
                        let mut res = "* CAPABILITY IMAP4rev1 CHILDREN\r\n"
                                       .to_string();
                        res.push_str(tag);
                        res.push_str(" OK Capability successful\r\n");
                        return res;
                    }
                    "login" => {
                        let login_args: Vec<&str> = args.collect();
                        if login_args.len() < 2 { return bad_res; }
                        let email = login_args[0].trim_matches('"');
                        let password = login_args[1].trim_matches('"');
                        let mut no_res  = tag.to_string();
                        no_res.push_str(" NO invalid username or password\r\n");
                        match LoginData::new(email.to_string(),
                                             password.to_string()) {
                            Some(login_data) => {
                                self.maildir = match self.serv.users.get
                                                      (&login_data.email) {
                                    Some(user) => {
                                        if user.auth_data.verify_auth
                                            (login_data.password) {
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
                        return match self.maildir {
                            Some(_) => {
                                let mut res = tag.to_string();
                                res.push_str(" OK logged in successfully as ");
                                res.push_str(email);
                                res.push_str("\r\n");
                                res
                            }
                            None => no_res
                        };
                    }
                    "logout" => {
                        // Close the connection after sending the response
                        self.logout = true;

                        // Write out current state of selected folder (if any)
                        // to disk
                        match self.folder {
                            Some(ref folder) => {
                                folder.expunge();
                            }
                            _ => {}
                        }

                        let mut res = "* BYE Server logging out\r\n"
                                       .to_string();
                        res.push_str(tag);
                        res.push_str(" OK Server logged out\r\n");
                        return res;
                    }
                    // Examine and Select should be nearly identical...
                    "select" => {
                        let maildir = match self.maildir {
                            None => { return bad_res; }
                            Some(ref maildir) => maildir.clone()
                        };
                        let (folder, res) = util::perform_select(&maildir[..],
                                                                 args.collect(),
                                                                false, tag);
                        self.folder = folder;
                        return match self.folder {
                            None => bad_res,
                            _ => res
                        };
                    }
                    "examine" => {
                        let maildir = match self.maildir {
                            None => { return bad_res; }
                            Some(ref maildir) => maildir.clone()
                        };
                        let (folder, res) = util::perform_select(&maildir[..],
                                                                 args.collect(),
                                                                true, tag);
                        self.folder = folder;
                        return match self.folder {
                            None => bad_res,
                            _ => res
                        };
                    }
                    "create" => {
                        let create_args: Vec<&str> = args.collect();
                        if create_args.len() < 1 { return bad_res; }
                        let mbox_name = util::inbox_re().replace
                                         (create_args[0].trim_matches('"'), "");
                        match self.maildir {
                            None => return bad_res,
                            Some(ref maildir) => {
                                let mut no_res = tag.to_string();
                                no_res.push_str(" NO Could not create folder.\r\n");
                                let maildir_path = Path::new(&maildir[..])
                                                    .join(mbox_name);

                                // Create directory for new mail
                                let newmaildir_path = maildir_path.join("new");
                                match fs::create_dir_all(&newmaildir_path) {
                                    Err(_) => return no_res,
                                    _ => {}
                                }
                                match fs::set_permissions(&newmaildir_path,
                                                          fs::Permissions::from_mode(0o755)) {
                                    Err(_) => return no_res,
                                    _ => {}
                                }

                                // Create directory for current mail
                                let curmaildir_path = maildir_path.join("cur");
                                match fs::create_dir_all(&curmaildir_path) {
                                    Err(_) => return no_res,
                                    _ => {}
                                }
                                match fs::set_permissions(&curmaildir_path,
                                                          fs::Permissions::from_mode(0o755)) {
                                    Err(_) => return no_res,
                                    _ => {}
                                }

                                let mut ok_res = tag.to_string();
                                ok_res.push_str(" OK CREATE successful.\r\n");
                                return ok_res;
                            }
                        }
                    }
                    "delete" => {
                        let delete_args: Vec<&str> = args.collect();
                        if delete_args.len() < 1 { return bad_res; }
                        let mbox_name = util::inbox_re().replace
                                         (delete_args[0].trim_matches('"'), "");
                        match self.maildir {
                            None => return bad_res,
                            Some(ref maildir) => {
                                let mut no_res = tag.to_string();
                                no_res.push_str(" NO Invalid folder.\r\n");
                                let maildir_path = Path::new(&maildir[..])
                                                    .join(mbox_name);
                                let newmaildir_path = maildir_path.join("new");
                                let curmaildir_path = maildir_path.join("cur");
                                opendirlisting!(&newmaildir_path, newlist,
                                                no_res,
                                    opendirlisting!(&curmaildir_path, curlist,
                                                    no_res,
                                    {
                                        // Delete the mail in the folder
                                        for file_entry in newlist {
                                            match file_entry {
                                                Ok(file) => {
                                                    match fs::remove_file(file.path()) {
                                                        Err(_) => return no_res,
                                                        _ => {}
                                                    }
                                                }
                                                Err(_) => return no_res
                                            }
                                        }
                                        for file_entry in curlist {
                                            match file_entry {
                                                Ok(file) => {
                                                    match fs::remove_file(file.path()) {
                                                        Err(_) => return no_res,
                                                        _ => {}
                                                    }
                                                }
                                                Err(_) => return no_res
                                            }
                                        }

                                        // Delete the folders holding the mail
                                        match fs::remove_dir(&newmaildir_path) {
                                            Err(_) => return no_res,
                                            _ => {}
                                        }
                                        match fs::remove_dir(&curmaildir_path) {
                                            Err(_) => return no_res,
                                            _ => {}
                                        }

                                        // This folder might contain subfolders
                                        // holding mail. For this reason, we
                                        // leave the other files, and the
                                        // folder itself, in tact.
                                        let mut ok_res = tag.to_string();
                                        ok_res.push_str(" OK DELETE successsful.\r\n");
                                        return ok_res;
                                    })
                                );
                            }
                        }
                    }
                    // List folders which match the specified regular expression.
                    "list" => {
                        let list_args: Vec<&str> = args.collect();
                        if list_args.len() < 2 { return bad_res; }
                        let reference = list_args[0].trim_matches('"');
                        let mailbox_name = list_args[1].trim_matches('"');
                        match self.maildir {
                            None => { return bad_res; }
                            Some(ref maildir) => {
                                if mailbox_name.len() == 0 {
                                    return format!("* LIST (\\Noselect) \"/\" \"{}\"\r\n{} OK List successful\r\n",
                                                   reference, tag);
                                }
                                let mailbox_name = mailbox_name
                                                    .replace("*", ".*")
                                                    .replace("%", "[^/]*");
                                let maildir_path = Path::new(&maildir[..]);
                                let re_opt = Regex::new
                                              (&format!
                                               ("{}/?{}/?{}$",
                                                maildir_path.file_name().unwrap().to_str()
                                                .unwrap(), reference,
                                                mailbox_name.replace
                                                ("INBOX", ""))[..]);
                                match re_opt {
                                    Err(_) => { return bad_res;}
                                    Ok(re) => {
                                        let list_responses = util::list(&maildir[..],
                                                                  re);
                                        let mut ok_res = String::new();
                                        for list_response in list_responses.iter() {
                                            ok_res.push_str(&list_response[..]);
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
                    // Resolve state of folder in memory with state of mail on
                    // disk
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
                    // Close the currently selected folder. Perform all
                    // required cleanup.
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
                    // Delete the messages currently marked for deletion.
                    "expunge" => {
                        match self.expunge() {
                            Err(_) => { return bad_res; }
                            Ok(v) => {
                                let mut ok_res = String::new();
                                for i in v.iter() {
                                    ok_res.push_str("* ");
                                    ok_res.push_str(&i.to_string()[..]);
                                    ok_res.push_str(" EXPUNGE\r\n");
                                }
                                ok_res.push_str(tag);
                                ok_res.push_str(" OK expunge completed\r\n");
                                return ok_res;
                            }
                        }
                    }
                    "fetch" => {
                        // Retrieve the current folder, if it exists.
                        // If it doesn't, the command is invalid.
                        let folder = match self.folder {
                            Some(ref mut folder) => folder,
                            None => return bad_res
                        };

                        // Parse command, make sure it is validly formed.
                        let parsed_cmd = match util::fetch(args) {
                            Ok(cmd) => cmd,
                            _ => return bad_res
                        };

                        /*
                         * Verify that the requested sequence set is valid.
                         *
                         * Per RFC 3501 seq-number definition:
                         * "The server should respond with a tagged BAD
                         * response to a command that uses a message
                         * sequence number greater than the number of
                         * messages in the selected mailbox. This
                         * includes "*" if the selected mailbox is empty."
                         */
                        let sequence_iter = sequence_set::iterator
                                             (&parsed_cmd.sequence_set,
                                              folder.message_count());
                        if sequence_iter.len() == 0 { return bad_res }
                        return util::fetch_loop(parsed_cmd, folder,
                                                      sequence_iter, tag,
                                                      false);
                    },
                    // These commands use UIDs instead of sequence numbers.
                    // Sequence numbers map onto the list of messages in the
                    // folder directly and change whenever messages are added
                    // or removed from the folder.
                    "uid" => {
                        match args.next() {
                            Some(uidcmd) => {
                                match &uidcmd.to_ascii_lowercase()[..] {
                                    "fetch" => {
                                        // Retrieve the current folder, if it
                                        // exists.
                                        let folder = match self.folder {
                                            Some(ref mut folder) => folder,
                                            None => return bad_res
                                        };
                                        // Parse the command with the PEG
                                        // parser.
                                        let mut parsed_cmd = match util::fetch(args) {
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
                                                            0usize
                                                        } else {
                                                            return bad_res;
                                                        }
                                                    }
                                                };
                                                let mut res = String::new();
                                                for index in start..folder.message_count() {
                                                    res.push_str(&folder.fetch(index+1, &parsed_cmd.attributes)[..]);
                                                }
                                                res.push_str(tag);
                                                res.push_str(" OK UID FETCH completed\r\n");
                                                res
                                            }
                                            _ => {
                                                /*
                                                 * Verify that the requested sequence set is valid.
                                                 *
                                                 * Per RFC 3501 seq-number definition:
                                                 * "The server should respond with a tagged BAD
                                                 * response to a command that uses a message
                                                 * sequence number greater than the number of
                                                 * messages in the selected mailbox. This
                                                 * includes "*" if the selected mailbox is empty."
                                                 */
                                                let sequence_iter = sequence_set::uid_iterator(&parsed_cmd.sequence_set);
                                                if sequence_iter.len() == 0 { return bad_res; }
                                                util::fetch_loop(parsed_cmd, folder, sequence_iter, tag, true)
                                            }
                                        };
                                    }
                                    "store" => {
                                        // There should be a folder selected.
                                        let folder = match self.folder {
                                            None => return bad_res,
                                            Some(ref mut folder) => folder
                                        };

                                        match util::store(folder, args.collect(),
                                                    true, tag) {
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
                        // There should be a folder selected.
                        let folder = match self.folder {
                            None => { return bad_res; }
                            Some(ref mut folder) => folder
                        };

                        match util::store(folder, args.collect(), false, tag) {
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
    fn expunge(&self) -> Result<Vec<usize>, Error> {
        match self.folder {
            None => {
                Err(Error::new(ImapStateError, "Not in selected state"))
            }
            Some(ref folder) => {
                Ok(folder.expunge())
            }
        }
    }
}
