use std::io::TcpStream;
use std::io::{Buffer, BufferedStream};
use std::str::StrSlice;
use std::ascii::OwnedAsciiExt;
use std::comm::{Sender, Receiver};

use login::LoginData;

static GREET: &'static [u8] = b"* OK Server ready.\r\n";

pub struct Session {
    stream: BufferedStream<TcpStream>,
    sendr: Sender<LoginData>,
    recvr: Receiver<Option<String>>,
    logout: bool,
    maildir: Option<String>,
    // folder: Option<Folder>
}

impl Session {
    pub fn new(stream: BufferedStream<TcpStream>, sendr: Sender<LoginData>, recvr: Receiver<Option<String>>) -> Session {
        Session {
            stream: stream,
            sendr: sendr,
            recvr: recvr,
            logout: false,
            maildir: None
        }
    }

    pub fn handle(&mut self) {
        match self.stream.write(GREET) {
            Err(_) => { return; }
            _ => {}
        }
        match self.stream.flush() {
            Err(_) => { return; }
            _ => {}
        }
        loop {
            match self.stream.read_line() {
                Ok(command) => {
                    if command.len() == 0 {
                        return;
                    }
                    let res = self.interpret(command.as_slice());
                    match self.stream.write(res.as_bytes()) {
                        Err(_) => { return; }
                        _ => {}
                    }
                    match self.stream.flush() {
                        Err(_) => { return; }
                        _ => {}
                    }
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
                                self.sendr.send(login_data);
                                self.maildir = self.recvr.recv();
                                match self.maildir {
                                    Some(_) => {
                                        return format!("{} OK logged in successfully as {}\r\n", tag, email);
                                    }
                                    None => { return no_res; }
                                }
                            }
                            None => { return no_res; }
                        }
                    }
                    "logout" => {
                        self.logout = true;
                        return format!("* BYE Server logging out\r\n{} OK Server logged out\r\n", tag);
                    }
                    _ => { return bad_res; }
                }
            }
            None => {}
        }
        bad_res
    }
}
