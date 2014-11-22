use std::io::fs;

pub struct Folder {
    pub name: String,
    pub owner: Option<Box<Folder>>,
    cur: Vec<Path>,
    new: Vec<Path>,
    tmp: Vec<Path>
}

macro_rules! make_vec_path(
    ($path:ident, $inp:ident, $next:expr) => ({
        match fs::readdir(&($path.join("$inp"))) {
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
        make_vec_path!(path, cur,
            make_vec_path!(path, new,
                make_vec_path!(path, tmp,
                    return Some(Folder {
                        name: name,
                        owner: owner,
                        cur: cur,
                        new: new,
                        tmp: tmp
                    })
                )
            )
        );
    }
}
