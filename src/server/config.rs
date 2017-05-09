use std::io::{Read, Write};
use std::fs::File;
use std::path::Path;
use std::str;

use toml;

/// Representation of configuration data for the server
#[derive(Debug, Serialize, Deserialize)]
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
        let mut conf_buf : Vec<u8> = Vec::new();
        match File::open(&path).unwrap().read_to_end(&mut conf_buf) {
            Ok(_) => {
                match toml::from_str(str::from_utf8(&conf_buf[..]).unwrap()) {
                    Ok(v) => v,
                    Err(e) => {
                        // Use default values if parsing failed.
                        warn!("Failed to parse config.toml.\nUsing default values: {}", e);
                        default_config()
                    }
                }
            },
            Err(_) => {
                // Create a default config file if it doesn't exist
                warn!("Failed to read config.toml; creating from defaults.");
                let config = default_config();
                let encoded = toml::to_string(&config).unwrap();
                let mut file = File::create(&path).unwrap();
                file.write(&encoded.into_bytes()[..]).ok();
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
