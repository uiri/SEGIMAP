//! SEGIMAP is an IMAP server implementation.

#![deny(non_camel_case_types)]

extern crate "rust-crypto" as crypto;
extern crate serialize;
extern crate toml;

pub use config::Config;
pub use conn::ClientConn;
pub use email::Email;
pub use user::User;
pub use server::Server;
pub use login::LoginData;

use std::io::{Listener, Acceptor, BufferedStream};
use std::comm::channel;

mod auth;
mod config;
mod conn;
mod email;
mod error;
mod login;
mod server;
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

    let serv = Server::new(config, users);
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
                        let (req_sendr, req_recvr): (Sender<LoginData>, Receiver<LoginData>) = channel();
                        let (res_sendr, res_recvr): (Sender<Option<String>>, Receiver<Option<String>>) = channel();
                        spawn(proc() {
                            let mut client_conn = ClientConn::new(BufferedStream::new(stream), req_sendr, res_recvr);
                            client_conn.handle();
                        });
                        for msg in req_recvr.iter() {
                            match serv.users.find(&msg.email) {
                                Some(user) => {
                                    if user.auth_data.verify_auth(msg.password) {
                                        res_sendr.send(Some(user.maildir.as_slice().to_string()));
                                    } else {
                                        res_sendr.send(None);
                                    }
                                }
                                None => { res_sendr.send(None); }
                            }
                        }
                    }
                }
            }
            drop(acceptor);
        }
    }
}
