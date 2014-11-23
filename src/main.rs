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

use std::io::File;

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

    let filename = "12345";
    let mail_file = File::open(&Path::new(filename)).unwrap().read_to_end().unwrap();
    let multipart_message = Message::parse(filename.to_string(), String::from_utf8_lossy(mail_file.as_slice()).to_string()).unwrap();

    let filename = "54321";
    let mail_file = File::open(&Path::new(filename)).unwrap().read_to_end().unwrap();
    let html_message = Message::parse(filename.to_string(), String::from_utf8_lossy(mail_file.as_slice()).to_string()).unwrap();

    // Avoid unused variable notices temporarily.
    //println!("Config: {}", config);
    //println!("Users: {}", users);
    //println!("Message: {}", multipart_message);
    //println!("Message: {}", html_message);

    let serv = Arc::new(Server::new(config, users));
    match serv.imap_listener() {
        Err(_) => {
            println!("Error listening on IMAP port!");
        }
        Ok(v) => {
            let mut acceptor = v.listen();
            for stream in acceptor.incoming() {
                match stream {
                    Err(e) => {
                        println!("Error accepting incoming connection!")
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
