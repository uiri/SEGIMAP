use std::ascii::OwnedAsciiExt;
use std::io::{Buffer, BufferedStream, TcpStream, File, PathAlreadyExists};
use std::sync::Arc;
use time;

use email::Email;
pub use server::Server;
use user::User;

macro_rules! return_on_err(
    ($inp:expr) => {
        match $inp {
            Err(_) => { return; }
            _ => {}
        }
    }
)

macro_rules! delivery_ioerror(
    ($res:ident) => ({
        $res.push_str("451 Error in processing.\r\n");
        break;
    })
)
    
macro_rules! grab_email_token(
    ($arg:expr) => {
        match $arg {
            Some(from_path) => from_path.trim_left_chars('<').trim_right_chars('>'),
            _ => { return None; }
        }
    }
)

pub struct Lmtp<'a> {
    serv: Arc<Server>,
    rev_path: Option<Email>,
    to_path: Vec<&'a User>,
    data: String,
    quit: bool
}

static OK: &'static str = "250 OK\r\n";

impl<'a> Lmtp<'a> {
    pub fn new(serv: Arc<Server>) -> Lmtp<'a> {
        Lmtp {
            serv: serv,
            rev_path: None,
            to_path: Vec::new(),
            data: String::new(),
            quit: false
        }
    }

    pub fn handle(&'a mut self, mut stream: BufferedStream<TcpStream>) {
        return_on_err!(stream.write(format!("220 {} LMTP server ready\r\n", *self.serv.host()).as_bytes()));
        return_on_err!(stream.flush());
        loop {
            match stream.read_line() {
                Ok(command) => {
                    if command.len() == 0 {
                        return;
                    }
                    let trimmed_command = command.as_slice().trim();
                    let mut args = trimmed_command.split(' ');
                    let invalid = "500 Invalid command\r\n".to_string();
                    let no_such_user = "550 No such user".to_string();
                    let data_res = b"354 Start mail input; end with <CRLF>.<CRLF>";
                    let ok_res = OK.to_string();
                    let res = match args.next() {
                        Some(cmd) => {
                            warn!("LMTP Cmd: {}", trimmed_command);
                            match cmd.to_string().into_ascii_lower().as_slice() {
                                "lhlo" => {
                                    match args.next() {
                                        Some(domain) => {
                                            format!("250 {}\r\n", domain)
                                        }
                                        _ => invalid
                                    }
                                }
                                "rset" => {
                                    self.rev_path = None;
                                    self.to_path = Vec::new();
                                    ok_res
                                }
                                "noop" => ok_res,
                                "quit" => {
                                    self.quit = true;
                                    format!("221 {} Closing connection\r\n", *self.serv.host())
                                }
                                "vrfy" => {
                                    invalid
                                }
                                "mail" => {
                                    match grab_email(args.next()) {
                                        None => invalid,
                                        s => {
                                            self.rev_path = s;
                                            ok_res
                                        }
                                    }
                                }
                                "rcpt" => {
                                    match self.rev_path {
                                        None => invalid,
                                        _ => {
                                            match grab_email(args.next()) {
                                                None => invalid,
                                                Some(email) => {
                                                    match self.serv.users.find(&email) {
                                                        None => no_such_user,
                                                        Some(user) => {
                                                            self.to_path.push(user);
                                                            ok_res
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                "data" => {
                                    return_on_err!(stream.write(data_res));
                                    return_on_err!(stream.flush());
                                    let mut loop_res = invalid;
                                    loop {
                                        match stream.read_line() {
                                            Ok(data_command) => {
                                                if data_command.len() == 0 {
                                                    break;
                                                }
                                                let data_cmd = data_command.as_slice().trim();
                                                if data_cmd == "." {
                                                    loop_res = self.deliver();
                                                    self.data = String::new();
                                                    break;
                                                }
                                                self.data.push_str(data_cmd);
                                                self.data.push('\n');
                                            }
                                            _ => { break; }
                                        }
                                    }
                                    loop_res
                                }
                                _ => invalid
                            }
                        }
                        None => invalid
                    };
                    return_on_err!(stream.write(res.as_bytes()));
                    return_on_err!(stream.flush());
                    if self.quit {
                        return;
                    }
                }
                _ => { break; }
            }
        }
    }

    pub fn deliver(&self) -> String {
        if self.to_path.len() == 0 {
            return "503 Bad sequence - no recipients".to_string();
        }
        let mut res = String::new();
        for rcpt in self.to_path.iter() {
            let mut timestamp = match time::get_time().sec.to_i32() {
                Some(i) => i,
                None => {
                    res.push_str("555 Unix 2038 error\r\n");
                    break;
                }
            };
            let maildir = rcpt.maildir.clone();
            let newdir_path = Path::new(maildir).join("new");
            loop {
                match File::create(&newdir_path.join(timestamp.to_string())) {
                    Err(e) => {
                        if e.kind == PathAlreadyExists {
                            timestamp += 1;
                        } else {
                            delivery_ioerror!(res);
                        }
                    }
                    Ok(mut file) => {
                        match file.write(self.data.as_bytes()) {
                            Err(_) => {
                                delivery_ioerror!(res);
                            }
                            _ => {}
                        }
                        match file.flush() {
                            Err(_) => {
                                delivery_ioerror!(res);
                            }
                            _ => {}
                        }
                        res.push_str("250 OK\r\n");
                        break;
                    }
                }
            }
        }
        res
    }
}

pub fn grab_email(arg: Option<&str>) -> Option<Email> {
    let from_path_split = match arg {
        Some(full_from_path) => {
            let mut split_arg = full_from_path.split(':');
            match split_arg.next() {
                Some(from_str) => {
                    match from_str.to_string().into_ascii_lower().as_slice() {
                        "from" => {
                            grab_email_token!(split_arg.next())
                        }
                        "to" => {
                            grab_email_token!(split_arg.next())
                        }
                        _ => { return None; }
                    }
                }
                _ => { return None; }
            }
        }
        _ => { return None; }
    };
    let mut from_parts = from_path_split.split('@');
    let local_part = match from_parts.next() {
        Some(part) => part.to_string(),
        _ => { return None; }
    };
    let domain_part = match from_parts.next() {
        Some(part) => part.to_string(),
        _ => { return None; }
    };
    Some(Email::new(local_part, domain_part))
}