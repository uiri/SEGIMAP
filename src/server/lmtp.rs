use std::ascii::AsciiExt;
use std::fs::File;
use std::io::{BufRead, Write};
use std::io::ErrorKind::AlreadyExists;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;

use bufstream::BufStream;
use rustc::util::num::ToPrimitive;
use time;

use server::Server;
use user::Email;
use user::User;

macro_rules! return_on_err(
    ($inp:expr) => {
        match $inp {
            Err(_) => { return; }
            _ => {}
        }
    }
);

macro_rules! delivery_ioerror(
    ($res:ident) => ({
        $res.push_str("451 Error in processing.\r\n");
        break;
    })
);

macro_rules! grab_email_token(
    ($arg:expr) => {
        match $arg {
            Some(from_path) => from_path.trim_left_matches('<').trim_right_matches('>'),
            _ => { return None; }
        }
    }
);

struct Lmtp<'a> {
    rev_path: Option<Email>,
    to_path: Vec<&'a User>,
    data: String,
    quit: bool
}

static OK: &'static str = "250 OK\r\n";

impl<'a> Lmtp<'a> {
    fn deliver(&self) -> String {
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
            let newdir_path = Path::new(&maildir[..]).join("new");
            loop {
                match File::create(&newdir_path.join(timestamp.to_string())) {
                    Err(e) => {
                        if e.kind() == AlreadyExists {
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

fn grab_email(arg: Option<&str>) -> Option<Email> {
    let from_path_split = match arg {
        Some(full_from_path) => {
            let mut split_arg = full_from_path.split(':');
            match split_arg.next() {
                Some(from_str) => {
                    match &from_str.to_ascii_lowercase()[..] {
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

pub fn serve(serv: Arc<Server>, mut stream: BufStream<TcpStream>) {
    let mut l = Lmtp {
        rev_path: None,
        to_path: Vec::new(),
        data: String::new(),
        quit: false
    };
    return_on_err!(stream.write(format!("220 {} LMTP server ready\r\n",
                                        *serv.host()).as_bytes()));
    return_on_err!(stream.flush());
    loop {
        let mut command = String::new();
        match stream.read_line(&mut command) {
            Ok(_) => {
                if command.len() == 0 {
                    return;
                }
                let trimmed_command = (&command[..]).trim();
                let mut args = trimmed_command.split(' ');
                let invalid = "500 Invalid command\r\n".to_string();
                let no_such_user = "550 No such user".to_string();
                let data_res = b"354 Start mail input; end with <CRLF>.<CRLF>";
                let ok_res = OK.to_string();
                let res = match args.next() {
                    Some(cmd) => {
                        warn!("LMTP Cmd: {}", trimmed_command);
                        match &cmd.to_ascii_lowercase()[..] {
                            "lhlo" => {
                                match args.next() {
                                    Some(domain) => {
                                        format!("250 {}\r\n", domain)
                                    }
                                    _ => invalid
                                }
                            }
                            "rset" => {
                                l.rev_path = None;
                                l.to_path = Vec::new();
                                ok_res
                            }
                            "noop" => ok_res,
                            "quit" => {
                                l.quit = true;
                                format!("221 {} Closing connection\r\n",
                                        *serv.host())
                            }
                            "vrfy" => {
                                invalid
                            }
                            "mail" => {
                                match grab_email(args.next()) {
                                    None => invalid,
                                    s => {
                                        l.rev_path = s;
                                        ok_res
                                    }
                                }
                            }
                            "rcpt" => {
                                match l.rev_path {
                                    None => invalid,
                                    _ => {
                                        match grab_email(args.next()) {
                                            None => invalid,
                                            Some(email) => {
                                                match serv.users.get(&email) {
                                                    None => no_such_user,
                                                    Some(user) => {
                                                        l.to_path.push(user);
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
                                    let mut data_command = String::new();
                                    match stream.read_line(&mut data_command) {
                                        Ok(_) => {
                                            if data_command.len() == 0 {
                                                break;
                                            }
                                            let data_cmd = (&data_command[..]).trim();
                                            if data_cmd == "." {
                                                loop_res = l.deliver();
                                                l.data = String::new();
                                                break;
                                            }
                                            l.data.push_str(data_cmd);
                                            l.data.push('\n');
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
                if l.quit {
                    return;
                }
            }
            _ => { break; }
        }
    }
}
