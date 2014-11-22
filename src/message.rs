use std::collections::HashMap;

use error::{
    Error, ImapResult, MessageDecodeError
};
use folder::Folder;

#[deriving(Show)]
pub struct Message {
    uid: u32,
    folder_index: u32,
    headers: String,
    body: String,
    flags: Vec<String>,
    date_received: String,
    size: u32,
    envelope_from: String,
    envelope_to: String,
    // TODO: Uncomment this
    //parent_folder: Folder
    raw_contents: String
}

impl Message {
    pub fn parse(filename: String, raw_contents: String) -> ImapResult<Message> {
        // Retrieve the UID from the provided filename.
        let uid = match from_str::<u32>(filename.as_slice()) {
            Some(uid) => uid,
            None => return Err(Error::simple(MessageDecodeError, "Failed to retrieve UID from filename."))
        };

        let contents: Vec<&str> = raw_contents.as_slice().split_str("\n\n").collect();
        let mut contents = contents.into_iter();
        let header = contents.next().unwrap();
        let contents: Vec<&str> = contents.collect();

        // Iterate over the lines of the header in reverse.
        // If a line with leading whitespace is detected, it is merged to the line before it.
        // This "unwraps" the header as indicated in RFC 2822 2.2.3
        let mut new_header: Vec<String> = Vec::new();
        let mut iterator = header.lines().rev();
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

            new_header.push(field.to_string());
        }
        let header: Vec<&String> = new_header.iter().rev().collect();

        let mut header_fields = HashMap::new();
        for line in header.iter() {
            let split: Vec<&str> = line.as_slice().splitn(1, ':').collect();
            header_fields.insert(split[0].clone(), split[1].slice_from(1).clone());
        }
        // Remove the "Received" key from the HashMap.
        header_fields.remove(&"Received");
        println!("Header: {}", header_fields);
        println!("Contents:");
        /*for msg in contents.iter() {
            println!("{}\n-------------------------\n", msg);
        }*/

        let message = Message {
            uid: uid,
            folder_index: 0u32,
            headers: "".to_string(),
            body: "".to_string(),
            flags: Vec::new(),
            date_received: "".to_string(),
            size: 0u32,
            envelope_from: "".to_string(),
            envelope_to: "".to_string(),
            // TODO: Uncomment this
            /*parent_folder: Folder {
                name: "some_folder".to_string(),
                owner: None
            },*/
            raw_contents: raw_contents.clone()
        };

        Ok(message)
    }
}
