use std::io::TcpStream;
use std::io::{Buffer, BufferedStream};
use std::str::StrSlice;
use std::ascii::OwnedAsciiExt;
use std::sync::Arc;

use login::LoginData;

pub use folder::Folder;
pub use server::Server;

macro_rules! return_on_err(
    ($inp:expr) => {
        match $inp {
            Err(_) => { return; }
            _ => {}
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
        let bad_res = format!("{} BAD Invalid command\r\n", tag);
        match args.next() {
            Some(cmd) => {
                match cmd.to_string().into_ascii_lower().as_slice() {
                    "login" => {
                        let login_args: Vec<&str> = args.collect();
                        if login_args.len() < 2 { return bad_res; }
                        let email = login_args[0];
                        let password = login_args[1];
                        let no_res  = format!("{} NO invalid username or password\r\n", tag);
                        match LoginData::new(email.to_string(), password.to_string()) {
                            Some(login_data) => {
                                match self.serv.users.find(&login_data.email) {
                                    Some(user) => {
                                        if user.auth_data.verify_auth(login_data.password) {
                                            self.maildir = Some(user.maildir.as_slice().to_string());
                                        } else {
                                            self.maildir = None;
                                        }
                                    }
                                    None => { self.maildir = None; }
                                }
                            }
                            None => { return no_res; }
                        }
                        match self.maildir {
                            Some(_) => {
                                return format!("{} OK logged in successfully as {}\r\n", tag, email);
                            }
                            None => { return no_res; }
                        }
                    }
                    "logout" => {
                        self.logout = true;
                        return format!("* BYE Server logging out\r\n{} OK Server logged out\r\n", tag);
                    }
                    "select" => {
                        let select_args: Vec<&str> = args.collect();
                        if select_args.len() < 1 { return bad_res; }
                        match self.maildir {
                            None => { return bad_res; }
                            _ => {}
                        }
                        let mailbox_name = select_args[0];
                        self.folder =  self.select_mailbox(mailbox_name);
                        match self.folder {
                            None => {
                                return format!("{} NO error finding mailbox", tag);
                            }
                            _ => {}
                        }
                        /* Use self.folder here... */
                        // * Flags
                        // * <n> EXISTS
                        // * <n> RECENT
                        // * OK UNSEEN
                        // * OK PERMANENTFLAGS
                        // * OK UIDNEXT
                        // * OK UIDVALIDITY
                        return format!("{} OK unimplemented", tag);
                    }
                    "create" => {
                        let create_args: Vec<&str> = args.collect();
                        if create_args.len() < 1 { return bad_res; }
                        let mailbox_name = create_args[0];
                        // match magic_mailbox_create(mailbox_name.to_string()) {
                        //     Ok(_) => {
                        //         return format!("{} OK create completed", tag);
                        //     }
                        //     Err(_) => {
                        //         return format!("{} OK create failed", tag);
                        //     }
                        // }
                        return format!("{} OK unimplemented", tag);
                    }
                    "delete" => {
                        let delete_args: Vec<&str> = args.collect();
                        if delete_args.len() < 1 { return bad_res; }
                        let mailbox_name = delete_args[0];
                        // match magic_mailbox_delete(mailbox_name.to_string()) {
                        //     Ok(_) => {
                        //         return format!("{} OK delete completed", tag);
                        //     }
                        //     Err(_) => {
                        //         return format!("{} OK delete failed", tag);
                        //     }
                        // }
                        return format!("{} OK unimplemented", tag);
                    }
                    "close" => {
                        // ignores list of expunge responses
                        self.close();
                        return format!("{} OK close completed", tag);
                    }
                    "expunge" => {
                        // actually uses list of expunge responses
                        self.close();
                        return format!("{} OK expunge completed", tag);
                    }
                    "fetch" => {
                        return format!("{} OK unimplemented", tag);
                    }
                    _ => { return bad_res; }
                }
            }
            None => {}
        }
        bad_res
    }

    // should generate list of sequence numbers that were deleted
    fn close(&self) {
        match self.folder {
            None => {return;}
            _ => {}
        }
        return;
    }

    fn select_mailbox(&self, mailbox_name: &str) -> Option<Folder> {
        match self.maildir {
            None => {return None;}
            Some(ref mdir) => {
                let mbox_name = regex!("INBOX").replace(mailbox_name, ".");
                return Folder::new("name".to_string(), None, Path::new(mdir.as_slice()).join(mbox_name));
            }
        }
    }
}
