#[deriving(Clone, Decodable, Encodable, Eq, Hash, PartialEq, Show)]
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
        format!("{}@{}", self.local_part, self.domain_part)
    }
}
