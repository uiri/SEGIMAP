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

        /*println!("Header: {}", header);
        for line in header.lines() {
            let split = line.as_slice().splitn(1, ':');
            match split {
                Some("Recieved", _) => println!("RECEIVED"),
                _ => { }
            }
        }*/
        println!("Contents:");
        for msg in contents.iter() {
            //println!("{}\n-------------------------\n", msg);
        }

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
