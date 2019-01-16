// use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::fs::{self, DirEntry};

#[derive(PartialEq, Eq, Clone)]
pub enum EntryType {
    Regular,
    Directory,
    Symlink,
}

#[derive(Clone)]
pub struct Entry {
    pub entrytype: EntryType,
    pub name: String,
    pub size: u64,
}


pub fn collect_dir(path: &PathBuf) -> Vec<Entry> {
    let mut vec = Vec::new();
    if !path.is_dir() { return vec; }
    let entries = fs::read_dir(path)
                        .expect(&format!("Could not read dir{:?}", path));
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
            size: 4096
        }]
    } else {
        let mut path = path.clone();
        path.pop();
        collect_dir(&path)
    }
}


fn into_entry(dir_entry: DirEntry) -> Entry {
    let name = dir_entry.file_name().to_str().unwrap().to_string();
    let meta = dir_entry.metadata().expect(&format!("Could not read metadata for {}", name));
    let size = meta.len();
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
            panic!("Unknown filetype!");
        }
        entrytype
    };

    Entry {
        entrytype,
        name,
        size,
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
    // panic!(format!("{}", parent_name)); // TODO
    0
    // index_of_name_inside(&parent.to_path_buf(), parent_name).unwrap()
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
