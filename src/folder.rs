use std::io::fs;
use std::fmt::{Show, Formatter, FormatError};

#[deriving(Decodable, Encodable)]
pub struct Folder {
    pub name: String,
    pub owner: Option<Box<Folder>>,
    recent: uint,
    exists: uint,
    path: Path,
    cur: Vec<Path>,
    new: Vec<Path>,
    tmp: Vec<Path>
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
    pub fn new(name: String, owner: Option<Box<Folder>>, path: Path) -> Option<Folder> {
        match fs::File::open(&path.join(".lock")) {
            Err(_) => {
                match fs::File::create(&path.join(".lock")) {
                    Ok(mut file) => {
                        file.write(b"selected");
                        drop(file);
                        make_vec_path!(path, cur, "cur",
                           make_vec_path!(path, new, "new",
                               make_vec_path!(path, tmp, "tmp",
                                   return Some(Folder {
                                       name: name,
                                       owner: owner,
                                       path: path,
                                       recent: new.len(),
                                       exists: cur.len() + new.len(),
                                       cur: cur,
                                       new: new,
                                       tmp: tmp
                                   })
                               )
                           )
                       );
                    }
                    _ => { return None; }
                }
            }
            _ => { return None; }
        }
    }

    pub fn exists(&self) -> uint {
        return self.exists;
    }

    pub fn recent(&self) -> uint {
        for msg in self.new.iter() {
            match msg.filename_str() {
                Some(filename) => {
                    fs::rename(msg, &self.path.join("cur").join(filename));
                }
                _ => {}
            }
        }
        self.recent
    }

    pub fn close(&self) {
        fs::unlink(&self.path.join(".lock"));
    }
}

impl Show for Folder {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
        self.name.fmt(f)
    }
}
