use error::ImapResult;
use openssl::error::ErrorStack;
use openssl::pkcs12::Pkcs12;
use openssl::ssl::{SslAcceptor, SslMethod};
use openssl::x509::X509;
use std::io::{Read, Error as IoError, Write};
use std::fs::File;
use std::path::Path;
use std::str;
use toml;

pub enum PkcsError {
    Io(IoError),
    Ssl(ErrorStack),
    PortsDisabled
}

impl From<IoError> for PkcsError {
    fn from(e: IoError) -> Self {
        PkcsError::Io(e)
    }
}

impl From<ErrorStack> for PkcsError {
    fn from(e: ErrorStack) -> Self {
        PkcsError::Ssl(e)
    }
}

/// Representation of configuration data for the server
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    // Host on which to listen
    pub host: String,
    // Plaintext port on which to listen for LMTP
    pub lmtp_port: Option<u16>,
    // Plaintext port on which to listen for IMAP
    pub imap_port: Option<u16>,
    // SSL port on which to listen for LMTP
    pub lmtp_ssl_port: Option<u16>,
    // SSL port on which to listen for IMAP
    pub imap_ssl_port: Option<u16>,
    // file in which user data is stored
    pub users: String,
    // Filename of PKCS #12 archive
    pub pkcs_file: String,
    // Password for PKCS #12 archive
    pub pkcs_pass: String,
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

    pub fn get_ssl_acceptor(&self) -> Result<SslAcceptor, PkcsError> {
        if self.imap_ssl_port == None && self.lmtp_ssl_port == None {
            return Err(PkcsError::PortsDisabled);
        }
        let mut buf = vec![];
        let mut file = File::open(&self.pkcs_file)?;
        file.read_to_end(&mut buf)?;
        let p = Pkcs12::from_der(&buf)?;
        let identity = p.parse(&self.pkcs_pass)?;
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
        builder.set_private_key(&identity.pkey)?;
        builder.set_certificate(&identity.cert)?;
        let chain: Vec<X509> = identity.chain.into_iter().flatten().collect();
        for cert in chain.iter().rev() {
            builder.add_extra_chain_cert(cert.to_owned())?;
        }
        Ok(builder.build())
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: "127.0.0.1".to_string(),
            lmtp_port: Some(3000),
            imap_port: Some(10000),
            lmtp_ssl_port: None,
            imap_ssl_port: Some(10001),
            users: "./users.json".to_string(),
            pkcs_file: String::new(),
            pkcs_pass: String::new(),
        }
    }
}
