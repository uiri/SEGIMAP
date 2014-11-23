use std::collections::HashMap;
use std::io::File;

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
    raw_contents: String
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

        let mut path = arg_path.filename_str().unwrap().splitn(1, ':');
        let filename = path.next().unwrap();
        let path_flags = path.next();

        // Retrieve the UID from the provided filename.
        let uid = match from_str::<u32>(filename) {
            Some(uid) => uid,
            None => return Err(Error::simple(MessageDecodeError, "Failed to retrieve UID from filename."))
        };
        // Parse the flags from the filename.

        let mut flags = match path_flags {
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
            headers.insert(split[0].clone().to_string(), split[1].slice_from(1).clone().to_string());
        }
        // Remove the "Received" key from the HashMap.
        headers.remove(&"Received".to_string());

        // Determine whether the message is MULTIPART or not.
        let content_type = headers["Content-Type".to_string()].clone();
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
            raw_contents: raw_contents.clone()
        };

        Ok(message)
    }

    // TODO: Make sure that returning a pointer is fine.
    pub fn date_received(&self) -> &String {
        self.headers.find(&"Received-On-Date".to_string()).unwrap()
    }

    // TODO: Make sure that returning a pointer is fine.
    pub fn envelope_from(&self) -> &String {
        self.headers.find(&"From".to_string()).unwrap()
    }

    // TODO: Make sure that returning a pointer is fine.
    pub fn envelope_to(&self) -> &String {
        self.headers.find(&"To".to_string()).unwrap()
    }
}
