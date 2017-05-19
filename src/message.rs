use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str;

use command::Attribute;
use command::Attribute::{
    Envelope,
    Flags,
    InternalDate,
    RFC822,
    Body,
    BodyPeek,
    BodySection,
    BodyStructure,
    UID
};
use command::RFC822Attribute::{
    AllRFC822,
    HeaderRFC822,
    SizeRFC822,
    TextRFC822
};
use command::store::StoreName;

use error::{Error, ImapResult};

use mime::Message as MIME_Message;

use time;
use time::Timespec;

/// Representation of a message flag
#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub enum Flag {
    Answered,
    Draft,
    Flagged,
    Seen,
    Deleted
}

/// Takes a flag argument and returns the corresponding enum.
pub fn parse_flag(flag: &str) -> Option<Flag> {
    match flag {
        "\\Deleted" => Some(Flag::Deleted),
        "\\Seen" => Some(Flag::Seen),
        "\\Draft" => Some(Flag::Draft),
        "\\Answered" => Some(Flag::Answered),
        "\\Flagged" => Some(Flag::Flagged),
        _ => None
    }
}

/// Representation of a Message
#[derive(Debug, Clone)]
pub struct Message {
    // a unique id (timestamp) for the message
    uid: usize,

    // filename
    path: PathBuf,

    mime_message: MIME_Message,

    // contains the message's flags
    flags: HashSet<Flag>,

    // marks the message for deletion
    deleted: bool,

}

impl Message {
    pub fn new(arg_path: &Path) -> ImapResult<Message> {
        let mime_message = MIME_Message::new(arg_path)?;

        // Grab the string in the filename representing the flags
        let mut path = arg_path.file_name().unwrap().to_str().unwrap().splitn(1, ':');
        let filename = path.next().unwrap();
        let path_flags = path.next();

        // Retrieve the UID from the provided filename.
        let uid = filename.parse().map_err(|_| Error::MessageUidDecode)?;

        // Parse the flags from the filename.
        let flags = match path_flags {
            // if there are no flags, create an empty set
            None => HashSet::new(),
            Some(flags) =>
                // The uid is separated from the flag part of the filename by a
                // colon. The flag part consists of a 2 followed by a comma and
                // then some letters. Those letters represent the message flags
                match flags.splitn(1, ',').nth(1) {
                    None => HashSet::new(),
                    Some(unparsed_flags) => {
                        let mut set_flags: HashSet<Flag> = HashSet::new();
                        for flag in unparsed_flags.chars() {
                            let parsed_flag = match flag {
                                'D' => Some(Flag::Draft),
                                'F' => Some(Flag::Flagged),
                                'R' => Some(Flag::Answered),
                                'S' => Some(Flag::Seen),
                                _ => None
                            };
                            if let Some(enum_flag) = parsed_flag {
                                set_flags.insert(enum_flag);
                            }
                        }
                        set_flags
                    }
                }
        };

        let message = Message {
            uid: uid,
            path: arg_path.to_path_buf(),
            mime_message: mime_message,
            flags: flags,
            deleted: false
        };

        Ok(message)
    }

    /// convenience method for determining if Seen is in this message's flags
    pub fn is_unseen(&self) -> bool {
        self.flags.contains(&Flag::Seen)
    }

    pub fn rename(&self, pb: PathBuf) -> Message {
        Message {
            uid: self.uid,
            path: pb,
            mime_message: self.mime_message.clone(),
            flags: self.flags.clone(),
            deleted: self.deleted
        }
    }

    pub fn remove_if_deleted(&self) -> bool {
        if self.deleted {
            // Get the compiler to STFU with empty match block
            match fs::remove_file(self.path.as_path()) {
                _ => {}
            }
        }
        self.deleted
    }

    pub fn get_path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn get_uid(&self) -> usize {
        self.uid
    }

    pub fn store(&mut self, flag_name: &StoreName,
                 new_flags: HashSet<Flag>) -> String {
        match *flag_name {
            StoreName::Sub => {
                for flag in &new_flags {
                    self.flags.remove(flag);
                }
            }
            StoreName::Replace => { self.flags = new_flags; }
            StoreName::Add => {
                for flag in new_flags {
                    self.flags.insert(flag);
                }
            }
        }

        self.deleted = self.flags.contains(&Flag::Deleted);
        self.print_flags()
    }

    /// Goes through the list of attributes, constructing a FETCH response for
    /// this message containing the values of the requested attributes
    pub fn fetch(&self, attributes: &[Attribute]) -> String {
        let mut res = String::new();
        let mut first = true;
        for attr in attributes.iter() {
            // We need to space separate the attribute values
            if first {
                first = false;
            } else {
                res.push(' ');
            }

            // Provide the attribute name followed by the attribute value
            match *attr {
                Envelope => {
                    res.push_str("ENVELOPE ");
                    res.push_str(&self.mime_message.get_envelope()[..]);
                },
                Flags => {
                    res.push_str("FLAGS ");
                    res.push_str(&self.print_flags()[..]);
                },
                InternalDate => {
                    res.push_str("INTERNALDATE \"");
                    res.push_str(&self.date_received()[..]);
                    res.push('"');
                }
                RFC822(ref attr) => {
                    res.push_str("RFC822");
                    match *attr {
                        AllRFC822 | TextRFC822 => {},
                        HeaderRFC822 => {
                            res.push_str(".HEADER {");
                            res.push_str(&self.mime_message.get_header_boundary()[..]);
                            res.push_str("}\r\n");
                            res.push_str(self.mime_message.get_header());
                        },
                        SizeRFC822 => {
                            res.push_str(".SIZE ");
                            res.push_str(&self.mime_message.get_size()[..]) },
                    };
                },
                Body | BodyStructure => {},
                BodySection(ref section, ref octets) |
                    BodyPeek(ref section, ref octets) => {
                        res.push_str(&self.mime_message.get_body(section, octets)[..]) },
                /*
                BodyStructure => {
                    let content_type: Vec<&str> = (&self.headers["CONTENT-TYPE".to_string()][..]).splitn(1, ';').take(1).collect();
                    let content_type: Vec<&str> = content_type[0].splitn(1, '/').collect();

                    // Retrieve the subtype of the content type.
                    let mut subtype = String::new();
                    if content_type.len() > 1 { subtype = content_type[1].to_ascii_uppercase() }

                    let content_type = content_type[0].to_ascii_uppercase();
                    println!("Content-type: {}/{}", content_type, subtype);
                    match &content_type[..] {
                        "MESSAGE" => {
                            match &subtype[..] {
                                "RFC822" => {
                                    // Immediately after the basic fields, add the envelope
                                    // structure, body structure, and size in text lines of
                                    // the encapsulated message.
                                },
                                _ => { },
                            }
                        },
                        "TEXT" => {
                            // Immediately after the basic fields, add the size of the body
                            // in text lines. This is the size in the content transfer
                            // encoding and not the size after any decoding.
                        },
                        "MULTIPART" => {

                        },
                        _ => { },
                    }
                },
                */
                UID => {
                    res.push_str("UID ");
                    res.push_str(&self.uid.to_string()[..])
                }
            }
        }
        res
    }

    // Creates a string of the current set of flags based on what is in
    // self.flags.
    fn print_flags(&self) -> String {
        let mut res = "(".to_string();
        let mut first = true;
        for flag in &self.flags {
            // The flags should be space separated.
            if first {
                first = false;
            } else {
                res.push(' ');
            }
            let flag_str = match *flag {
                Flag::Answered => { "\\Answered" },
                Flag::Draft => { "\\Draft" },
                Flag::Flagged => { "\\Flagged" },
                Flag::Seen => { "\\Seen" }
                Flag::Deleted => { "\\Deleted" }
            };
            res.push_str(flag_str);
        }
        res.push(')');
        res
    }

    /// Creates a new filename using the convention that we use while parsing
    /// the message's filename. UID followed by a colon, then 2, then the
    /// single character per flag representation of the current set of flags.
    pub fn get_new_filename(&self) -> String {
        let mut res = self.uid.to_string();

        // it is just the UID if no flags are set.
        if self.flags.is_empty() {
            return res;
        }

        // Add the prelud which separates the flags
        res.push_str(":2,");

        // As per the Maildir standard, the flags are to be written in
        // alphabetical order
        if self.flags.contains(&Flag::Draft) {
            res.push('D');
        }
        if self.flags.contains(&Flag::Flagged) {
            res.push('F');
        }
        if self.flags.contains(&Flag::Answered) {
            res.push('R');
        }
        if self.flags.contains(&Flag::Seen) {
            res.push('S');
        }
        res
    }

    fn date_received(&self) -> String {
        // Retrieve the date received from the UID.
        let date_received = Timespec { sec: self.uid as i64, nsec: 0i32 };
        let date_received_tm = time::at_utc(date_received);

        let month = match date_received_tm.tm_mon {
            0 => "Jan",
            1 => "Feb",
            2 => "Mar",
            3 => "Apr",
            4 => "May",
            5 => "Jun",
            6 => "Jul",
            7 => "Aug",
            8 => "Sep",
            9 => "Oct",
            10 => "Nov",
            11 => "Dec",
            // NOTE: this should never happen.
            _ => panic!("Unable to determine month!")
        };

        format!(
            "{:0>2}-{}-{:0>2} {:0>2}:{:0>2}:{:0>2} -0000",
            date_received_tm.tm_mday,
            month,
            date_received_tm.tm_year + 1900i32,
            date_received_tm.tm_hour,
            date_received_tm.tm_min,
            date_received_tm.tm_sec)
    }
}
