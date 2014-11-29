use std::collections::HashMap;
use std::io::File;

use serialize::json;

use auth::AuthData;
use email::Email;
use error::{
    Error, ImapResult, InternalIoError, SerializationError
};

/// Representation of a User.
#[deriving(Decodable, Encodable, Show)]
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
    let path = Path::new(path_str);
    let file = match File::open(&path).read_to_end() {
        Ok(v) => v,
        Err(err) => return Err(Error::new(InternalIoError(err),
                                          "Failed to read users.json."))
    };
    let users: Vec<User> = match json::decode(String::from_utf8_lossy
                                              (file.as_slice()).as_slice()) {
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
    let path = Path::new(path_str);
    let encoded = json::encode(&users);
    let mut file = File::create(&path);
    file.write(encoded.into_bytes().as_slice()).ok();
}
