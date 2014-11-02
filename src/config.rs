use std::io::File;

use serialize::Encodable;
use toml::{decode_str, encode_str};

#[deriving(Decodable, Encodable, Show)]
pub struct Config {
    host: String,
    lmtp_port: u16,
    imap_port: u16,
    uid: String
}

impl Config {
    pub fn new() -> Config {
        let path = Path::new("./config.toml");
        match File::open(&path).read_to_end() {
            Ok(v) => {
                match decode_str(String::from_utf8_lossy(v.as_slice()).as_slice()) {
                    Some(v) => v,
                    None => {
                        println!("Failed to parse config.toml.\nUsing default values.");
                        default_config()
                    }
                }
            },
            Err(_) => {
                println!("Failed to read config.toml; creating from defaults.");
                let config = default_config();
                let encoded = encode_str(&config);
                let mut file = File::create(&path);
                file.write(encoded.into_bytes().as_slice()).ok();
                config
            }
        }
    }
}

fn default_config() -> Config {
    Config {
        host: "localhost".to_string(),
        lmtp_port: 3000,
        imap_port: 143,
        uid: "imapusr".to_string()
    }
}
