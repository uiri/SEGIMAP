//! SEGIMAP is an IMAP server implementation.
#![deny(non_camel_case_types)]
#![cfg_attr(feature = "unstable", feature(test))]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

extern crate bufstream;
extern crate crypto;
#[macro_use]
extern crate log;
#[macro_use]
extern crate nom;
extern crate num;
extern crate rand;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate time;
extern crate toml;
extern crate walkdir;

use server::Server;
use server::lmtp_serve;
use user::Session;

use std::sync::Arc;
use std::thread::spawn;
use bufstream::BufStream;

mod command;
mod error;
mod folder;
mod mime;
mod parser;
#[macro_use]
mod server;
#[macro_use]
mod util;

mod user;
mod message;

fn main() {
    // Create the server. We wrap it so that it is atomically reference
    // counted. This allows us to safely share it across threads

    let serv = match Server::new() {
        Err(e) => {
            error!("Error starting server: {}", e);
            return;
        },
        Ok(s) => Arc::new(s)
    };

    // Spawn a separate thread for listening for LMTP connections
    match serv.lmtp_listener() {
        Err(e) => {
            error!("Error listening on LMTP port: {}", e);
        }
        Ok(v) => {
            let lmtp_serv = serv.clone();
            spawn(move || {
                // let mut acceptor = v.listen();
                // We spawn a separate thread for each LMTP session
                for stream in v.incoming() {
                    match stream {
                        Err(e) => {
                            error!("Error accepting incoming LMTP connection: {}",
                                   e);
                        }
                        Ok(stream) => {
                            let session_serv = lmtp_serv.clone();
                            spawn(move || { lmtp_serve(session_serv, BufStream::new(stream)) });
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
            // let mut acceptor = v.listen();
            // For each IMAP session, we spawn a separate thread.
            for stream in v.incoming() {
                match stream {
                    Err(e) => {
                        error!("Error accepting incoming IMAP connection: {}",
                               e)
                    }
                    Ok(stream) => {
                        let session_serv = serv.clone();
                        spawn(move || {
                            let mut session = Session::new(
                                               BufStream::new(stream),
                                               session_serv);
                            session.handle();
                        });
                    }
                }
            }
            drop(v);
        }
    }
}
