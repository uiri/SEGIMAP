use error::ImapResult;
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
    // Plaintext Port on which to listen for LMTP
    pub lmtp_port: u16,
    // Plaintext Port on which to listen for IMAP
    pub imap_port: u16,
    // SSL Port on which to listen for LMTP
    pub lmtp_ssl_port: u16,
    // SSL Port on which to listen for IMAP
    pub imap_ssl_port: u16,
    // file in which user data is stored
    pub users: String
}

impl Config {
    pub fn new() -> ImapResult<Config> {
        let path = Path::new("./config.toml");

        let config = match File::open(&path) {
            Ok(mut file) => {
                let mut encoded: String = String::new();
                match file.read_to_string(&mut encoded) {
                    Ok(_) => match toml::from_str(&encoded) {
                        Ok(v) => v,
                        Err(e) => {
                            // Use default values if parsing failed.
                            warn!("Failed to parse config.toml.\nUsing default values: {}", e);
                            Config::default()
                        },
                    },
                    Err(e) => {
                        // Use default values if reading failed.
                        warn!("Failed to read config.toml.\nUsing default values: {}", e);
                        Config::default()
                    },
                }
            },
            Err(e) => {
                // Create a default config file if it doesn't exist
                warn!("Failed to open config.toml; creating from defaults: {}", e);
                let config = Config::default();
                let encoded = toml::to_string(&config)?;
                let mut file = File::create(&path)?;
                file.write_all(encoded.as_bytes())?;
                config
            },
        };

        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: "127.0.0.1".to_string(),
            lmtp_port: 3000,
            imap_port: 10000,
            lmtp_ssl_port: 0,
            imap_ssl_port: 10001,
            users: "./users.json".to_string()
        }
    }
}
