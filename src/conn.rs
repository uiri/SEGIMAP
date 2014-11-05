use std::io::TcpStream;

use user::User;

pub struct ClientConn {
    stream: TcpStream,
    user: Option<User>,
    // folder: Folder
}

impl ClientConn {
     pub fn new(stream: TcpStream) -> ClientConn {
         ClientConn {
             stream: stream,
             user: None
         }
     }
     pub fn handle(&mut self) {
         loop {
              match self.stream.read_to_string() {
                    Ok(command) => {
                        if command.len() == 0 {
                            return;
                        }
                        println!("{}", command);
                    }
                    Err(_) => { return; }
              }
         }
     }
}