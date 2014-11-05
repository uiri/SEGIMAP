#[deriving(Decodable, Encodable, Show)]
pub struct Email {
    pub local_part: String,
    pub domain_part: String
}
