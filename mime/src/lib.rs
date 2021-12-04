use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str;

pub use self::command::BodySectionType;
use self::command::BodySectionType::{
    AllSection,
    MsgtextSection,
    PartSection
};

pub use self::command::Msgtext;
use self::command::Msgtext::{
    HeaderMsgtext,
    HeaderFieldsMsgtext,
    HeaderFieldsNotMsgtext,
    TextMsgtext,
    MimeMsgtext
};

pub use self::error::Error;
use self::error::Result as MimeResult;

mod error;
mod command;

static RECEIVED: &'static str = "RECEIVED";

#[derive(Debug, Clone)]
pub struct Message {
   // maps header field names to values
    headers: HashMap<String, String>,

    // contains the MIME Parts (if more than one) of the message
    body: Vec<MIMEPart>,

     // size stored in case FETCH asks for it
    size: usize,

    // the raw contents of the file representing the message
    raw_contents: String,

    // where in raw_contents the header ends and the body begins
    header_boundary: usize
}

/// Representation of a MIME message part
#[derive(Debug, Clone)]
struct MIMEPart {
    mime_header: String,
    mime_body: String
}

impl Message {
    pub fn new(arg_path: &Path) -> MimeResult<Message> {
        // Load the file contents.
        let mut file = File::open(arg_path)?;
        let mut raw_contents = String::new();
        file.read_to_string(&mut raw_contents)?;

        // This slice will avoid copying later
        let size = raw_contents.len();

        // Find boundary between header and body.
        // Use it to create &str of the raw header and raw body
        let header_boundary = match raw_contents.find("\n\n") {
            None => { return Err(Error::ParseMultipartBoundary); }
            Some(n) => n + 1
        };
        let raw_header = &raw_contents[ .. header_boundary];
        let raw_body = &raw_contents[header_boundary .. ];

        // Iterate over the lines of the header in reverse.
        // If a line with leading whitespace is detected, it is merged to the
        // line before it.
        // This "unfolds" the header as indicated in RFC 2822 2.2.3
        let mut iterator = raw_header.lines().rev();
        let mut headers = HashMap::new();
        while let Some(line) = iterator.next() {
            if line.starts_with(' ') || line.starts_with('\t') {
                while let Some(next) = iterator.next() {
                    let mut trimmed_next = next.trim_start_matches(' ')
                                            .trim_start_matches('\t').to_string();

                    // Add a space between the merged lines.
                    trimmed_next.push(' ');
                    trimmed_next.push_str(line.trim_start_matches(' ')
                                           .trim_start_matches('\t'));
                    if !next.starts_with(' ') && !next.starts_with('\t') {
                        let split: Vec<&str> = (&trimmed_next[..])
                                                .splitn(2, ':').collect();
                        headers.insert(split[0].to_ascii_uppercase(),
                                       split[1][1 .. ].to_string());
                        break;
                    }
                }
            } else {
                let split: Vec<&str> = line.splitn(2, ':').collect();
                headers.insert(split[0].to_ascii_uppercase(),
                               split[1][1 .. ].to_string());
            }
        }

        // Remove the "Received" key from the HashMap.
        let received_key = &RECEIVED.to_string();
        if headers.get(received_key).is_some() {
            headers.remove(received_key);
        }

        // Determine whether the message is MULTIPART or not.
        let mut body = Vec::new();
        match headers.get(&"CONTENT-TYPE".to_string()) {
            Some(content_type) => {
                if (&content_type[..]).contains("MULTIPART") {
                    // We need the boundary to determine where this part ends
                    let mime_boundary = {
                        let value: Vec<&str> = (&content_type[..])
                                                .split("BOUNDARY=\"")
                                                .collect();
                        if value.len() < 2 {
                            return Err(Error::ParseMultipartBoundary)
                        }
                        let value: Vec<&str> = value[1].splitn(2, '"')
                                                .collect();
                        if value.len() < 1 {
                            return Err(Error::ParseMultipartBoundary)
                        }
                        format!("--{}--\n", value[0])
                    };

                    // Grab the content type for this part
                    let first_content_type_index =
                        match raw_body.find("Content-Type") {
                            Some(val) => val,
                            None => return Err(Error::MissingContentType),
                    };
                    let mime_boundary_slice = &mime_boundary[..];
                    let raw_body = &raw_body[first_content_type_index .. ];
                    let raw_body: Vec<&str> = raw_body.split(
                        mime_boundary_slice).collect();
                    let raw_body_end = raw_body.len() - 1;
                    let raw_body = &raw_body[ .. raw_body_end];

                    // Throw the parts of the message into a list of MIMEParts
                    for part in raw_body.iter() {
                        let header_boundary = match part.find("\n\n") {
                            None => return Err(Error::ParseMultipartBoundary),
                            Some(n) => n
                        };
                        let header = &part[ .. header_boundary];
                        let mut content_type = String::new();
                        for line in header.lines() {
                            let split_line: Vec<&str> = line.splitn(2, ':')
                                                         .collect();
                            if split_line[0] == "Content-Type" {
                                let content_type_values: Vec<&str> =
                                    split_line[1].splitn(2, ';').collect();
                                content_type = content_type_values[0][1 .. ].to_string();
                                break;
                            }
                        }
                        let body_part = MIMEPart {
                            mime_header: content_type.to_string(),
                            // TODO: double check that this is working as
                            // intended.
                            mime_body: part.to_string()
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
            headers: headers,
            body: body,
            size: size,
            raw_contents: raw_contents.to_string(),
            header_boundary: header_boundary
        };

        // We created the message with no errors. Yay!
        Ok(message)
    }

    // Both BodyPeek and BodySection grab parts of the message
    // BodyPeek does not set the Seen flag while BodySection does.
    // Setting the Seen flag is handled in the Session by detecting BodySection
    pub fn get_body<'a>(&self, section: &'a BodySectionType,
                    _octets: &Option<(usize, usize)>) -> String {
        let empty_string = "".to_string();
        let peek_attr = match *section {
            AllSection => {
                format!("] {{{}}}\r\n{} ", (&self.raw_contents[..]).len(),
                        self.raw_contents)
            }
            MsgtextSection(ref msgtext) => {
                match *msgtext {
                    HeaderMsgtext |
                        HeaderFieldsNotMsgtext(_) |
                        TextMsgtext |
                        MimeMsgtext => { empty_string },
                    HeaderFieldsMsgtext(ref fields) => {
                        let mut field_keys = String::new();
                        let mut field_values = String::new();
                        let mut first = true;
                        for field in fields.iter() {
                            match self.headers.get(field) {
                                Some(v) => {
                                    let field_slice = &field[..];
                                    if first {
                                        first = false;
                                    } else {
                                        field_keys.push(' ');
                                    }
                                    field_keys.push_str(field_slice);
                                    field_values.push_str("\r\n");
                                    field_values.push_str(field_slice);
                                    field_values.push_str(": ");
                                    field_values.push_str(&v[..]);
                                },
                                None => continue
                            }
                        }
                        format!("HEADER.FIELDS ({})] {{{}}}{}", field_keys,
                                &field_values[..].len(), field_values)
                    },
                }
            }
            PartSection(_, _) => { "?]".to_string() }
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
    pub fn get_envelope(&self) -> String {
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

    pub fn get_field_or_nil(&self, key: &str) -> &str {
        match self.headers.get(&key.to_string()) {
            Some(v) => &v[..],
            None => "NIL"
        }
    }

    /**
     * RFC3501 - 7.4.2 - P.76
     *
     * The RFC requests that the data be returned as a parenthesized list, but
     * the current format is also acceptible by most mail clients.
     */
    pub fn get_parenthesized_addresses(&self, key: &str) -> &str {
        match self.headers.get(&key.to_string()) {
            Some(v) => &v[..],
            None => "NIL"
        }
    }

    pub fn get_size(&self) -> String {
        self.size.to_string()
    }

    pub fn get_header_boundary(&self) -> String {
        self.header_boundary.to_string()
    }

    pub fn get_header(&self) -> &str {
        &self.raw_contents[ .. self.header_boundary]
    }
}
