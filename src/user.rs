use std::io::File;

use serialize::json;

use auth::AuthData;
use email::Email;
use error::{
    Error, ImapResult, InternalIoError, SerializationError
};

#[deriving(Decodable, Encodable, Show)]
pub struct User {
    /// The email address through which the user logs in.
    email: Email,
    /// The authentication data the used to verify the user's identity.
    auth_data: AuthData,
    /// The root directory in which the user's mail is stored.
    mail_dir: String
}

impl User {
    /// Creates a new user from a provided email, plaintext password, and root
    /// mail directory.
    pub fn new(email: Email, password: String, mail_dir: String) -> User {
        User {
            email: email,
            auth_data: AuthData::new(password),
            mail_dir: mail_dir
        }
    }
}

pub fn load_users(path_str: String) -> ImapResult<Vec<User>> {
    let path = Path::new(path_str);
    match File::open(&path).read_to_end() {
        Ok(v) => {
            match json::decode(String::from_utf8_lossy(v.as_slice()).as_slice()) {
                Ok(v) => Ok(v),
                Err(err) => Err(Error::simple(SerializationError(err), "Failed to decode users.json."))
            }
        },
        Err(err) => Err(Error::simple(InternalIoError(err), "Failed to read users.json."))
    }
}

pub fn save_users(path_str: String, users: Vec<User>) {
    let path = Path::new(path_str);
    let encoded = json::encode(&users);
    let mut file = File::create(&path);
    file.write(encoded.into_bytes().as_slice()).ok();
}
