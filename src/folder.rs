use std::io::fs;

pub struct Folder {
    pub name: String,
    pub cur: Vec<Path>,
    pub new: Vec<Path>,
    pub tmp: Vec<Path>
}

macro_rules! make_vec_path(
    ($path:ident, $inp:ident) => {
        match fs::readdir(&($path.join("$inp"))) {
            Err(_) => { return None; }
            Ok(res) => { $inp = res; }
        }
    };
)

impl Folder {
    pub fn new(name: String, path_str: String) -> Option<Folder> {
        let path = Path::new(path_str);
        let mut cur = Vec::new();
        make_vec_path!(path, cur);
        let mut new = Vec::new();
        make_vec_path!(path, new);
        let mut tmp = Vec::new();
        make_vec_path!(path, tmp);
        return Some(Folder {
            name: name,
            cur: cur,
            new: new,
            tmp: tmp
        });
    }
}
