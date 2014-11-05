//! SEGIMAP is an IMAP server implementation.

#![deny(non_camel_case_types)]

extern crate "rust-crypto" as crypto;
extern crate serialize;
extern crate toml;

pub use config::Config;
pub use email::Email;
pub use user::User;

mod auth;
mod config;
mod email;
mod error;
mod user;

/// The file in which user data is stored.
// TODO: add the ability for the user to specify the user data file as an
// argument.
static USER_DATA_FILE: &'static str = "./users.json";

fn main() {
    // Load configuration.
    let config = Config::new();

    // Load user data.
    let mut users: Vec<User> = Vec::new();

    // Save an example user data file.
    users.push(User::new(
        Email {
            local_part: "nikitapekin".to_string(),
            domain_part: "gmail.com".to_string()
        },
        "12345".to_string(),
        "~/.maildir".to_string()
    ));
    users.push(User::new(
        Email {
            local_part: "willpearson".to_string(),
            domain_part: "xqz.ca".to_string()
        },
        "54321".to_string(),
        "~/.maildir".to_string()
    ));
    user::save_users("./users.json.example".to_string(), users);

    // Load the user data from the user data file.
    // TODO: figure out what to do for error handling.
    let users = user::load_users(USER_DATA_FILE.to_string()).unwrap();

    // Avoid unused variable notices temporarily.
    println!("Config: {}", config);
    println!("Users: {}", users);
}
