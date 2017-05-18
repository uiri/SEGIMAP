use std::collections::HashMap;
use std::io::Result;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;

use bufstream::BufStream;

use self::config::Config;
use user::{load_users, Email, User};

mod config;
mod lmtp;

/// Holds configuration state and email->user map
pub struct Server {
    conf: Config,
    users: HashMap<Email, User>
}

impl Server {
    pub fn new() -> Server {
        Server::new_with_conf(Config::new())
    }

    /// Create server to hold the Config and User HashMap
    fn new_with_conf(conf: Config) -> Server {
        // Load the user data from the specified user data file.
        let users = load_users(&conf.users).unwrap();

        Server {
            conf: conf,
            users: users
        }
    }

    /// Create a TCP listener on the server host and imap post
    pub fn imap_listener(&self) -> Result<TcpListener> {
        TcpListener::bind((&self.conf.host[..], self.conf.imap_port))
    }

    /// Create a TCP listener on the server host and lmtp port
    pub fn lmtp_listener(&self) -> Result<TcpListener> {
        TcpListener::bind((&self.conf.host[..], self.conf.lmtp_port))
    }

    fn host(&self) -> &String {
        &self.conf.host
    }

    pub fn get_user(&self, email: &Email) -> Option<&User> {
        self.users.get(email)
    }
}

pub fn lmtp_serve(serv: Arc<Server>, stream: BufStream<TcpStream>) {
    lmtp::serve(serv, stream)
}
