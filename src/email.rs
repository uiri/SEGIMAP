#[deriving(Clone, Decodable, Encodable, Eq, Hash, PartialEq, Show)]
pub struct Email {
    pub local_part: String,
    pub domain_part: String
}
