// use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::fs::{self, DirEntry};


//-----------------------------------------------------------------------------
use std::time::{SystemTime, UNIX_EPOCH};
use std::fs::OpenOptions;
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::io::Write;

pub fn log(s: &str) {
    // let name = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();
    let name = "log.txt";
    let mut file = OpenOptions::new().append(true).create(true).open(name).unwrap();
    file.write_all(s.as_bytes());
    file.write_all(b"\n");
}
//-----------------------------------------------------------------------------
pub struct Permissions {
    owner: u32,
    group: u32,
    world: u32,
    is_directory: bool,
}

impl Permissions {
    pub fn string_representation(&self) -> String {
        let dir = (if self.is_directory {"d"} else {"-"}).to_string();
        let owner = permission_number_to_string_representation(self.owner);
        let group = permission_number_to_string_representation(self.group);
        let world = permission_number_to_string_representation(self.world);
        dir + &owner + &group + &world
    }
}

fn permission_number_to_string_representation(mut n: u32) -> String {
    let mut s = String::new();
    if n >= 4 {
        s.push('r');
        n -= 4;
    } else { s.push('-'); }
    if n >= 2 {
        s.push('w');
        n -= 2;
    } else { s.push('-'); }
    if n >= 1 {
        s.push('x');
        n -= 1;
    } else { s.push('-'); }
    assert!(n == 0);

    s
}

pub fn permissions_of(path: &PathBuf) -> Permissions {
    let metadata = fs::metadata(path);
    if metadata.is_err() { return Permissions {
        owner: 0,
        group: 0,
        world: 0,
        is_directory: false,
    }};
    let metadata = metadata.unwrap();
    let is_directory = metadata.is_dir();
    let field = metadata.permissions().mode();
    let world = field % 8;
    let group = (field / 8) % 8;
    let owner = (field / (8*8)) % 8;
    Permissions {
        owner,
        group,
        world,
        is_directory,
    }
}
//-----------------------------------------------------------------------------
pub fn modify_time(path: &PathBuf) -> u64 {
    fs::metadata(path).expect("Could not read metadata")
        .modified().expect("Could not read modify time")
        .duration_since(UNIX_EPOCH).unwrap().as_secs()
}
pub fn create_time(path: &PathBuf) -> u64 {
    fs::metadata(path).expect("Could not read metadata")
        .created().expect("Could not read create time")
        .duration_since(UNIX_EPOCH).unwrap().as_secs()
}
pub fn access_time(path: &PathBuf) -> u64 {
    fs::metadata(path).expect("Could not read metadata")
        .accessed().expect("Could not read access time")
        .duration_since(UNIX_EPOCH).unwrap().as_secs()
}
//-----------------------------------------------------------------------------
#[derive(PartialEq, Eq, Clone)]
pub enum EntryType {
    Regular,
    Directory,
    Symlink,
    Unknown,
}

#[derive(Clone)]
pub struct Entry {
    pub entrytype: EntryType,
    pub name: String,
    pub size: u64,
    pub time_modified: u64,
}

impl Entry {
    pub fn is_symlink(&self) -> bool {
        self.entrytype == EntryType::Symlink
    }
    pub fn is_regular(&self) -> bool {
        self.entrytype == EntryType::Regular
    }
    pub fn is_dir(&self) -> bool {
        self.entrytype == EntryType::Directory
    }
}


pub fn collect_maybe_dir(path: &PathBuf) -> Vec<Entry> {
    let mut vec = Vec::new();
    if !path.is_dir() { return vec; }
    let entries = fs::read_dir(path);
    if !entries.is_ok() { return Vec::new(); }
    let entries = entries.expect(&format!("Could not read dir{:?}", path));
    for entry in entries {
        let dir_entry = entry.expect("Could not retrieve entry");
        vec.push(into_entry(dir_entry));
    }

    vec
}

pub fn collect_siblings_of(path: &PathBuf) -> Vec<Entry> {
    if is_root(&path) {
        vec![Entry {
            entrytype: EntryType::Directory,
            name: "/".to_string(),
            size: 4096,
            time_modified: 0,
        }]
    } else {
        let mut path = path.clone();
        path.pop();
        collect_maybe_dir(&path)
    }
}


fn into_entry(dir_entry: DirEntry) -> Entry {
    let name = dir_entry.file_name().to_str().unwrap().to_string();
    let meta = dir_entry.metadata().expect(&format!("Could not read metadata for {}", name));
    let size = meta.len();
    // let time_modified = meta.modified().expect("Could not read modified time")
    //     .duration_since(UNIX_EPOCH).unwrap().as_secs();
    let entrytype = {
        let ft = dir_entry.file_type().expect("Could not retrieve filetype");
        let entrytype: EntryType;
        if ft.is_file() {
            entrytype = EntryType::Regular;
        } else if ft.is_dir() {
            entrytype = EntryType::Directory;
        } else if ft.is_symlink() {
            entrytype = EntryType::Symlink;
        } else {
            entrytype = EntryType::Unknown;
            // panic!("Unknown filetype!");
        }
        entrytype
    };

    Entry {
        entrytype,
        name,
        size,
        time_modified: 0,
    }
}

pub fn first_entry_inside(pathbuf: &PathBuf) -> Option<Entry> {
    let result = fs::read_dir(pathbuf)
        .expect(&format!("Could not read dir{}", pathbuf.to_str().expect("")))
        .nth(0);
    if let Some(entry) = result {
        Some(into_entry(entry.unwrap()))
    } else { None }
}

pub fn index_of_name_inside(pathbuf: &PathBuf, name: &str) -> Option<usize> {
    let result = fs::read_dir(pathbuf)
        .expect(&format!("Could not read dir{}", pathbuf.to_str().expect("")))
        .into_iter()
        .map(|elem| elem.unwrap().file_name().to_str().unwrap().to_string())
        .enumerate()
        .find(|(_, elem)| elem == &name);
    if let Some((index, _)) = result {
        Some(index)
    } else { None }
}

pub fn index_inside(path: &PathBuf) -> usize {
    if is_root(path) { return 0; }

    let name = path.file_name().unwrap().to_str().unwrap();
    let parent = path.parent().unwrap().to_path_buf();
    index_of_name_inside(&parent, name).unwrap()
}

pub fn get_parent_index(path: &PathBuf) -> usize {
    if is_root(path) {
        panic!("get_parent_index: given path is root");
    }

    let parent = path.parent().unwrap();
    let parent_name = parent.file_name();
    if let None = parent_name {
        return 0; // the index of '/' is always 0 (it is the only one there)
    }
    let parent_name = parent_name.unwrap().to_str().unwrap();
    0
}

pub fn files_count_in_dir(pathbuf: &PathBuf) -> usize {
    fs::read_dir(pathbuf)
        .expect(&format!("Could not read dir{}", pathbuf.to_str().expect("")))
        .count()
}

pub fn absolute_path() -> String {
    std::env::current_exe().expect("Cannot get absolute path")
        .to_str().unwrap().to_string()
}

pub fn absolute_pathbuf() -> PathBuf {
    std::env::current_exe().expect("Cannot get absolute PathBuf")
}

pub fn is_root(path: &PathBuf) -> bool {
    path.parent() == None
}
