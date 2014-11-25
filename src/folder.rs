use std::collections::HashSet;
use std::io::fs;
use std::fmt::{Show, Formatter, FormatError};

use command::command::Attribute;
use error::{
    Error, ImapResult, NoSuchMessageError
};
use message::Message;
use message::StoreName;
use message::Flag;

pub struct Folder {
    pub name: String,
    pub owner: Option<Box<Folder>>,
    recent: uint,
    pub exists: uint,
    pub unseen: uint,
    path: Path,
    messages: Vec<Message>,
    pub readonly: bool,
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
    pub fn new(name: String, owner: Option<Box<Folder>>, path: Path, examine: bool) -> Option<Folder> {
        let readonly = if examine {
            true
        } else {
            match fs::File::open(&path.join(".lock")) {
                Err(_) => {
                    match fs::File::create(&path.join(".lock")) {
                        Ok(mut file) => {
                            // Get rustc to STFU with this match
                            match file.write(b"selected") { _ => {} }
                            drop(file);
                            false
                        }
                        _ => true,
                    }
                }
                _ => true,
            }
        };
        make_vec_path!(path, cur, "cur",
            make_vec_path!(path, new, "new",
                           {
                               let mut messages = Vec::new();
                               // populate messages
                               let mut unseen = -1;
                               for msg_path in cur.iter() {
                                   match Message::parse(msg_path) {
                                       Ok(message) => {
                                           if unseen == -1 &&
                                              message.is_unseen() {
                                                  unseen = messages.len()+1;
                                           }
                                           messages.push(message);
                                       }
                                       _ => {}
                                   }
                               }
                               let old = messages.len()+1;
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
                                   recent: exists-old+1,
                                   unseen: unseen,
                                   exists: exists,
                                   messages: messages,
                                   readonly: readonly,
                                   cur: cur,
                                   new: new
                               })}
                           )
                       );
    }

    pub fn unseen(&self) -> String {
        if self.unseen <= self.exists {
            format!("* OK [UNSEEN {}] Message {}th is the first unseen\r\n", self.unseen, self.unseen)
        } else {
            "".to_string()
        }
    }

    pub fn recent(&self) -> uint {
        if !self.readonly {
            for msg in self.new.iter() {
                match msg.filename_str() {
                    Some(filename) => {
                        // Get rustc to STFU with this match block
                        // Make sure to add some damn flags
                        match fs::rename(msg, &self.path.join("cur").join(filename)) {
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        self.recent
    }

    pub fn expunge(&self) -> Vec<uint> {
        let mut result = Vec::new();
        if !self.readonly {
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
        }
        return result;
    }

    pub fn message_count(&self) -> uint {
        self.messages.len()
    }

    pub fn fetch(&self, index: uint, attributes: &Vec<Attribute>) -> String {
        self.messages[index].fetch(attributes)
    }

    pub fn get_index_from_uid(&self, uid: uint) -> ImapResult<uint> {
        for index in range(0u, self.messages.len()) {
            if self.messages[index].uid == uid as u32 { return Ok(index + 1) }
        }
        Err(Error::simple(NoSuchMessageError, "Failed to find message by UID."))
    }

    pub fn store(&mut self, sequence_set: Vec<uint>, flag_name: StoreName, silent: bool, flags: HashSet<Flag>) -> String {
        let mut responses = String::new();
        for i in sequence_set.iter() {
            let ref mut message = self.messages.get_mut(*i-1);
            responses = format!("{}* {} FETCH {}\r\n", responses, i, message.store(flag_name, flags.clone()));
        }
        if silent {
            String::new()
        } else {
            responses
        }
    }
}

impl Show for Folder {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
        self.name.fmt(f)
    }
}
