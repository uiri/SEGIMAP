//! SEGIMAP is an IMAP server implementation.
#![deny(non_camel_case_types)]
#![feature(
    box_patterns,
    default_type_params,
    macro_rules,
    plugin,
    regex_macros
)]
#![plugin(peg_syntax_ext, regex_macros)]

extern crate crypto;
#[macro_use] extern crate log;
extern crate peg_syntax_ext;
extern crate regex;
extern crate regex_macros;
extern crate serialize;
extern crate time;
extern crate toml;

pub use config::Config;
pub use email::Email;
pub use lmtp::Lmtp;
pub use login::LoginData;
pub use message::Message;
pub use server::Server;
pub use session::Session;
pub use user::User;

use std::io::{Listener, Acceptor, BufferedStream};
use std::sync::Arc;

mod auth;
mod command;
mod config;
mod email;
mod error;
mod folder;
mod lmtp;
mod login;
mod message;
mod parser;
mod server;
mod session;
mod user;
mod util;

fn main() {
    // Load configuration.
    let config = Config::new();

    // This function only needs to be run once, really.
    // create_default_users(config.users.clone());

    // Create the server. We wrap it so that it is atomically reference
    // counted. This allows us to safely share it across threads
    let serv = Arc::new(Server::new(config));

    // Spawn a separate thread for listening for LMTP connections
    match serv.lmtp_listener() {
        Err(e) => {
            error!("Error listening on LMTP port: {}", e);
        }
        Ok(v) => {
            let lmtp_serv = serv.clone();
            spawn(move || {
                let mut acceptor = v.listen();
                // We spawn a separate thread for each LMTP session
                for stream in acceptor.incoming() {
                    match stream {
                        Err(e) => {
                            error!("Error accepting incoming LMTP connection: {}",
                                   e);
                        }
                        Ok(stream) => {
                            let session_serv = lmtp_serv.clone();
                            spawn(move || {
                                let mut lmtp = Lmtp::new(session_serv);
                                lmtp.handle(BufferedStream::new(stream));
                            });
                        }
                    }
                }
            });
        }
    }

    // The main thread handles listening for IMAP connections
    match serv.imap_listener() {
        Err(e) => {
            error!("Error listening on IMAP port: {}", e);
        }
        Ok(v) => {
            let mut acceptor = v.listen();
            // For each IMAP session, we spawn a separate thread.
            for stream in acceptor.incoming() {
                match stream {
                    Err(e) => {
                        error!("Error accepting incoming IMAP connection: {}",
                               e)
                    }
                    Ok(stream) => {
                        let session_serv = serv.clone();
                        spawn(move || {
                            let mut session = Session::new(
                                               BufferedStream::new(stream),
                                               session_serv);
                            session.handle();
                        });
                    }
                }
            }
            drop(acceptor);
        }
    }
}

/// Function to create our default users.json for testing
#[allow(dead_code)]
fn create_default_users(filename: String) {
    let mut users = Vec::new();
    users.push(User::new(Email::new("will".to_string(), "xqz.ca".to_string()),
                         "54321".to_string(), "./maildir".to_string()));
    users.push(User::new(Email::new("nikitapekin".to_string(),
                                    "gmail.com".to_string()),
                         "12345".to_string(), "./maildir".to_string()));
    user::save_users(filename, users);
}
