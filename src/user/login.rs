use super::email::Email;

/// Representation of an email and password login attempt.
pub struct LoginData {
    pub email: Email,
    pub password: String
}

impl LoginData {
    pub fn new(email: String, password: String) -> Option<LoginData> {
        let mut parts = (&email[..]).split('@');
        if let Some(local_part) = parts.next() {
            if let Some(domain_part) = parts.next() {
                let login_data = LoginData {
                    email: Email {
                        local_part: local_part.to_string(),
                        domain_part: domain_part.to_string()
                    },
                    password: password
                };
                return Some(login_data);
            }
        }

        None
    }
}
