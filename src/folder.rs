use std::collections::{HashMap,HashSet};
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
    uid_to_seqnum: HashMap<uint, uint>,
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
                               let mut uid_to_seqnum: HashMap<uint, uint> = HashMap::new();
                               let mut i = 0u;
                               // populate messages
                               let mut unseen = -1;
                               for msg_path in cur.iter() {
                                   match Message::parse(msg_path) {
                                       Ok(message) => {
                                           if unseen == -1 &&
                                              message.is_unseen() {
                                                  unseen = messages.len()+1;
                                           }
                                           uid_to_seqnum.insert(message.uid.to_uint().unwrap(), i);
                                           i += 1;
                                           messages.push(message);
                                       }
                                       _ => {}
                                   }
                               }
                               let old = i+2;
                               for msg_path in new.iter() {
                                   match Message::parse(msg_path) {
                                       Ok(message) => {
                                           uid_to_seqnum.insert(message.uid.to_uint().unwrap(), i);
                                           i += 1;
                                           messages.push(message);
                                       }
                                       _ => {}
                                   }
                               }
                               let exists = i+1;
                               return Some(Folder {
                                   name: name,
                                   owner: owner,
                                   path: path,
                                   recent: exists-old+1,
                                   unseen: unseen,
                                   exists: exists,
                                   messages: messages,
                                   readonly: readonly,
                                   uid_to_seqnum: uid_to_seqnum,
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

    pub fn get_uid_from_index(&self, index: uint) -> uint {
        self.messages[index].uid as uint
    }

    pub fn get_index_from_uid(&self, uid: &uint) -> Option<&uint> {
        return self.uid_to_seqnum.find(uid);
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
