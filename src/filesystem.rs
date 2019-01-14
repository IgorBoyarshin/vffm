// use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::fs::{self};

#[derive(PartialEq, Eq)]
pub enum EntryType {
    Regular,
    Directory,
    Symlink,
}

pub struct Entry {
    pub entrytype: EntryType,
    pub name: String,
    pub size: u64,
}


pub fn collect_dir_pathbuf(pathbuf: &PathBuf) -> Vec<Entry> {
    collect_dir(pathbuf.to_str().unwrap())
}

pub fn collect_dir(path: &str) -> Vec<Entry> {
    let mut vec = Vec::new();
    let entries = fs::read_dir(Path::new(path))
                        .expect(&format!("Could not read dir{}", path));
    for entry in entries {
        let p = entry.expect("Could not retrieve entry");
        let name = p.file_name().to_str().unwrap().to_string();
        let meta = p.metadata().expect(&format!("Could not read metadata for {}", name));
        let size = meta.len();
        let entrytype = {
            let ft = p.file_type().expect("Could not retrieve filetype");
            let entrytype: EntryType;
            if ft.is_file() {
                entrytype = EntryType::Regular;
            } else if ft.is_dir() {
                entrytype = EntryType::Directory;
            } else if ft.is_symlink() {
                entrytype = EntryType::Symlink;
            } else {
                panic!("Unknown filetype!");
            }
            entrytype
        };
        vec.push(Entry {
            entrytype,
            name,
            size,
        });
    }

    vec
}

pub fn files_in_dir(pathbuf: &PathBuf) -> usize {
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
