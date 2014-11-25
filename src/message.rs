use std::ascii::OwnedAsciiExt;
use std::collections::HashMap;
use std::io::File;

use time;
use time::{
    Timespec,
    Tm,
};

use command::command::{
    Attribute,
    Envelope,
    Flags,
    InternalDate,
    RFC822,
    Body,
    BodyPeek,
    BodyStructure,
    UID
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

#[deriving(Show)]
enum Flag {
    Answered,
    Draft,
    Flagged,
    Seen
}

#[deriving(Show)]
pub struct Message {
    uid: u32,
    pub path: String,
    headers: HashMap<String, String>,
    body: Vec<MIMEPart>,
    flags: Vec<Flag>,
    pub deleted: bool,
    size: uint,
    raw_contents: String,
    raw_header: String
}

#[deriving(Show)]
pub struct MIMEPart {
    mime_header: String,
    mime_body: String
}

impl Message {
    pub fn parse(arg_path: &Path) -> ImapResult<Message> {
        // Load the file contents.
        let file = match File::open(arg_path) {
            Ok(mut file) => match file.read_to_end() {
                Ok(contents) => contents,
                Err(e) => return Err(Error::simple(InternalIoError(e), "Failed to read mail file."))
            },
            Err(e) => return Err(Error::simple(InternalIoError(e), "Failed to open mail file."))
        };
        let raw_contents = String::from_utf8_lossy(file.as_slice()).to_string();
        let size = raw_contents.as_slice().len();

        let mut path = arg_path.filename_str().unwrap().splitn(1, ':');
        let filename = path.next().unwrap();
        let path_flags = path.next();

        // Retrieve the UID from the provided filename.
        let uid = match from_str::<u32>(filename) {
            Some(uid) => uid,
            None => return Err(Error::simple(MessageDecodeError, "Failed to retrieve UID from filename."))
        };
        // Parse the flags from the filename.
        let flags = match path_flags {
            None => Vec::new(),
            Some(flags) => {
                let unparsed_flags = flags.splitn(1, ',').skip(1).next().unwrap();
                let mut vec_flags: Vec<Flag> = Vec::new();
                for flag in unparsed_flags.chars() {
                    let parsed_flag = match flag {
                        'D' => Some(Draft),
                        'F' => Some(Flagged),
                        'R' => Some(Answered),
                        'S' => Some(Seen),
                        _ => None
                    };
                    match parsed_flag {
                        Some(enum_flag) => vec_flags.push(enum_flag),
                        None => { }
                    }
                }
                vec_flags
            }
        };

        let header_boundary = raw_contents.as_slice().find_str("\n\n").unwrap();
        let raw_header = raw_contents.as_slice().slice_to(header_boundary);
        let raw_body = raw_contents.as_slice().slice_from(header_boundary + 2);

        // Iterate over the lines of the header in reverse.
        // If a line with leading whitespace is detected, it is merged to the line before it.
        // This "unwraps" the header as indicated in RFC 2822 2.2.3
        let mut parsed_header: Vec<String> = Vec::new();
        let mut iterator = raw_header.lines().rev();
        loop {
            let line = match iterator.next() {
                Some(line) => line,
                None => break
            };
            let mut field = line.to_string();
            if line.as_slice().starts_with(" ") || line.as_slice().starts_with("\t") {
                field = field.as_slice().trim_left_chars(' ').trim_left_chars('\t').to_string();
                loop {
                    let next = iterator.next().unwrap().to_string();
                    let mut trimmed_next = next.as_slice().trim_left_chars(' ').trim_left_chars('\t').to_string();
                    // Add a space between the merged lines.
                    trimmed_next.push_str(" ".as_slice());
                    trimmed_next.push_str(field.as_slice());
                    field = trimmed_next;
                    if !next.as_slice().starts_with(" ") && !next.as_slice().starts_with("\t") {
                        break;
                    }
                }
            }

            parsed_header.push(field.to_string());
        }
        let parsed_header: Vec<&String> = parsed_header.iter().rev().collect();

        let mut headers = HashMap::new();
        for line in parsed_header.iter() {
            let split: Vec<&str> = line.as_slice().splitn(1, ':').collect();
            headers.insert(split[0].to_string().into_ascii_upper().to_string(), split[1].slice_from(1).clone().to_string());
        }
        // Remove the "Received" key from the HashMap.
        headers.remove(&"RECEIVED".to_string());

        // Determine whether the message is MULTIPART or not.
        let content_type = headers["CONTENT-TYPE".to_string()].clone();
        let mut body = Vec::new();
        if content_type.as_slice().contains("MULTIPART") {
            let mime_boundary = {
                let value: Vec<&str> = content_type.as_slice().split_str("BOUNDARY=\"").collect();
                if value.len() < 2 { return Err(Error::simple(MessageDecodeError, "Failed to determine MULTIPART boundary.")) }
                let value: Vec<&str> = value[1].splitn(1, '"').collect();
                if value.len() < 1 { return Err(Error::simple(MessageDecodeError, "Failed to determine MULTIPART boundary.")) }
                format!("--{}--\n", value[0])
            };

            let first_content_type_index = match raw_body.find_str("Content-Type") {
                Some(val) => val,
                None => return Err(Error::simple(MessageDecodeError, "Missing Content-Type for body part"))
            };
            let raw_body = raw_body.slice_from(first_content_type_index);
            let raw_body: Vec<&str> = raw_body.split_str(mime_boundary.as_slice()).collect();
            let raw_body = raw_body.slice_to(raw_body.len() - 1);

            for part in raw_body.iter() {
                let header_boundary = part.as_slice().find_str("\n\n").unwrap();
                let header = part.as_slice().slice_to(header_boundary);
                let mut content_type = String::new();
                for line in header.lines() {
                    let split_line: Vec<&str> = line.splitn(1, ':').collect();
                    if split_line[0] == "Content-Type" {
                        let content_type_values: Vec<&str> = split_line[1].splitn(1, ';').collect();
                        content_type = content_type_values[0].slice_from(1).to_string();
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
            let body_part = MIMEPart {
                mime_header: content_type.to_string(),
                mime_body: raw_body.to_string()
            };
            body.push(body_part);
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
            raw_header: raw_header.to_string()
        };

        Ok(message)
    }

    // TODO: Make sure that returning a pointer is fine.
    pub fn envelope_from(&self) -> &String {
        self.headers.find(&"FROM".to_string()).unwrap()
    }

    // TODO: Make sure that returning a pointer is fine.
    pub fn envelope_to(&self) -> &String {
        self.headers.find(&"TO".to_string()).unwrap()
    }

    pub fn fetch(&self, attributes: &Vec<Attribute>) -> String {
        let mut res = String::new();
        for attr in attributes.iter() {
            let attr_str = match attr {
                &Envelope => { format!(" ENVELOPE {}", self.get_envelope()) }, // TODO: Finish implementing this.
                &Flags => { "".to_string() },
                &InternalDate => { format!(" INTERNALDATE \"{}\"", self.date_received()) }
                &RFC822(ref attr) => {
                    let rfc_attr = match attr {
                        &AllRFC822 => { "".to_string() },
                        &HeaderRFC822 => { format!(".HEADER {{{}}}\n{}", self.raw_header.len(), self.raw_header) },
                        &SizeRFC822 => { format!(".SIZE {}", self.size) },
                        &TextRFC822 => { "".to_string() }
                    };
                    format!(" RFC822{}", rfc_attr)
                },
                &Body(ref section, ref octets) => { "".to_string() },
                &BodyPeek(ref section, ref octets) => { "".to_string() },
                &BodyStructure => { "".to_string() },
                &UID => { format!(" UID {}", self.uid) }
            };
            res = format!("{}{}", res, attr_str);
        }
        res
    }

    /**
     * RFC3501 - 7.4.2 - P.76-77
     *
     * Returns a parenthesized list that described the envelope structure of a
     * message.
     * Computed by parsing the [RFC-2822] header into the component parts,
     * defaulting various fields as necessary.
     *
     * Requires (in the following order): date, subject, from, sender, reply-to,
     * to, cc, bcc, in-reply-to, and message-id.
     * The date, subject, in-reply-to, and message-id fields are strings.
     * The from, sender, reply-to, to, cc, and bcc fields are parenthesized
     * lists of address structures.
     */
    // TODO: Finish implementing this.
    fn get_envelope(&self) -> String {
        let date = self.get_quoted_field_or_nil("DATE".to_string());
        let subject = self.get_quoted_field_or_nil("SUBJECT".to_string());
        let from = self.get_parenthesized_addresses("FROM".to_string());
        let sender = self.get_parenthesized_addresses("SENDER".to_string());
        let reply_to = self.get_parenthesized_addresses("REPLY-TO".to_string());
        let to = self.get_parenthesized_addresses("TO".to_string());
        let cc = self.get_parenthesized_addresses("CC".to_string());
        let bcc = self.get_parenthesized_addresses("BCC".to_string());
        let in_reply_to = self.get_quoted_field_or_nil("IN-REPLY-TO".to_string());
        let message_id = self.get_quoted_field_or_nil("MESSAGE-ID".to_string());

        format!(
            "({} {} {} {} {} {} {} {} {} {})",
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

    fn get_quoted_field_or_nil(&self, key: String) -> String {
        match self.headers.find(&key) {
            Some(v) => format!("\"{}\"", v),
            None => "NIL".to_string()
        }
    }

    /**
     * RFC3501 - 7.4.2 - P.76
     */
    // TODO: Finish implementing this.
    fn get_parenthesized_addresses(&self, key: String) -> String {
        let addresses = match self.headers.find(&key) {
            Some(v) => v, // TODO: this is not parenthesized.
            None => return "NIL".to_string()
        };

        addresses.clone()
    }

    fn date_received(&self) -> String {
        // Retrieve the date received from the UID.
        let date_received = Timespec { sec: self.uid as i64, nsec: 0i32 };
        let date_received_tm = time::at_utc(date_received);

        let month = match date_received_tm.tm_mon {
            0 => "Jan".to_string(),
            1 => "Feb".to_string(),
            2 => "Mar".to_string(),
            3 => "Apr".to_string(),
            4 => "May".to_string(),
            5 => "Jun".to_string(),
            6 => "Jul".to_string(),
            7 => "Aug".to_string(),
            8 => "Sep".to_string(),
            9 => "Oct".to_string(),
            10 => "Nov".to_string(),
            11 => "Dec".to_string(),
            _ => fail!("Unable to determine month!") // NOTE: this should never happen.
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
