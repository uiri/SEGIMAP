//! SEGIMAP is an IMAP server implementation.
#![deny(non_camel_case_types)]
#![cfg_attr(feature = "unstable", feature(test))]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

extern crate bufstream;
extern crate crypto;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate mime;
#[macro_use]
extern crate nom;
extern crate num;
extern crate openssl;
extern crate rand;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate thiserror;
extern crate time;
extern crate toml;
extern crate walkdir;

use crate::server::{imap_serve, lmtp_serve, Server};

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::spawn;

mod command;
mod error;
mod folder;
mod parser;
#[macro_use]
mod util;
#[macro_use]
mod server;
mod message;

fn listen_generic(
    v: TcpListener,
    serv: Arc<Server>,
    prot: &str,
    serve_func: fn(Arc<Server>, TcpStream),
) {
    for stream in v.incoming() {
        match stream {
            Err(e) => {
                error!("Error accepting incoming {} connection: {}", prot, e);
            }
            Ok(stream) => {
                let session_serv = serv.clone();
                spawn(move || serve_func(session_serv, stream));
            }
        }
    }
}

fn listen_lmtp(v: TcpListener, serv: Arc<Server>) {
    listen_generic(v, serv, "LMTP", lmtp_serve);
}

fn listen_imap(v: TcpListener, serv: Arc<Server>) {
    listen_generic(v, serv, "IMAP", imap_serve);
}

fn main() {
    let _ = env_logger::init().unwrap();
    info!("Application started");

    // Create the server. We wrap it so that it is atomically reference
    // counted. This allows us to safely share it across threads

    let serv = match Server::new() {
        Err(e) => {
            error!("Error starting server: {}", e);
            return;
        }
        Ok(s) => Arc::new(s),
    };

    // Spawn a separate thread for listening for LMTP connections
    let lmtp_h = if let Some(lmtp_listener) = serv.lmtp_listener() {
        match lmtp_listener {
            Err(e) => {
                error!("Error listening on LMTP port: {}", e);
                None
            }
            Ok(v) => {
                let lmtp_serv = serv.clone();
                Some(spawn(move || listen_lmtp(v, lmtp_serv)))
            }
        }
    } else {
        None
    };

    let lmtp_ssl_h = if let Some(lmtp_listener) = serv.lmtp_ssl_listener() {
        match lmtp_listener {
            Err(e) => {
                error!("Error listening on LMTP SSL port: {}", e);
                None
            }
            Ok(v) => {
                let lmtp_serv = serv.clone();
                Some(spawn(move || listen_lmtp(v, lmtp_serv)))
            }
        }
    } else {
        None
    };

    // The main thread handles listening for IMAP connections
    let imap_h = if let Some(imap_listener) = serv.imap_listener() {
        match imap_listener {
            Err(e) => {
                error!("Error listening on IMAP port: {}", e);
                None
            }
            Ok(v) => {
                let imap_serv = serv.clone();
                Some(spawn(move || listen_imap(v, imap_serv)))
            }
        }
    } else {
        None
    };

    let imap_ssl_h = if let Some(imap_listener) = serv.imap_ssl_listener() {
        match imap_listener {
            Err(e) => {
                error!("Error listening on IMAP port: {}", e);
                None
            }
            Ok(v) => Some(spawn(move || listen_imap(v, serv))),
        }
    } else {
        None
    };

    if let Some(lh) = lmtp_h {
        return_on_err!(lh.join());
    }

    if let Some(lsh) = lmtp_ssl_h {
        return_on_err!(lsh.join());
    }

    if let Some(ih) = imap_h {
        return_on_err!(ih.join());
    }

    if let Some(ish) = imap_ssl_h {
        return_on_err!(ish.join());
    }
}
