use crate::error::ImapResult;
use self::auth::AuthData;
use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::str;

pub use self::email::Email;
pub use self::login::LoginData;

mod auth;
mod email;
mod login;

/// Representation of a User.
#[derive(Debug, Deserialize, Serialize)]
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

/// Reads a JSON file and turns it into a `HashMap` of emails to users.
/// May throw an `std::io::Error`, hence the `Result<>` type.
pub fn load_users(path_str: &str) -> ImapResult<HashMap<Email, User>> {
    let path = Path::new(&path_str[..]);

    let users = match File::open(&path) {
        Ok(mut file) => {
            let mut file_buf: String = String::new();
            file.read_to_string(&mut file_buf)?;
            serde_json::from_str(&file_buf)?
        },
        Err(e) => {
            warn!("Failed to open users file, creating default: {}", e);
            create_default_users(&path)?
        }
    };

    let mut map = HashMap::<Email, User>::new();
    for user in users {
        map.insert(user.email.clone(), user);
    }

    Ok(map)
}

/// Writes a list of users to a new file on the disk.
pub fn save_users(path: &Path, users: &[User]) -> ImapResult<()> {
    let encoded = serde_json::to_string(&users)?;

    let mut file = File::create(&path)?;
    file.write(encoded.as_bytes())?;

    Ok(())
}

/// Function to create an example users JSON file at the specified path.
///
/// Returns the list of example users.
fn create_default_users(path: &Path) -> ImapResult<Vec<User>> {
    let users = vec![
        User::new(
            Email::new("will".to_string(), "xqz.ca".to_string()),
            "54321".to_string(),
            "./maildir".to_string()
        ),
        User::new(
            Email::new("nikitapekin".to_string(), "gmail.com".to_string()),
            "12345".to_string(),
            "./maildir".to_string()
        ),
    ];

    save_users(path, &users)?;

    Ok(users)
}
