use std::collections::{HashMap,HashSet};
use std::io::fs;
use std::fmt::{Show, Formatter, FormatError};

use command::command::Attribute;

use message::Message;
use message::StoreName;
use message::Flag;

//#[deriving(Clone)]
pub struct Folder {
    pub name: String,
    pub recent: uint,
    pub exists: uint,
    pub unseen: uint,
    path: Path,
    messages: Vec<Message>,
    readonly: bool,
    uid_to_seqnum: HashMap<uint, uint>
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
    pub fn new(name: String, path: Path, examine: bool) -> Option<Folder> {
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
                                   match Message::new(msg_path) {
                                       Ok(message) => {
                                           if unseen == -1 &&
                                              message.is_unseen() {
                                                  unseen = i+1;
                                           }
                                           uid_to_seqnum.insert(message.uid.to_uint().unwrap(), i);
                                           i += 1;
                                           messages.push(message);
                                       }
                                       _ => {}
                                   }
                               }
                               let old = i+1;
                               for msg_path in new.iter() {
                                   match Message::new(msg_path) {
                                       Ok(message) => {
                                           uid_to_seqnum.insert(message.uid.to_uint().unwrap(), i);
                                           i += 1;
                                           messages.push(message);
                                       }
                                       _ => {}
                                   }
                               }
                               messages = move_new(messages, path.clone(), unseen-1);
                               let exists = i;
                               return Some(Folder {
                                   name: name,
                                   path: path,
                                   recent: exists-old+1,
                                   unseen: unseen,
                                   exists: exists,
                                   messages: messages,
                                   readonly: readonly,
                                   uid_to_seqnum: uid_to_seqnum,
                               })}
                           )
                       );
    }

    pub fn unseen(&self) -> String {
        if self.unseen <= self.exists {
            let unseen_str = self.unseen.to_string();
            let mut res = "* OK [UNSEEN ".to_string();
            res.push_str(unseen_str.as_slice());
            res.push_str("] Message ");
            res.push_str(unseen_str.as_slice());
            res.push_str("th is the first unseen\r\n");
            res
        } else {
            "".to_string()
        }
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
        let mut res = "* ".to_string();
        res.push_str((index+1).to_string().as_slice());
        res.push_str(" FETCH (");
        res.push_str(self.messages[index].fetch(attributes).as_slice());
        res.push_str(")\r\n");
        res
    }

    pub fn get_index_from_uid(&self, uid: &uint) -> Option<&uint> {
        return self.uid_to_seqnum.find(uid);
    }

    pub fn store(&mut self, sequence_set: Vec<uint>, flag_name: StoreName, silent: bool, flags: HashSet<Flag>, seq_uid: bool) -> String {
        let mut responses = String::new();
        for num in sequence_set.iter() {
            let (uid, i) = if seq_uid {
                match self.get_index_from_uid(num) {
                    None => (*num, 0u),
                    Some(ind) => (*num, *ind+1)
                }
            } else {
                (0u, *num)
            };
            if i == 0 {
                continue;
            }
            let ref mut message = self.messages.get_mut(i-1);
            responses.push_str("* ");
            responses.push_str(i.to_string().as_slice());
            responses.push_str(" FETCH (FLAGS ");
            responses.push_str(message.store(flag_name, flags.clone()).as_slice());
            if seq_uid {
                let uid_res = format!(" UID {}", uid);
                responses.push_str(uid_res.as_slice());
            }
            responses.push_str(" )\r\n");
        }
        if silent {
            String::new()
        } else {
            responses
        }
    }

    pub fn check(&mut self) {
        if self.readonly {
            return;
        }
        let mut new_messages = Vec::new();
        for msg in self.messages.iter() {
            let filename = msg.get_new_filename();
            let curpath = self.path.join("cur").join(filename);
            let msg_path = Path::new(msg.path.clone());
            if curpath == msg_path {
                new_messages.push(msg.clone());
                continue;
            }
            match fs::rename(&msg_path, &curpath) {
                Ok(_) => {
                    let mut new_msg = msg.clone();
                    new_msg.set_path(curpath.display().to_string());
                    new_messages.push(new_msg);
                }
                _ => {}
            }
        }
        self.messages = new_messages;
    }

    pub fn read_status(&self) -> &'static str {
        if self.readonly {
            "[READ-ONLY]"
        } else {
            "[READ-WRITE]"
        }
    }
}

impl Show for Folder {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
        self.name.fmt(f)
    }
}

fn move_new(messages: Vec<Message>, path: Path, start_index: uint) -> Vec<Message> {
    let mut new_messages = Vec::new();
    for i in range(0u, messages.len()) {
        if i < start_index {
            new_messages.push(messages[i].clone());
            continue;
        }
        let ref msg = messages[i];
        let curpath = path.join("cur").join(msg.uid.to_string());
        let msg_path = Path::new(msg.path.clone());
        match fs::rename(&msg_path, &curpath) {
            Ok(_) => {
                let mut new_msg = msg.clone();
                new_msg.set_path(curpath.display().to_string());
                new_messages.push(new_msg);
            }
            _ => {}
        }
    }
    new_messages
}
