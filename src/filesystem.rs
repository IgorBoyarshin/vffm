// use std::path::Path;
use std::path::PathBuf;
use std::fs::{self, DirEntry, Metadata};
use std::io::BufReader;
use std::io::BufRead;
use std::ffi::OsStr;

//-----------------------------------------------------------------------------
// use std::time::{SystemTime};
// use std::time::{UNIX_EPOCH};
use std::io::{Read};
use std::fs::File;
use std::os::unix::fs::PermissionsExt;

use std::fs::OpenOptions;
use std::io::{Write};
pub fn log(s: &str) {
    // let name = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();
    let name = "log.txt";
    let mut file = OpenOptions::new().append(true).create(true).open(name).unwrap();
    file.write_all(s.as_bytes()).unwrap();
    file.write_all(b"\n").unwrap();
}
//-----------------------------------------------------------------------------
#[derive(Clone)]
pub struct Permissions {
    pub owner: u32,
    pub group: u32,
    pub world: u32,
    pub is_directory: bool,
    pub is_symlink: bool,
}

impl Permissions {
    pub fn string_representation(&self) -> String {
        let dir = (if      self.is_directory {"d"}
                   else if self.is_symlink   {"l"}
                   else                      {"-"}).to_string();
        let owner = permission_number_to_string_representation(self.owner);
        let group = permission_number_to_string_representation(self.group);
        let world = permission_number_to_string_representation(self.world);
        dir + &owner + &group + &world
    }

    fn empty() -> Permissions {
        Permissions {
            owner: 0,
            group: 0,
            world: 0,
            is_directory: false,
            is_symlink: false,
        }
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

fn permissions_from_metadata(metadata: Metadata) -> Permissions {
    let is_file = metadata.is_file();
    let is_directory = metadata.is_dir();
    let is_symlink = !(is_file || is_directory);
    let field = metadata.permissions().mode();
    let world = field % 8;
    let group = (field / 8) % 8;
    let owner = (field / (8*8)) % 8;
    Permissions {
        owner,
        group,
        world,
        is_directory,
        is_symlink,
    }
}

// pub fn permissions_of(path: &PathBuf) -> Permissions {
//     let metadata = fs::symlink_metadata(path);
//     if metadata.is_err() { Permissions::empty() }
//     else                 { permissions_from_metadata(metadata.unwrap()) }
// }
//-----------------------------------------------------------------------------
// pub fn modify_time(path: &PathBuf) -> u64 {
//     fs::metadata(path).expect("Could not read metadata")
//         .modified().expect("Could not read modify time")
//         .duration_since(UNIX_EPOCH).unwrap().as_secs()
// }
// pub fn create_time(path: &PathBuf) -> u64 {
//     fs::metadata(path).expect("Could not read metadata")
//         .created().expect("Could not read create time")
//         .duration_since(UNIX_EPOCH).unwrap().as_secs()
// }
// pub fn access_time(path: &PathBuf) -> u64 {
//     fs::metadata(path).expect("Could not read metadata")
//         .accessed().expect("Could not read access time")
//         .duration_since(UNIX_EPOCH).unwrap().as_secs()
// }
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
    pub permissions: Permissions,
}

// impl Entry {
    // pub fn is_symlink(&self) -> bool {
    //     self.entrytype == EntryType::Symlink
    // }
    // pub fn is_regular(&self) -> bool {
    //     self.entrytype == EntryType::Regular
    // }
    // pub fn is_dir(&self) -> bool {
    //     self.entrytype == EntryType::Directory
    // }
// }

pub fn read_lines(path: &PathBuf, amount: usize, max_bytes: u64) -> Vec<String> {
    let file = File::open(path);
    if file.is_err() { return Vec::new(); }
    let file = file.unwrap();
    let mut reader = BufReader::new(file).take(max_bytes);
    let mut lines = Vec::new();
    for _ in 0..amount {
        let mut line = String::new();
        let result = reader.read_line(&mut line);
        if line.is_empty() { return lines; }
        if result.is_err() { return lines; }
        lines.push(line);
    }
    lines
}

// pub fn read_contents(path: &PathBuf) -> String {
//     let mut file = File::open(path).expect("Could not read file");
//     let mut contents = String::new();
//     file.read_to_string(&mut contents).expect("Could not read contents");
//     contents
// }

pub fn resolve_symlink(path: &PathBuf) -> Option<PathBuf> {
    let resolved = path.read_link();
    if resolved.is_ok() { Some(resolved.unwrap()) }
    else                { None }
}

pub fn is_symlink(path: &PathBuf) -> bool {
    let meta = path.symlink_metadata(); // Does not resolve the symlink
    if meta.is_err() { return false; }
    let meta = meta.unwrap();
    (!meta.is_file() && !meta.is_dir())
}

pub fn is_dir(path: &PathBuf) -> bool {
    let meta = path.symlink_metadata(); // Does not resolve the symlink
    if meta.is_err() { return false; }
    let meta = meta.unwrap();
    meta.is_dir()
}

pub fn file_name(path: &PathBuf) -> String {
    path.file_name().unwrap().to_str().unwrap().to_string()
}

pub fn path_to_str(path: &PathBuf) -> &str {
    path.to_str().unwrap()
}

pub fn path_to_string(path: &PathBuf) -> String {
    path.to_str().unwrap().to_string()
}

pub fn osstr_to_str(osstr: &OsStr) -> &str {
    osstr.to_str().unwrap()
}

pub fn maybe_resolve_symlink_recursively(path: &PathBuf) -> PathBuf {
    if is_symlink(path) {
        if let Some(mut resolved_path) = resolve_symlink(path) {
            if !resolved_path.is_absolute() { // if not absolute, make it one
                resolved_path = path.parent().unwrap().join(resolved_path);
            }
            return maybe_resolve_symlink_recursively(&resolved_path);
        }
    }
    path.clone()
}

// Follows the symlinks
pub fn collect_maybe_dir(path: &PathBuf, max_count: Option<usize>, include_hidden: bool) -> Vec<Entry> {
    let mut vec = Vec::new();
    if path.is_file() { return vec; }
    if !path.is_dir() { // so it is a symlink
        let new_path = path.read_link().expect("Somewhy not a symlink");
        return collect_maybe_dir(&new_path, max_count, include_hidden);
    } // otherwise it is a directory
    let entries = fs::read_dir(path);
    if !entries.is_ok() { return Vec::new(); }
    let entries = entries.expect(&format!("Could not read dir{:?}", path));
    let entries = entries.map(|e| e.expect("Could not retrieve entry"))
                         .filter(|e|
                             if include_hidden { true }
                             else { !e.file_name().to_str().unwrap().starts_with(".") });
    for (index, entry) in entries.enumerate() {
        let dir_entry = entry;
        vec.push(into_entry(dir_entry));
        if let Some(max_count) = max_count {
            if index > max_count { break; }
        }
    }

    vec
}

pub fn collect_siblings_of(path: &PathBuf, include_hidden: bool) -> Vec<Entry> {
    if is_root(&path) {
        vec![Entry {
            entrytype: EntryType::Directory,
            name: "/".to_string(),
            size: 4096,
            time_modified: 0,
            permissions: Permissions::empty(),
        }]
    } else {
        let mut path = path.clone();
        path.pop();
        collect_maybe_dir(&path, None, include_hidden)
    }
}


fn into_entry(dir_entry: DirEntry) -> Entry {
    let name = dir_entry.file_name().to_str().unwrap().to_string();
    let meta = dir_entry.metadata().expect(&format!("Could not read metadata for {}", name));
    let size = meta.len();
    let permissions = permissions_from_metadata(meta);
    let entrytype = {
        let ft = dir_entry.file_type().expect("Could not retrieve filetype");
        if ft.is_file()         { EntryType::Regular }
        else if ft.is_dir()     { EntryType::Directory }
        else if ft.is_symlink() { EntryType::Symlink }
        else                    { EntryType::Unknown }
    };

    Entry {
        entrytype,
        name,
        size,
        time_modified: 0,
        permissions,
    }
}

pub type Size = u64;

pub fn maybe_size(path: &PathBuf) -> Option<Size> {
    let meta = path.symlink_metadata(); // does not follow symlinks
    if meta.is_err() { return None; }
    Some(meta.unwrap().len())
}

fn size(path: &PathBuf) -> Size {
    path.symlink_metadata() // does not follow symlinks
        .expect(&format!("Could not read metadata for {:?}", path))
        .len()
}

pub fn human_size(mut size: u64) -> String {
    if size < 1024 { return size.to_string() + " B"; }

    let mut letter_index = 0;
    let mut full;
    loop {
        full = size / 1024;
        if full < 1024 { break; }
        letter_index += 1;
        size /= 1024;
    }

    let mut string = full.to_string();
    let remainder = size % 1024;
    if remainder != 0 {
        string += ".";
        string += &(remainder * 10 / 1024).to_string();
    }
    string += " ";

    string += "KMGTP".get(letter_index..letter_index+1).expect("Size too large");
    string
}


pub fn cumulative_size(path: &PathBuf) -> Size {
    let meta = path.symlink_metadata();
    if meta.is_err() { return 0; }
    if meta.unwrap().is_dir() { // does not follow symlinks
        fs::read_dir(path).unwrap()
            .map(|entry| entry.unwrap().path())
            .map(|path| cumulative_size(&path))
            .sum()
    } else { size(path) }
}

pub fn absolute_pathbuf() -> PathBuf {
    std::env::current_exe().expect("Cannot get absolute PathBuf")
}

pub fn is_root(path: &PathBuf) -> bool {
    path.parent() == None
}

pub fn maybe_parent(path: &PathBuf) -> Option<PathBuf> {
    if path_to_str(path) == "/" { None }
    else { Some(path.parent().unwrap().to_path_buf()) }
}
