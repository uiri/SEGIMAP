use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use crate::command::Attribute;
use crate::message::Flag;
use crate::message::Message;

use crate::command::store::StoreName;

/// Representation of a Folder
#[derive(Clone, Debug)]
pub struct Folder {
    // How many messages are in folder/new/
    recent: usize,
    // How many messages are in the folder total
    exists: usize,
    // How many messages are not marked with the Seen flag
    unseen: usize,
    // Whether the folder has been opened as read-only or not
    readonly: bool,
    path: PathBuf,
    messages: Vec<Message>,
    // A mapping of message uids to indices in folder.messages
    uid_to_seqnum: HashMap<usize, usize>,
}

// Macro to handle each message in the folder
macro_rules! handle_message(
    ($msg_path_entry:ident, $uid_map:ident, $messages:ident, $i:ident, $unseen:ident) => ({
        if let Ok(msg_path) = $msg_path_entry {
            if let Ok(message) = Message::new(msg_path.path().as_path()) {
                if $unseen == !0usize && message.is_unseen() {
                    $unseen = $i;
                }
                $uid_map.insert(message.get_uid(), $i);
                $i += 1;
                $messages.push(message);
            }
        }
    });
);

// Perform a rename operation on a message
macro_rules! rename_message(
    ($msg:ident, $curpath:expr, $new_messages:ident) => ({
        if fs::rename($msg.get_path(), &$curpath).is_ok() {
            // if the rename operation succeeded then clone the message,
            // update its path and add the clone to our new list
            $new_messages.push($msg.rename($curpath));
        } else {
            // if the rename failed, just add the old message to our
            // new list
            $new_messages.push($msg.clone());
        }
    })
);

impl Folder {
    pub fn new(path: PathBuf, examine: bool) -> Option<Folder> {
        // the EXAMINE command is always read-only or we test SELECT for read-only status
        // We use a lock file to determine write access on a folder
        let readonly = if examine || fs::File::open(&path.join(".lock")).is_ok() {
            true
        } else {
            if let Ok(mut file) = fs::File::create(&path.join(".lock")) {
                // Get the compiler to STFU with this match
                let _ = file.write(b"selected");
                false
            } else {
                true
            }
        };

        if let Ok(cur) = fs::read_dir(&(path.join("cur"))) {
            if let Ok(new) = fs::read_dir(&(path.join("new"))) {
                let mut messages = Vec::new();
                let mut uid_to_seqnum: HashMap<usize, usize> = HashMap::new();
                let mut i = 0usize;
                let mut unseen = !0usize;

                // populate messages
                for msg_path in cur {
                    handle_message!(msg_path, uid_to_seqnum, messages, i, unseen);
                }

                let old = i;
                for msg_path in new {
                    handle_message!(msg_path, uid_to_seqnum, messages, i, unseen);
                }

                // Move the messages from folder/new to folder/cur
                messages = move_new(&messages, path.as_path(), unseen);
                return Some(Folder {
                    path: path,
                    recent: i - old,
                    unseen: unseen,
                    exists: i,
                    messages: messages,
                    readonly: readonly,
                    uid_to_seqnum: uid_to_seqnum,
                });
            }
        }
        None
    }

    /// Generate the SELECT/EXAMINE response based on data in the folder
    pub fn select_response(&self, tag: &str) -> String {
        let unseen_res = if self.unseen <= self.exists {
            let unseen_str = self.unseen.to_string();
            let mut res = "* OK [UNSEEN ".to_string();
            res.push_str(&unseen_str[..]);
            res.push_str("] Message ");
            res.push_str(&unseen_str[..]);
            res.push_str("th is the first unseen\r\n");
            res
        } else {
            "".to_string()
        };

        let read_status = if self.readonly {
            "[READ-ONLY]"
        } else {
            "[READ-WRITE]"
        };

        // * <n> EXISTS
        // * <n> RECENT
        // * OK UNSEEN
        // * Flags - Should match values in enum Flag in message.rs
        // * OK PERMANENTFLAG - Should match values in enum Flag in message.rs
        // * OK UIDNEXT
        // * OK UIDVALIDITY
        format!("* {} EXISTS\r\n* {} RECENT\r\n{}* FLAGS (\\Answered \\Deleted \\Draft \\Flagged \\Seen)\r\n* OK [PERMANENTFLAGS (\\Answered \\Deleted \\Draft \\Flagged \\Seen)] Permanent flags\r\n{} OK {} SELECT command was successful\r\n", 
                 self.exists, self.recent, unseen_res, tag, read_status)
    }

    /// Delete on disk all the messages marked for deletion
    /// Returns the list of sequence numbers which have been deleted on disk
    /// Per RFC 3501, the later sequence numbers are calculated based on the
    /// sequence numbers at the time of the deletion not at the start of the function
    pub fn expunge(&self) -> Vec<usize> {
        let mut result = Vec::new();
        // We can't perform the deletion if the folder has been opened as
        // read-only
        if !self.readonly {
            // Vectors are 0-indexed
            let mut index = 0usize;

            // self.messages will get smaller as we go through it
            while index < self.messages.len() {
                if self.messages[index].remove_if_deleted() {
                    // Sequence numbers are 1-indexed
                    result.push(index + 1);
                } else {
                    index += 1;
                }
            }
            // Get the compiler to STFU with empty match block
            match fs::remove_file(&self.path.join(".lock")) {
                _ => {}
            }
        }
        result
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Perform a fetch of the specified attributes on self.messsages[index]
    /// Return the FETCH response string to be sent back to the client
    pub fn fetch(&self, index: usize, attributes: &[Attribute]) -> String {
        let mut res = "* ".to_string();
        res.push_str(&(index + 1).to_string()[..]);
        res.push_str(" FETCH (");
        res.push_str(&self.messages[index].fetch(attributes)[..]);
        res.push_str(")\r\n");
        res
    }

    /// Turn a UID into a sequence number
    pub fn get_index_from_uid(&self, uid: &usize) -> Option<&usize> {
        self.uid_to_seqnum.get(uid)
    }

    /// Perform a STORE on the specified set of sequence numbers
    /// This modifies the flags of the specified messages
    /// Returns the String response to be sent back to the client.
    pub fn store(
        &mut self,
        sequence_set: Vec<usize>,
        flag_name: &StoreName,
        silent: bool,
        flags: HashSet<Flag>,
        seq_uid: bool,
        tag: &str,
    ) -> String {
        let mut responses = String::new();
        for num in &sequence_set {
            let (uid, i) = if seq_uid {
                match self.get_index_from_uid(num) {
                    // 0 is an invalid sequence number
                    // Return it if the UID isn't found
                    None => (*num, 0usize),
                    Some(ind) => (*num, *ind + 1),
                }
            } else {
                (0usize, *num)
            };

            // if i == 0 then the UID wasn't in the sequence number map
            if i == 0 {
                continue;
            }

            // Create the FETCH response for this STORE operation.
            if let Some(message) = self.messages.get_mut(i - 1) {
                responses.push_str("* ");
                responses.push_str(&i.to_string()[..]);
                responses.push_str(" FETCH (FLAGS ");
                responses.push_str(&message.store(flag_name, flags.clone())[..]);

                // UID STORE needs to respond with the UID for each FETCH response
                if seq_uid {
                    let uid_res = format!(" UID {}", uid);
                    responses.push_str(&uid_res[..]);
                }
                responses.push_str(" )\r\n");
            }
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
        for msg in &self.messages {
            // Grab the new filename composed of this message's UID and its current flags.
            let filename = msg.get_new_filename();
            let curpath = self.path.join("cur").join(filename);

            // If the new filename is the same as the current filename, add the
            // current message to our new list and move on to the next message
            if curpath == msg.get_path() {
                new_messages.push(msg.clone());
                continue;
            }
            rename_message!(msg, curpath, new_messages);
        }

        // Set the current list of messages to the new list of messages
        // The compiler *should* make this discard the old list...
        self.messages = new_messages;
    }
}

/// This moves a list of messages from folder/new/ to folder/cur/ and returns a
/// new list of messages
fn move_new(messages: &[Message], path: &Path, start_index: usize) -> Vec<Message> {
    let mut new_messages = Vec::new();

    // Go over the messages by index
    for (i, msg) in messages.iter().enumerate() {
        // messages before start_index are already in folder/cur/
        if i + 1 < start_index {
            new_messages.push(msg.clone());
            continue;
        }
        let curpath = path.join("cur").join(msg.get_uid().to_string());
        rename_message!(msg, curpath, new_messages);
    }

    // Return the new list of messages
    new_messages
}
