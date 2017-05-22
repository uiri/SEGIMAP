use std::ascii::AsciiExt;
use std::fs;
use std::io::{BufRead, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::MAIN_SEPARATOR;
use std::str::Split;
use std::sync::Arc;
use bufstream::BufStream;
use regex::Regex;

use folder::Folder;
use server::Server;
use server::Stream;

use command::Attribute::UID;
use command::fetch;
use command::store;
use command::sequence_set;
use command::sequence_set::SequenceItem::{
    Number,
    Range,
    Wildcard
};
use error::Error;
use super::login::LoginData;
use util;

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
    pub fn new(serv: Arc<Server>) -> Session {
        Session {
            serv: serv,
            logout: false,
            maildir: None,
            folder: None
        }
    }

    /// Handles client commands as they come in on the stream and writes
    /// responeses back to the stream.
    pub fn handle(&mut self, orig_stream: TcpStream) {
        let mut stream = BufStream::new(self.serv.imap_ssl(orig_stream));
        // Provide the client with an IMAP greeting.
        return_on_err!(stream.write(GREET));
        return_on_err!(stream.flush());

        let mut command = String::new();
        loop {
            command.truncate(0);
            match stream.read_line(&mut command) {
                Ok(_) => {
                    // If the command is empty, exit.
                    // Exitting will close the stream for us.
                    if command.is_empty() {
                        return;
                    }

                    let mut args = command.trim().split(' ');
                    let inv_str = " BAD Invalid command\r\n";

                    // The client will need the tag in the response in order to match up
                    // the response to the command it issued because the client does not
                    // have to wait on our response in order to issue new commands.
                    let mut starttls = false;
                    let res = match args.next() {
                        None => inv_str.to_string(),
                        Some(tag) => {
                            let mut bad_res = tag.to_string();
                            bad_res.push_str(inv_str);

                            // Interpret the command and generate a response
                            match args.next() {
                                None => bad_res,
                                Some(c) => {
                                    warn!("Cmd: {}", command.trim());
                                    match &c.to_ascii_lowercase()[..] {
                                        // STARTTLS is handled here because it modifies the stream
                                        "starttls" => {
                                            match stream.get_ref() {
                                                &Stream::Tcp(_) =>
                                                    if self.serv.can_starttls() {
                                                        starttls = true;
                                                        let mut ok_res = tag.to_string();
                                                        ok_res.push_str(" OK Begin TLS negotiation now\r\n");
                                                        ok_res
                                                    } else {
                                                        bad_res
                                                    },
                                                _ => bad_res
                                            }
                                        },
                                        cmd => self.interpret(cmd, &mut args, tag, bad_res)
                                    }
                                }
                            }
                        }
                    };

                    // Log the response
                    warn!("Response:\n{}", res);

                    return_on_err!(stream.write(res.as_bytes()));
                    return_on_err!(stream.flush());

                    if starttls {
                        if let Some(ssl_stream) = self.serv.starttls(stream.into_inner()) {
                            stream = BufStream::new(Stream::Ssl(ssl_stream));
                        } else {
                            return;
                        }
                    }

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
    fn interpret(&mut self, cmd: &str, args: &mut Split<char>,
                 tag: &str, bad_res: String) -> String {
        // The argument after the tag specified the command issued.
        // Additional arguments are arguments for that specific command.
        match cmd {
            "noop" => {
                let mut res = tag.to_string();
                res += " OK NOOP\r\n";
                res
            }

            // Inform the client of the supported IMAP version and
            // extension(s)
            "capability" => {
                let mut res = "* CAPABILITY IMAP4rev1 CHILDREN\r\n"
                    .to_string();
                res.push_str(tag);
                res.push_str(" OK Capability successful\r\n");
                res
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
                        self.maildir = match self.serv.get_user(&login_data.email) {
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
                        res
                    }
                    None => no_res
                }
            }
            "logout" => {
                // Close the connection after sending the response
                self.logout = true;

                // Write out current state of selected folder (if any)
                // to disk
                if let Some(ref folder) = self.folder {
                    folder.expunge();
                }

                let mut res = "* BYE Server logging out\r\n"
                    .to_string();
                res.push_str(tag);
                res.push_str(" OK Server logged out\r\n");
                res
            }
            // Examine and Select should be nearly identical...
            "select" => {
                let maildir = match self.maildir {
                    None => { return bad_res; }
                    Some(ref maildir) => maildir.clone()
                };
                let (folder, res) = util::perform_select(&maildir[..],
                                                         &args.collect::<Vec<&str>>(),
                                                         false, tag);
                self.folder = folder;
                match self.folder {
                    None => bad_res,
                    _ => res
                }
            }
            "examine" => {
                let maildir = match self.maildir {
                    None => { return bad_res; }
                    Some(ref maildir) => maildir.clone()
                };
                let (folder, res) = util::perform_select(&maildir[..],
                                                         &args.collect::<Vec<&str>>(),
                                                         true, tag);
                self.folder = folder;
                match self.folder {
                    None => bad_res,
                    _ => res
                }
            }
            "create" => {
                let create_args: Vec<&str> = args.collect();
                if create_args.len() < 1 { return bad_res; }
                let mbox_name = create_args[0].trim_matches('"').replace("INBOX", "");
                match self.maildir {
                    None => bad_res,
                    Some(ref maildir) => {
                        let mut no_res = tag.to_string();
                        no_res.push_str(" NO Could not create folder.\r\n");
                        let maildir_path = Path::new(&maildir[..]).join(mbox_name);

                        // Create directory for new mail
                        let newmaildir_path = maildir_path.join("new");
                        if fs::create_dir_all(&newmaildir_path).is_err() {
                            return no_res;
                        }
                        if fs::set_permissions(&newmaildir_path,
                                               fs::Permissions::from_mode(0o755)).is_err() {
                            return no_res;
                        }

                        // Create directory for current mail
                        let curmaildir_path = maildir_path.join("cur");
                        if fs::create_dir_all(&curmaildir_path).is_err() {
                            return no_res;
                        }
                        if fs::set_permissions(&curmaildir_path,
                                               fs::Permissions::from_mode(0o755)).is_err() {
                            return no_res;
                        }

                        let mut ok_res = tag.to_string();
                        ok_res.push_str(" OK CREATE successful.\r\n");
                        ok_res
                    }
                }
            }
            "delete" => {
                let delete_args: Vec<&str> = args.collect();
                if delete_args.len() < 1 { return bad_res; }
                let mbox_name = delete_args[0].trim_matches('"').replace("INBOX", "");
                match self.maildir {
                    None => bad_res,
                    Some(ref maildir) => {
                        let mut no_res = tag.to_string();
                        no_res.push_str(" NO Invalid folder.\r\n");
                        let maildir_path = Path::new(&maildir[..]).join(mbox_name);
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
                                                                        if fs::remove_file(file.path()).is_err() {
                                                                            return no_res;
                                                                        }
                                                                    }
                                                                    Err(_) => return no_res
                                                                }
                                                            }
                                                            for file_entry in curlist {
                                                                match file_entry {
                                                                    Ok(file) => {
                                                                        if fs::remove_file(file.path()).is_err() {
                                                                            return no_res;
                                                                        }
                                                                    }
                                                                    Err(_) => return no_res
                                                                }
                                                            }

                                                            // Delete the folders holding the mail
                                                            if fs::remove_dir(&newmaildir_path).is_err() {
                                                                return no_res;
                                                            }
                                                            if fs::remove_dir(&curmaildir_path).is_err() {
                                                                return no_res;
                                                            }

                                                            // This folder might contain subfolders
                                                            // holding mail. For this reason, we
                                                            // leave the other files, and the
                                                            // folder itself, in tact.
                                                            let mut ok_res = tag.to_string();
                                                            ok_res.push_str(" OK DELETE successsful.\r\n");
                                                            ok_res
                                                        })
                                        )
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
                    None => bad_res,
                    Some(ref maildir) => {
                        if mailbox_name.is_empty() {
                            return format!("* LIST (\\Noselect) \"/\" \"{}\"\r\n{} OK List successful\r\n",
                                           reference, tag);
                        }
                        let mailbox_name = mailbox_name
                            .replace("*", ".*")
                            .replace("%", "[^/]*");
                        let maildir_path = Path::new(&maildir[..]);
                        let re_opt = Regex::new
                            (&format!
                             ("{}{}?{}{}?{}$",
                              path_filename_to_str!(maildir_path),
                              MAIN_SEPARATOR, reference,
                              MAIN_SEPARATOR,
                              mailbox_name.replace("INBOX", ""))[..]);
                        match re_opt {
                            Err(_) => bad_res,
                            Ok(re) => {
                                let list_responses = util::list(&maildir[..],
                                                                &re);
                                let mut ok_res = String::new();
                                for list_response in &list_responses {
                                    ok_res.push_str(&list_response[..]);
                                    ok_res.push_str("\r\n");
                                }
                                ok_res.push_str(tag);
                                ok_res.push_str(" OK list successful\r\n");
                                ok_res
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
                    None => bad_res,
                    Some(ref mut folder) => {
                        folder.check();
                        let mut ok_res = tag.to_string();
                        ok_res.push_str(" OK Check completed\r\n");
                        ok_res
                    }
                }
            }
            // Close the currently selected folder. Perform all
            // required cleanup.
            "close" => {
                match self.expunge() {
                    Err(_) => bad_res,
                    Ok(_) => {
                        if let Some(ref mut folder) = self.folder {
                            folder.check();
                        }
                        self.folder = None;
                        format!("{} OK close completed\r\n", tag)
                    }
                }
            }
            // Delete the messages currently marked for deletion.
            "expunge" => {
                match self.expunge() {
                    Err(_) => bad_res,
                    Ok(v) => {
                        let mut ok_res = String::new();
                        for i in &v {
                            ok_res.push_str("* ");
                            ok_res.push_str(&i.to_string()[..]);
                            ok_res.push_str(" EXPUNGE\r\n");
                        }
                        ok_res.push_str(tag);
                        ok_res.push_str(" OK expunge completed\r\n");
                        ok_res
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
                let parsed_cmd = match fetch::fetch(args.collect()) {
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
                if sequence_iter.is_empty() { return bad_res }
                fetch::fetch_loop(&parsed_cmd, folder,
                                  &sequence_iter, tag,
                                  false)
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
                                let mut parsed_cmd = match fetch::fetch(args.collect()) {
                                    Ok(cmd) => cmd,
                                    _ => return bad_res
                                };
                                parsed_cmd.attributes.push(UID);

                                // SPECIAL CASE FOR RANGES WITH WILDCARDS
                                if let Range(ref a, ref b) = parsed_cmd.sequence_set[0] {
                                    if let Number(n) = **a {
                                        if let Wildcard = **b {
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
                                            return res
                                        }
                                    }
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
                                let sequence_iter = sequence_set::uid_iterator(&parsed_cmd.sequence_set);
                                if sequence_iter.is_empty() { return bad_res; }
                                fetch::fetch_loop(&parsed_cmd, folder, &sequence_iter, tag, true)
                            }
                            "store" => {
                                // There should be a folder selected.
                                let folder = match self.folder {
                                    None => return bad_res,
                                    Some(ref mut folder) => folder
                                };

                                match store::store(folder, &args.collect::<Vec<&str>>(),
                                                   true, tag) {
                                    Some(res) => res,
                                    _ => bad_res
                                }
                            }
                            _ => bad_res
                        }
                    }
                    None => bad_res
                }
            },
            "store" => {
                // There should be a folder selected.
                let folder = match self.folder {
                    None => { return bad_res; }
                    Some(ref mut folder) => folder
                };

                match store::store(folder, &args.collect::<Vec<&str>>(), false, tag) {
                    Some(res) => res,
                    _ => bad_res
                }
            }
            _ => bad_res
        }
    }

    // should generate list of sequence numbers that were deleted
    fn expunge(&self) -> Result<Vec<usize>, Error> {
        match self.folder {
            None => {
                Err(Error::InvalidImapState)
            }
            Some(ref folder) => {
                Ok(folder.expunge())
            }
        }
    }
}
