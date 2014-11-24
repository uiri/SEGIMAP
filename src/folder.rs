use std::io::fs;
use std::fmt::{Show, Formatter, FormatError};

use message::Message;

pub struct Folder {
    pub name: String,
    pub owner: Option<Box<Folder>>,
    recent: uint,
    pub exists: uint,
    pub unseen: uint,
    path: Path,
    messages: Vec<Message>,
    cur: Vec<Path>,
    new: Vec<Path>
}

macro_rules! make_vec_path(
    ($path:ident, $inp:ident, $str:expr, $next:expr) => ({
        match fs::readdir(&($path.join($str))) {
            Err(_) => { return None; }
            Ok(res) => {
                let $inp = res;
                $next
            }
        }
    });
)

impl Folder {
    pub fn new(name: String, owner: Option<Box<Folder>>, path: Path) -> Option<Folder> {
        match fs::File::open(&path.join(".lock")) {
            Err(_) => {
                match fs::File::create(&path.join(".lock")) {
                    Ok(mut file) => {
                        // Get rustc to STFU with this match
                        match file.write(b"selected") { _ => {} }
                        drop(file);
                        make_vec_path!(path, cur, "cur",
                           make_vec_path!(path, new, "new",
                           {
                               let mut messages = Vec::new();
                               // populate messages
                               for msg_path in cur.iter() {
                                   match Message::parse(msg_path) {
                                       Ok(message) => {
                                           messages.push(message);
                                       }
                                       _ => {}
                                   }
                               }
                               let unseen = messages.len();
                               for msg_path in new.iter() {
                                   match Message::parse(msg_path) {
                                       Ok(message) => {
                                           messages.push(message);
                                       }
                                       _ => {}
                                   }
                               }
                               let exists = messages.len();
                               return Some(Folder {
                                   name: name,
                                   owner: owner,
                                   path: path,
                                   recent: exists-unseen,
                                   unseen: unseen,
                                   exists: exists,
                                   messages: messages,
                                   cur: cur,
                                   new: new
                               })}
                           )
                       );
                    }
                    _ => { return None; }
                }
            }
            _ => { return None; }
        }
    }

    pub fn recent(&self) -> uint {
        for msg in self.new.iter() {
            match msg.filename_str() {
                Some(filename) => {
                    // Get rustc to STFU with this match block
                    match fs::rename(msg, &self.path.join("cur").join(filename)) {
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        self.recent
    }

    pub fn close(&self) -> Vec<uint> {
        let mut result = Vec::new();
        let mut index = 0u;
        while index < self.messages.len() {
            if self.messages[index].deleted {
                match fs::unlink(&Path::new(self.messages[index].path.clone())) { _ => {} }
                result.push(index + 1);
            } else {
                index = index + 1;
            }
        }
        // Get rustc to STFU with this match block
        match fs::unlink(&self.path.join(".lock")) { _ => {} }
        return result;
    }

    pub fn get_message(&self, index: uint) -> &Message {
        &self.messages[index]
    }

    pub fn message_count(&self) -> uint {
        self.messages.len()
    }
}

impl Show for Folder {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
        self.name.fmt(f)
    }
}
