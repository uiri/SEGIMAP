// Use OsRng to ensure that the randomly generated data is cryptographically
// secure.
use rand::Rng;
use rand::os::OsRng;

// Use bcrypt for the hashing algorithm to ensure that the outputted data is
// cryptograpically secure and difficult to crack, even if the authentication
// database is leaked.
use crypto::bcrypt_pbkdf::bcrypt_pbkdf;

/// The number of rounds of bcrypt hashing to apply to the password.
static ROUNDS: u32 = 10;

/// Secure representation of the user's password
#[derive(RustcDecodable, RustcEncodable, Debug)]
pub struct AuthData {
    /// Added to the password before hashing
    salt: Vec<u8>,
    /// The hash of the password
    out: Vec<u8>
}

impl AuthData {
    /// Generates a hash and salt for secure storage of a password
    pub fn new(password: String) -> AuthData {
        let salt = gen_salt();
        let password = password.into_bytes();
        // Perform the bcrypt hashing, storing it to an output vector.
        let ref mut out = [0u8; 32];
        bcrypt_pbkdf(&password[..], &salt[..], ROUNDS, out);

        AuthData {
            salt: salt,
            out: out.to_vec()
        }
    }

    /// Verify a password string against the stored auth data to see if it
    /// matches.
    pub fn verify_auth(&self, password: String) -> bool {
        let ref mut out = [0u8; 32];
        bcrypt_pbkdf(
                &password.into_bytes()[..],
                &self.salt[..],
                ROUNDS,
                out);
        self.out == out.to_vec()
    }
}

/// Generate a random salt using the cryptographically secure PRNG provided by
/// the OS, for use with bcrypt hashing.
fn gen_salt() -> Vec<u8> {
    // Use the cryptographically secure OsRng for randomness.
    let mut rng = match OsRng::new() {
        Ok(v) => v,
        Err(e) => panic!("Failed to create secure Rng: {}", e)
    };
    // Generate the salt from a set of random ascii characters.
    let mut salt = String::new();
    for n in rng.gen_ascii_chars().take(16) {
        salt.push(n);
    }
    // Convert the salt into bytes for hashing.
    salt.into_bytes()
}

#[cfg(test)]
mod tests {
    use user::auth;

    #[test]
    fn test_valid_auth_data() {
        let auth_data = auth::AuthData::new("12345".to_string());
        assert!(auth_data.verify_auth("12345".to_string()));
    }

    #[test]
    fn test_invalid_auth_data() {
        let auth_data = auth::AuthData::new("12345".to_string());
        assert!(!auth_data.verify_auth("54321".to_string()));
    }
}
