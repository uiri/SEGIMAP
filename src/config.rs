use std::old_io::File;

use toml::{decode_str, encode_str};

/// Representation of configuration data for the server
#[derive(RustcDecodable, RustcEncodable, Debug)]
pub struct Config {
    // Host on which to listen
    pub host: String,
    // Port on which to listen for LMTP
    pub lmtp_port: u16,
    // Port on which to listen for IMAP
    pub imap_port: u16,
    // file in which user data is stored
    pub users: String
}

impl Config {
    pub fn new() -> Config {
        let path = Path::new("./config.toml");
        match File::open(&path).read_to_end() {
            Ok(v) => {
                match decode_str(String::from_utf8_lossy(v.as_slice()).as_slice()) {
                    Some(v) => v,
                    None => {
                        // Use default values if parsing failed.
                        warn!("Failed to parse config.toml.\nUsing default values.");
                        default_config()
                    }
                }
            },
            Err(_) => {
                // Create a default config file if it doesn't exist
                warn!("Failed to read config.toml; creating from defaults.");
                let config = default_config();
                let encoded = encode_str(&config);
                let mut file = File::create(&path);
                file.write(encoded.into_bytes().as_slice()).ok();
                config
            }
        }
    }
}

/// Default config values
fn default_config() -> Config {
    Config {
        host: "127.0.0.1".to_string(),
        lmtp_port: 3000,
        imap_port: 10000,
        users: "./users.json".to_string()
    }
}
