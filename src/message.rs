use std::ascii::OwnedAsciiExt;
use std::collections::{HashSet, HashMap};
use std::io::File;
use std::hash::Hash;

use time;
use time::Timespec;

use command::command::{
    Attribute,
    Envelope,
    Flags,
    InternalDate,
    RFC822,
    Body,
    BodyPeek,
    BodySection,
    BodySectionType,
    BodyStructure,
    UID
};
use command::command::{
    AllSection,
    MsgtextSection,
    PartSection
};
use command::command::{
    HeaderMsgtext,
    HeaderFieldsMsgtext,
    HeaderFieldsNotMsgtext,
    TextMsgtext,
    MimeMsgtext
};
use command::command::{
    AllRFC822,
    HeaderRFC822,
    SizeRFC822,
    TextRFC822
};
use error::{
    Error, ImapResult, InternalIoError, MessageDecodeError
};
use session::{StoreName, Replace, Add, Sub};

static RECEIVED: &'static str = "RECEIVED";

/// Representation of a message flag
#[deriving(Eq, PartialEq, Hash, Show, Clone)]
pub enum Flag {
    Answered,
    Draft,
    Flagged,
    Seen,
    Deleted
}

/// Representation of a Message
#[deriving(Show, Clone)]
pub struct Message {
    // a unique id (timestamp) for the message
    pub uid: u32,

    // filename
    pub path: String,

    // maps header field names to values
    headers: HashMap<String, String>,

    // contains the MIME Parts (if more than one) of the message
    body: Vec<MIMEPart>,

    // contains the message's flags
    flags: HashSet<Flag>,

    // marks the message for deletion
    pub deleted: bool,

    // size stored in case FETCH asks for it
    size: uint,

    // the raw contents of the file representing the message
    raw_contents: String,

    // where in raw_contents the header ends and the body begins
    header_boundary: uint
}

/// Representation of a MIME message part
#[deriving(Show, Clone)]
pub struct MIMEPart {
    mime_header: String,
    mime_body: String
}

impl Message {
    pub fn new(arg_path: &Path) -> ImapResult<Message> {
        // Load the file contents.
        let raw_contents = match File::open(arg_path) {
            Ok(mut file) => match file.read_to_end() {
                Ok(contents) => {
                    String::from_utf8_lossy(contents.as_slice()).to_string()
                }
                Err(e) => return Err(Error::new(InternalIoError(e),
                                                "Failed to read mail file."))
            },
            Err(e) => return Err(Error::new(InternalIoError(e),
                                            "Failed to open mail file."))
        };

        // This slice will avoid copying later
        let raw_slice = raw_contents.as_slice();
        let size = raw_slice.len();

        // Grab the string in the filename representing the flags
        let mut path = arg_path.filename_str().unwrap().splitn(1, ':');
        let filename = path.next().unwrap();
        let path_flags = path.next();

        // Retrieve the UID from the provided filename.
        let uid = match from_str::<u32>(filename) {
            Some(uid) => uid,
            None => return Err(Error::new(MessageDecodeError,
                                          "Failed to retrieve UID from filename."))
        };

        // Parse the flags from the filename.
        let flags = match path_flags {
            // if there are no flags, create an empty set
            None => HashSet::new(),
            Some(flags) => {
                // The uid is separated from the flag part of the filename by a
                // colon. The flag part consists of a 2 followed by a comma and
                // then some letters. Those letters represent the message flags
                let unparsed_flags = flags.splitn(1, ',').skip(1).next()
                                      .unwrap();
                let mut set_flags: HashSet<Flag> = HashSet::new();
                for flag in unparsed_flags.chars() {
                    let parsed_flag = match flag {
                        'D' => Some(Draft),
                        'F' => Some(Flagged),
                        'R' => Some(Answered),
                        'S' => Some(Seen),
                        _ => None
                    };
                    match parsed_flag {
                        Some(enum_flag) => {set_flags.insert(enum_flag);}
                        None => {}
                    }
                }
                set_flags
            }
        };

        // Find boundary between header and body.
        // Use it to create &str of the raw header and raw body
        let header_boundary = raw_slice.find_str("\n\n").unwrap() + 1;
        let raw_header = raw_slice.slice_to(header_boundary);
        let raw_body = raw_slice.slice_from(header_boundary + 1);

        // Iterate over the lines of the header in reverse.
        // If a line with leading whitespace is detected, it is merged to the
        // line before it.
        // This "unfolds" the header as indicated in RFC 2822 2.2.3
        let mut iterator = raw_header.lines().rev();
        let mut headers = HashMap::new();
        loop {
            let line = match iterator.next() {
                Some(line) => line,
                None => break
            };
            if line.starts_with(" ") || line.starts_with("\t") {
                loop {
                    let next = iterator.next().unwrap();
                    let mut trimmed_next = next.trim_left_chars(' ')
                                            .trim_left_chars('\t').to_string();

                    // Add a space between the merged lines.
                    trimmed_next.push(' ');
                    trimmed_next.push_str(line.trim_left_chars(' ')
                                           .trim_left_chars('\t'));
                    if !next.starts_with(" ") && !next.starts_with("\t") {
                        let split: Vec<&str> = trimmed_next.as_slice()
                                                .splitn(1, ':').collect();
                        headers.insert(split[0].to_string().into_ascii_upper(),
                                       split[1].slice_from(1).to_string());
                        break;
                    }
                }
            } else {
                let split: Vec<&str> = line.splitn(1, ':').collect();
                headers.insert(split[0].to_string().into_ascii_upper(),
                               split[1].slice_from(1).to_string());
            }
        }

        // Remove the "Received" key from the HashMap.
        let received_key = &RECEIVED.to_string();
        match headers.find(received_key) {
            Some(_) => { headers.remove(received_key); }
            _ => {}
        }

        // Determine whether the message is MULTIPART or not.
        let mut body = Vec::new();
        match headers.find(&"CONTENT-TYPE".to_string()) {
            Some(ref content_type) => {
                if content_type.as_slice().contains("MULTIPART") {
                    // We need the boundary to determine where this part ends
                    let mime_boundary = {
                        let value: Vec<&str> = content_type.as_slice()
                                                .split_str("BOUNDARY=\"")
                                                .collect();
                        if value.len() < 2 {
                            return Err(Error::new(
                                MessageDecodeError,
                                "Failed to determine MULTIPART boundary."))
                        }
                        let value: Vec<&str> = value[1].splitn(1, '"')
                                                .collect();
                        if value.len() < 1 {
                            return Err(Error::new(
                                MessageDecodeError,
                                "Failed to determine MULTIPART boundary."))
                        }
                        format!("--{}--\n", value[0])
                    };

                    // Grab the content type for this part
                    let first_content_type_index =
                        match raw_body.find_str("Content-Type") {
                            Some(val) => val,
                            None =>
                                return Err(Error::new(
                                    MessageDecodeError,
                                    "Missing Content-Type for body part"))
                    };
                    let mime_boundary_slice = mime_boundary.as_slice();
                    let raw_body = raw_body.slice_from(first_content_type_index);
                    let raw_body: Vec<&str> = raw_body.split_str(
                                               mime_boundary_slice).collect();
                    let raw_body = raw_body.slice_to(raw_body.len() - 1);

                    // Throw the parts of the message into a list of MIMEParts
                    for part in raw_body.iter() {
                        let header_boundary = part.as_slice().find_str("\n\n")
                                               .unwrap();
                        let header = part.as_slice().slice_to(header_boundary);
                        let mut content_type = String::new();
                        for line in header.lines() {
                            let split_line: Vec<&str> = line.splitn(1, ':')
                                                         .collect();
                            if split_line[0] == "Content-Type" {
                                let content_type_values: Vec<&str> =
                                    split_line[1].splitn(1, ';').collect();
                                content_type = content_type_values[0]
                                                .slice_from(1).to_string();
                                break;
                            }
                        }
                        let body_part = MIMEPart {
                            mime_header: content_type.to_string(),
                            mime_body: raw_body.to_string()
                        };
                        body.push(body_part);
                    }
                } else {
                    // Not a multipart message.
                    let body_part = MIMEPart {
                        mime_header: content_type.to_string(),
                        mime_body: raw_body.to_string()
                    };
                    body.push(body_part);
                }
            }
            // No Content Type header so it is not a MIME message
            _ => {
                let non_mime_part = MIMEPart {
                    mime_header: "text/plain".to_string(),
                    mime_body: raw_body.to_string()
                };
                body.push(non_mime_part);
            }
        }
        let message = Message {
            uid: uid,
            path: arg_path.display().to_string(),
            headers: headers,
            body: body,
            flags: flags,
            deleted: false,
            size: size,
            raw_contents: raw_contents.clone(),
            header_boundary: header_boundary
        };

        // We created the message with no errors. Yay!
        Ok(message)
    }

    /// convenience method for determining if Seen is in this message's flags
    pub fn is_unseen(&self) -> bool {
        self.flags.contains(&Seen)
    }

    /// Goes through the list of attributes, constructing a FETCH response for
    /// this message containing the values of the requested attributes 
    pub fn fetch(&self, attributes: &Vec<Attribute>) -> String {
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
            match attr {
                &Envelope => {
                    res.push_str("ENVELOPE ");
                    res.push_str(self.get_envelope().as_slice());
                }, // TODO: Finish implementing this.
                &Flags => {
                    res.push_str("FLAGS ");
                    res.push_str(self.print_flags().as_slice());
                },
                &InternalDate => {
                    res.push_str("INTERNALDATE \"");
                    res.push_str(self.date_received().as_slice());
                    res.push('"');
                }
                &RFC822(ref attr) => {
                    res.push_str("RFC822");
                    match attr {
                        &AllRFC822 => {},
                        &HeaderRFC822 => {
                            res.push_str(".HEADER {");
                            res.push_str(self.header_boundary.to_string()
                                          .as_slice());
                            res.push_str("}\r\n");
                            res.push_str(self.raw_contents.as_slice()
                                          .slice_to(self.header_boundary));
                        },
                        &SizeRFC822 => {
                            res.push_str(".SIZE ");
                            res.push_str(self.size.to_string().as_slice()) },
                        &TextRFC822 => {}
                    };
                },
                &Body => {},
                &BodySection(ref section, ref octets) => {
                    res.push_str(self.get_body(section, octets).as_slice()) },
                &BodyPeek(ref section, ref octets) => {
                    res.push_str(self.get_body(section, octets).as_slice()) },
                &BodyStructure => {
                    /*let content_type: Vec<&str> = self.headers["CONTENT-TYPE".to_string()].as_slice().splitn(1, ';').take(1).collect();
                    let content_type: Vec<&str> = content_type[0].splitn(1, '/').collect();

                    // Retrieve the subtype of the content type.
                    let mut subtype = String::new();
                    if content_type.len() > 1 { subtype = content_type[1].to_string().into_ascii_upper() }

                    let content_type = content_type[0].to_string().into_ascii_upper();
                    println!("Content-type: {}/{}", content_type, subtype);
                    match content_type.as_slice() {
                        "MESSAGE" => {
                            match subtype.as_slice() {
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
                    }*/
                },
                &UID => {
                    res.push_str("UID ");
                    res.push_str(self.uid.to_string().as_slice())
                }
            }
        }
        res
    }

    // Both BodyPeek and BodySection grab parts of the message
    // BodyPeek does not set the Seen flag while BodySection does.
    // Setting the Seen flag is handled in the Session by detecting BodySection
    fn get_body<'a>(&self, section: &'a BodySectionType,
                    _octets: &Option<(uint, uint)>) -> String {
        let empty_string = "".to_string();
        let peek_attr = match section {
            &AllSection => {
                format!("] {{{}}}\r\n{} ", self.raw_contents.as_slice().len(),
                        self.raw_contents)
            }
            &MsgtextSection(ref msgtext) => {
                let msgtext_attr = match msgtext {
                    &HeaderMsgtext => { empty_string },
                    &HeaderFieldsMsgtext(ref fields) => {
                        let mut field_keys = String::new();
                        let mut field_values = String::new();
                        let mut first = true;
                        for field in fields.iter() {
                            match self.headers.find(field) {
                                Some(v) => {
                                    let field_slice = field.as_slice();
                                    if first {
                                        first = false;
                                    } else {
                                        field_keys.push(' ');
                                    }
                                    field_keys.push_str(field_slice);
                                    field_values.push_str("\r\n");
                                    field_values.push_str(field_slice);
                                    field_values.push_str(": ");
                                    field_values.push_str(v.as_slice());
                                },
                                None => continue
                            }
                        }
                        format!("HEADER.FIELDS ({})] {{{}}}{}", field_keys,
                                field_values.as_slice().len(), field_values)
                    },
                    &HeaderFieldsNotMsgtext(_) => { empty_string },
                    &TextMsgtext => { empty_string },
                    &MimeMsgtext => { empty_string }
                };
                msgtext_attr
            }
            &PartSection(_, _) => { "?]".to_string() }
        };
        format!("BODY[{} ", peek_attr)
    }

    /**
     * RFC3501 - 7.4.2 - P.76-77
     *
     * Returns a parenthesized list that described the envelope structure of a
     * message.
     * Computed by parsing the [RFC-2822] header into the component parts,
     * defaulting various fields as necessary.
     *
     * Requires (in the following order): date, subject, from, sender,
     * reply-to, to, cc, bcc, in-reply-to, and message-id.
     * The date, subject, in-reply-to, and message-id fields are strings.
     * The from, sender, reply-to, to, cc, and bcc fields are parenthesized
     * lists of address structures.
     */
    // TODO: Finish implementing this.
    fn get_envelope(&self) -> String {
        let date = self.get_field_or_nil("DATE");
        let subject = self.get_field_or_nil("SUBJECT");
        let from = self.get_parenthesized_addresses("FROM");
        let sender = self.get_parenthesized_addresses("SENDER");
        let reply_to = self.get_parenthesized_addresses("REPLY-TO");
        let to = self.get_parenthesized_addresses("TO");
        let cc = self.get_parenthesized_addresses("CC");
        let bcc = self.get_parenthesized_addresses("BCC");
        let in_reply_to = self.get_field_or_nil("IN-REPLY-TO");
        let message_id = self.get_field_or_nil("MESSAGE-ID");

        format!(
            "(\"{}\" \"{}\" {} {} {} {} {} {} \"{}\" \"{}\")",
            date,
            subject,
            from,
            sender,
            reply_to,
            to,
            cc,
            bcc,
            in_reply_to,
            message_id)
    }

    fn get_field_or_nil(&self, key: &str) -> &str {
        match self.headers.find(&key.to_string()) {
            Some(v) => v.as_slice(),
            None => "NIL"
        }
    }

    /**
     * RFC3501 - 7.4.2 - P.76
     */
    // TODO: Finish implementing this.
    fn get_parenthesized_addresses(&self, key: &str) -> &str {
        match self.headers.find(&key.to_string()) {
            Some(v) => v.as_slice(),
            None => "NIL"
        }
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
            _ => fail!("Unable to determine month!")
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

    // Perform a STORE operation on the message. This involves replacing,
    // adding or removing (as specified by flag_name) the set of new_flags
    // Returns a string containing the new set of flags.
    pub fn store(&mut self, flag_name: StoreName,
                 new_flags: HashSet<Flag>) -> String {
        match flag_name {
            Sub => {
                for flag in new_flags.iter() {
                    self.flags.remove(flag);
                }
            }
            Replace => { self.flags = new_flags; }
            Add => {
                for flag in new_flags.iter() {
                    self.flags.insert(*flag);
                }
            }
        }
        self.deleted = self.flags.contains(&Deleted);
        self.print_flags()
    }

    // Creates a string of the current set of flags based on what is in
    // self.flags.
    fn print_flags(&self) -> String {
        let mut res = "(".to_string();
        let mut first = true;
        for flag in self.flags.iter() {
            // The flags should be space separated.
            if first {
                first = false;
            } else {
                res.push(' ');
            }
            let flag_str = match flag {
                &Answered => { "\\Answered" },
                &Draft => { "\\Draft" },
                &Flagged => { "\\Flagged" },
                &Seen => { "\\Seen" }
                &Deleted => { "\\Deleted" }
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
        if self.flags.len() == 0 {
            return res;
        }

        // Add the prelud which separates the flags
        res.push_str(":2,");

        // As per the Maildir standard, the flags are to be written in
        // alphabetical order
        if self.flags.contains(&Draft) {
            res.push('D');
        }
        if self.flags.contains(&Flagged) {
            res.push('F');
        }
        if self.flags.contains(&Answered) {
            res.push('R');
        }
        if self.flags.contains(&Seen) {
            res.push('S');
        }
        res
    }
}
