//! SEGIMAP is an IMAP server implementation.
#![feature(macro_rules)]
#![deny(non_camel_case_types)]
#![feature(phase)]

extern crate "rust-crypto" as crypto;
extern crate regex;
#[phase(plugin)] extern crate regex_macros;
#[phase(plugin, link)] extern crate log;
extern crate serialize;
extern crate toml;

pub use config::Config;
pub use email::Email;
pub use login::LoginData;
pub use message::Message;
pub use server::Server;
pub use session::Session;
pub use user::User;

use std::io::{Listener, Acceptor, BufferedStream};
use std::sync::Arc;

mod auth;
mod config;
mod email;
mod error;
mod folder;
mod login;
mod message;
mod server;
mod session;
mod user;

/// The file in which user data is stored.
// TODO: add the ability for the user to specify the user data file as an
// argument.
static USER_DATA_FILE: &'static str = "./users.json";

fn main() {
    // Load configuration.
    let config = Config::new();

    // Load the user data from the user data file.
    // TODO: figure out what to do for error handling.
    let users = user::load_users(USER_DATA_FILE.to_string()).unwrap();

    let multipart_message = Message::parse(&Path::new("maildir/new/12345:2,FRS")).unwrap();
    let html_message = Message::parse(&Path::new("maildir/cur/54321:2,FS")).unwrap();

    // Avoid unused variable notices temporarily.
    //println!("Config: {}", config);
    //println!("Users: {}", users);
    //println!("Message: {}", multipart_message);
    //println!("Message: {}", html_message);

    let serv = Arc::new(Server::new(config, users));
    match serv.imap_listener() {
        Err(e) => {
            error!("Error listening on IMAP port: {}", e);
        }
        Ok(v) => {
            let mut acceptor = v.listen();
            for stream in acceptor.incoming() {
                match stream {
                    Err(e) => {
                        error!("Error accepting incoming connection: {}", e)
                    }
                    Ok(stream) => {
                        let session_serv = serv.clone();
                        spawn(proc() {
                            let mut session = Session::new(BufferedStream::new(stream), session_serv);
                            session.handle();
                        });
                    }
                }
            }
            drop(acceptor);
        }
    }
}
