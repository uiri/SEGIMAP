use std::io::TcpStream;
use std::io::{Buffer, BufferedStream};
use std::str::StrSlice;
use std::ascii::AsciiStr;

use user::User;

pub struct ClientConn {
    stream: BufferedStream<TcpStream>,
    user: Option<User>,
    // folder: Folder
}

impl ClientConn {
    pub fn new(stream: BufferedStream<TcpStream>) -> ClientConn {
        ClientConn {
            stream: stream,
            user: None
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
        let mut args = command.split(' ');
        match args.nth(0) {
            Some(cmd) if cmd.len() == "login".len() && cmd.to_ascii().eq_ignore_case("login".to_ascii()) => {
                return "OK\n".to_string()
            }
            Some(_) => {}
            None => {}
        }
        "BAD Invalid command\n".to_string()
    }
}
