use std::collections::{HashMap,HashSet};
use std::io::fs;

use command::command::Attribute;

use message::Message;
use message::Flag;
use util::StoreName;

/// Representation of a Folder
pub struct Folder {
    // How many messages are in folder/new/
    pub recent: uint,
    // How many messages are in the folder total
    pub exists: uint,
    // How many messages are not marked with the Seen flag
    pub unseen: uint,
    path: Path,
    messages: Vec<Message>,
    // Whether the folder has been opened as read-only or not
    readonly: bool,
    // A mapping of message uids to indices in folder.messages
    uid_to_seqnum: HashMap<uint, uint>
}

// Macro to open up a directory and do stuff with it after
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
);

// Macro to handle each message in the folder
macro_rules! handle_message(
    ($msg_path:ident, $uid_map:ident, $messages:ident, $i:ident, $unseen:ident) => ({
        let message = match Message::new($msg_path) {
            Ok(message) => message,
            _ => continue
        };
        if $unseen == -1 && message.is_unseen() {
            $unseen = $i;
        }
        $uid_map.insert(message.uid.to_uint().unwrap(), $i);
        $i += 1;
        $messages.push(message);
    });
);

// Perform a rename operation on a message
macro_rules! rename_message(
    ($msg:ident, $msg_path:expr, $curpath:expr, $new_messages:ident) => ({
        match fs::rename($msg_path, $curpath) {
            Ok(_) => {
                // if the rename operation succeeded then clone the message,
                // update its path and add the clone to our new list
                let mut new_msg = $msg.clone();
                new_msg.path = $curpath.display().to_string();
                $new_messages.push(new_msg);
            }
            _ => {
                // if the rename failed, just add the old message to our
                // new list
                $new_messages.push($msg.clone());
            }
        }
    })
);

impl Folder {
    pub fn new(path: Path, examine: bool) -> Option<Folder> {
        // the EXAMINE command is always read-only
        let readonly = if examine {
            true
        } else {
            // Test SELECT for read-only status
            // We use a lock file to determine write access on a folder
            match fs::File::open(&path.join(".lock")) {
                Err(_) => {
                    match fs::File::create(&path.join(".lock")) {
                        Ok(mut file) => {
                            // Get the compiler to STFU with this match
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
            make_vec_path!(path, new, "new", {
                let mut messages = Vec::new();
                let mut uid_to_seqnum: HashMap<uint, uint> = HashMap::new();
                let mut i = 0u;
                let mut unseen = -1;

                // populate messages
                for msg_path in cur.iter() {
                    handle_message!(msg_path, uid_to_seqnum, messages, i, unseen);
                }

                let old = i;

                for msg_path in new.iter() {
                    handle_message!(msg_path, uid_to_seqnum, messages, i, unseen);
                }

                // Move the messages from folder/new to folder/cur
                messages = move_new(messages, path.clone(), unseen-1);
                Some(Folder {
                    path: path,
                    recent: i-old,
                    unseen: unseen,
                    exists: i,
                    messages: messages,
                    readonly: readonly,
                    uid_to_seqnum: uid_to_seqnum,
                })
            })
        )
    }

    /// Generate the UNSEEN message for the SELECT/EXAMINE response if necessary
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

    /// Delete on disk all the messages marked for deletion
    /// Returns the list of sequence numbers which have been deleted on disk
    /// Per RFC 3501, the later sequence numbers are calculated based on the
    /// sequence numbers at the time of the deletion not at the start of the function
    pub fn expunge(&self) -> Vec<uint> {
        let mut result = Vec::new();
        // We can't perform the deletion if the folder has been opened as
        // read-only
        if !self.readonly {
            // Vectors are 0-indexed
            let mut index = 0u;

            // self.messages will get smaller as we go through it
            while index < self.messages.len() {
                if self.messages[index].deleted {
                    // Get the compiler to STFU with empty match block
                    match fs::unlink(&Path::new(self.messages[index].path.clone())) {
                        _ => {}
                    }
                    // Sequence numbers are 1-indexed
                    result.push(index + 1);
                } else {
                    index = index + 1;
                }
            }
            // Get the compiler to STFU with empty match block
            match fs::unlink(&self.path.join(".lock")) { _ => {} }
        }
        return result;
    }

    pub fn message_count(&self) -> uint {
        self.messages.len()
    }

    /// Perform a fetch of the specified attributes on self.messsages[index]
    /// Return the FETCH response string to be sent back to the client
    pub fn fetch(&self, index: uint, attributes: &Vec<Attribute>) -> String {
        let mut res = "* ".to_string();
        res.push_str((index+1).to_string().as_slice());
        res.push_str(" FETCH (");
        res.push_str(self.messages[index].fetch(attributes).as_slice());
        res.push_str(")\r\n");
        res
    }

    /// Turn a UID into a sequence number
    pub fn get_index_from_uid(&self, uid: &uint) -> Option<&uint> {
        return self.uid_to_seqnum.find(uid);
    }

    /// Perform a STORE on the specified set of sequence numbers
    /// This modifies the flags of the specified messages
    /// Returns the String response to be sent back to the client.
    pub fn store(&mut self, sequence_set: Vec<uint>, flag_name: StoreName,
                 silent: bool, flags: HashSet<Flag>, seq_uid: bool,
                 tag: &str) -> String {
        let mut responses = String::new();
        for num in sequence_set.iter() {
            let (uid, i) = if seq_uid {
                match self.get_index_from_uid(num) {
                    // 0 is an invalid sequence number
                    // Return it if the UID isn't found
                    None => (*num, 0u),
                    Some(ind) => (*num, *ind+1)
                }
            } else {
                (0u, *num)
            };

            // if i == 0 then the UID wasn't in the sequence number map
            if i == 0 {
                continue;
            }

            // Create the FETCH response for this STORE operation.
            let ref mut message = self.messages.get_mut(i-1);
            responses.push_str("* ");
            responses.push_str(i.to_string().as_slice());
            responses.push_str(" FETCH (FLAGS ");
            responses.push_str(message.store(flag_name, flags.clone()).as_slice());

            // UID STORE needs to respond with the UID for each FETCH response
            if seq_uid {
                let uid_res = format!(" UID {}", uid);
                responses.push_str(uid_res.as_slice());
            }
            responses.push_str(" )\r\n");
        }

        // Return an empty string if the client wanted the STORE to be SILENT
        if silent {
            responses = String::new();
        }
        responses.push_str(tag);
        responses.push_str(" OK STORE complete\r\n");
        responses
    }

    /// Reconcile the internal state of the folder with the disk.
    pub fn check(&mut self) {
        // If it is read-only we can't write any changes to disk
        if self.readonly {
            return;
        }

        // We need to create a new list of messages because the compiler will
        // yell at us for inspecting the internal state of the message and
        // modifying that state at the same time
        let mut new_messages = Vec::new();
        for msg in self.messages.iter() {
            // Grab the new filename composed of this message's UID and its current flags.
            let filename = msg.get_new_filename();
            let curpath = self.path.join("cur").join(filename);

            // Grab the current filename
            let msg_path = Path::new(msg.path.clone());

            // If the new filename is the same as the current filename, add the
            // current message to our new list and move on to the next message
            if curpath == msg_path {
                new_messages.push(msg.clone());
                continue;
            }
            rename_message!(msg, &msg_path, &curpath, new_messages);
        }

        // Set the current list of messages to the new list of messages
        // The compiler *should* make this discard the old list...
        self.messages = new_messages;
    }

    /// Generate the read state portion of the SELECT response
    pub fn read_status(&self) -> &'static str {
        if self.readonly {
            "[READ-ONLY]"
        } else {
            "[READ-WRITE]"
        }
    }
}

/// This moves a list of messages from folder/new/ to folder/cur/ and returns a
/// new list of messages
fn move_new(messages: Vec<Message>, path: Path,
            start_index: uint) -> Vec<Message> {
    let mut new_messages = Vec::new();

    // Go over the messages by index
    for i in range(0u, messages.len()) {
        // messages before start_index are already in folder/cur/
        if i < start_index {
            new_messages.push(messages[i].clone());
            continue;
        }
        let ref msg = messages[i];
        let curpath = path.join("cur").join(msg.uid.to_string());
        let msg_path = Path::new(msg.path.clone());
        rename_message!(msg, &msg_path, &curpath, new_messages);
    }

    // Return the new list of messages
    new_messages
}
