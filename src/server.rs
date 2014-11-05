use std::collections::HashMap;

use std::io::TcpListener;
use std::io::IoResult;

use config::Config;
use email::Email;
use user::User;

pub struct Server {
    conf: Config,
    pub users: HashMap<Email, User>
}

impl Server {
     pub fn new(conf: Config, users: HashMap<Email, User>) -> Server {
         Server {
                conf: conf,
                users: users
         }
     }
     pub fn imap_listener(&self) -> IoResult<TcpListener> {
         return TcpListener::bind(self.conf.host.as_slice(), self.conf.imap_port);
     }

     pub fn lmtp_listener<'r>(&self) -> IoResult<TcpListener> {
         return TcpListener::bind(self.conf.host.as_slice(), self.conf.lmtp_port);
     }
}

