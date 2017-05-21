use std::collections::HashMap;
use std::io::{Read, Result, Write};
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;

use bufstream::BufStream;
use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslMethod, SslStream};

use error::ImapResult;
use self::config::Config;
use user::{load_users, Email, User};

mod config;
#[macro_use]
pub mod lmtp;

pub enum Stream {
    Ssl(SslStream<TcpStream>),
    Tcp(TcpStream)
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match *self {
            Stream::Ssl(ref mut s) => s.write(buf),
            Stream::Tcp(ref mut s) => s.write(buf)
        }
    }

    fn flush(&mut self) -> Result<()> {
        match *self {
            Stream::Ssl(ref mut s) => s.flush(),
            Stream::Tcp(ref mut s) => s.flush()
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match *self {
            Stream::Ssl(ref mut s) => s.read(buf),
            Stream::Tcp(ref mut s) => s.read(buf)
        }
    }
}

/// Holds configuration state and email->user map
pub struct Server {
    conf: Config,
    users: HashMap<Email, User>,
    ssl_acceptor: Option<SslAcceptor>,
}

impl Server {
    pub fn new() -> ImapResult<Server> {
        Server::new_with_conf(Config::new()?)
    }

    /// Create server to hold the Config and User HashMap
    fn new_with_conf(conf: Config) -> ImapResult<Server> {
        // Load the user data from the specified user data file.
        let users = load_users(&conf.users)?;
        let ssl_acceptor = if let Ok(identity) = conf.get_ssl_keys() {
            if conf.imap_ssl_port != 0 || conf.lmtp_ssl_port != 0 {
                match SslAcceptorBuilder::mozilla_intermediate(
                        SslMethod::tls(), &identity.pkey, 
                        &identity.cert, &identity.chain) {
                    Ok(a) => Some(a.build()),
                    _ => None
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(Server {
            conf: conf,
            users: users,
            ssl_acceptor: ssl_acceptor,
        })
    }

    /// Create a TCP listener on the server host and imap post
    pub fn imap_listener(&self) -> Result<TcpListener> {
        TcpListener::bind((&self.conf.host[..], self.conf.imap_port))
    }

    pub fn imap_ssl(&self, stream: TcpStream) -> Stream {
        let local_addr = stream.local_addr();
        match local_addr {
            Ok(addr) =>
                if addr.port() != self.conf.imap_ssl_port {
                    return Stream::Tcp(stream);
                },
            _ => return Stream::Tcp(stream)
        }
        if let Some(ref ssl_acceptor) = self.ssl_acceptor {
            Stream::Ssl(ssl_acceptor.accept(stream).unwrap())
        } else {
            Stream::Tcp(stream)
        }
    }

    pub fn can_starttls(&self) -> bool {
        if let Some(_) = self.ssl_acceptor {
            true
        } else {
            false
        }
    }

    pub fn starttls(&self, stream: TcpStream) -> Option<SslStream<TcpStream>> {
        if let Some(ref ssl_acceptor) = self.ssl_acceptor {
            Some(ssl_acceptor.accept(stream).unwrap())
        } else {
            None
        }
    }

    /// Create a TCP listener on the server host and lmtp port
    pub fn lmtp_listener(&self) -> Result<TcpListener> {
        TcpListener::bind((&self.conf.host[..], self.conf.lmtp_port))
    }

    fn host(&self) -> &String {
        &self.conf.host
    }

    pub fn get_user(&self, email: &Email) -> Option<&User> {
        self.users.get(email)
    }
}

pub fn lmtp_serve(serv: Arc<Server>, stream: TcpStream) {
    lmtp::serve(serv, BufStream::new(stream))
}
