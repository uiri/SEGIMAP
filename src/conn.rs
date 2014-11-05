use std::io::TcpStream;
use std::io::{Buffer, BufferedStream};
use std::str::StrSlice;
use std::ascii::AsciiStr;
use std::comm::{Sender, Receiver};

use login::LoginData;

pub struct ClientConn {
    stream: BufferedStream<TcpStream>,
    sendr: Sender<LoginData>,
    recvr: Receiver<Option<String>>,
    maildir: Option<String>,
    // folder: Option<Folder>
}

impl ClientConn {
    pub fn new(stream: BufferedStream<TcpStream>, sendr: Sender<LoginData>, recvr: Receiver<Option<String>>) -> ClientConn {
        ClientConn {
            stream: stream,
            sendr: sendr,
            recvr: recvr,
            maildir: None
        }
    }
    pub fn handle(&mut self) {
        loop {
            match self.stream.read_line() {
                Ok(command) => {
                    if command.len() == 0 {
                        return;
                    }
                    let res = self.interpret(command.as_slice());
                    match self.stream.write(res.as_bytes()) {
                        Ok(_) => {}
                        Err(_) => { return; }
                    }
                    match self.stream.flush() {
                        Ok(_) => {}
                        Err(_) => { return; }
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
            Some(cmd) if cmd.len() == "login".len() && cmd.to_ascii().eq_ignore_case("login".to_ascii()) => {
                match args.next() {
                    Some(email) => {
                        match args.next() {
                            Some(password) => {
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
                            None => { return bad_res; }
                        }
                    }
                    None => { return bad_res; }
                }
            }
            Some(_) => {}
            None => {}
        }
        bad_res
    }
}
