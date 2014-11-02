//! SEGIMAP is an IMAP server implementation.

#![deny(non_camel_case_types)]

extern crate "rust-crypto" as crypto;
extern crate serialize;
extern crate toml;

pub use config::Config;

mod config;
mod email;
mod user;

fn main() {
    let config = Config::new();
}
