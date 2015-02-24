/// Representation of an email
/// This helps ensure the email at least has an '@' in it...
#[derive(Clone, RustcDecodable, RustcEncodable, Eq, Hash, PartialEq, Debug)]
pub struct Email {
    pub local_part: String,
    pub domain_part: String
}

impl Email {
    pub fn new(local_part: String, domain_part: String) -> Email {
        Email {
            local_part: local_part,
            domain_part: domain_part
        }
    }
    pub fn to_string(&self) -> String {
        let mut res = self.local_part.clone();
        res.push('@');
        res.push_str(self.domain_part.as_slice());
        res
    }
}
