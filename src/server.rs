use std::collections::HashMap;

use std::io::Result;
use std::net::TcpListener;

use config::Config;
use email::Email;
use user::{load_users, User};

/// Holds configuration state and email->user map
pub struct Server {
    conf: Config,
    pub users: HashMap<Email, User>
}

impl Server {
    /// Create server to hold the Config and User HashMap
    pub fn new(conf: Config) -> Server {
        // Load the user data from the specified user data file.
        let users = load_users(conf.users.clone()).unwrap();

        Server {
            conf: conf,
            users: users
        }
    }

    /// Create a TCP listener on the server host and imap post
    pub fn imap_listener(&self) -> Result<TcpListener> {
        return TcpListener::bind((&self.conf.host[..], self.conf.imap_port));
    }

    /// Create a TCP listener on the server host and lmtp port
    pub fn lmtp_listener<'r>(&self) -> Result<TcpListener> {
        return TcpListener::bind((&self.conf.host[..], self.conf.lmtp_port));
    }

    /// Return the server's host
    pub fn host(&self) -> &String {
        &self.conf.host
    }
}
