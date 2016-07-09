use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::str;

use rustc_serialize::json;

use self::auth::AuthData;
pub use self::email::Email;
use error::{Error, ImapResult};
use error::ErrorKind::{InternalIoError, SerializationError};
pub use self::session::Session;

mod auth;
mod email;
mod login;
mod session;

/// Representation of a User.
#[derive(RustcDecodable, RustcEncodable, Debug)]
pub struct User {
    /// The email address through which the user logs in.
    pub email: Email,
    /// The authentication data the used to verify the user's identity.
    pub auth_data: AuthData,
    /// The root directory in which the user's mail is stored.
    pub maildir: String
}

impl User {
    /// Creates a new user from a provided email, plaintext password, and root
    /// mail directory.
    pub fn new(email: Email, password: String, maildir: String) -> User {
        User {
            email: email,
            auth_data: AuthData::new(password),
            maildir: maildir
        }
    }
}

/// Reads a JSON file and turns it into a HashMap of emails to users.
/// May throw an IoError, hence the Result<> type.
pub fn load_users(path_str: String) -> ImapResult<HashMap<Email, User>> {
    let path = Path::new(&path_str[..]);
    let mut file_buf : Vec<u8> = Vec::new();
    let file = match File::open(&path).unwrap().read_to_end(&mut file_buf) {
        Ok(_) => str::from_utf8(&file_buf[..]).unwrap(),
        Err(err) => return Err(Error::new(InternalIoError(err),
                                          "Failed to read users.json."))
    };
    let users: Vec<User> = match json::decode(file) {
        Ok(v) => v,
        Err(err) => return Err(Error::new(SerializationError(err),
                                          "Failed to decode users.json."))
    };

    let mut map = HashMap::<Email, User>::new();
    for user in users.into_iter() {
        map.insert(user.email.clone(), user);
    }
    Ok(map)
}

/// Writes a list of users to disk
/// Not currently used because IMAP has no provisions for user account
/// management.
#[allow(dead_code)]
pub fn save_users(path_str: String, users: Vec<User>) {
    let path = Path::new(&path_str[..]);
    let encoded = json::encode(&users).unwrap();
    let mut file = File::create(&path).unwrap();
    file.write(&encoded.into_bytes()[..]).ok();
}
