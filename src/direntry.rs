use crate::filesystem::*;
use crate::coloring::*;
use crate::input::*;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone)]
pub struct DirEntry {
    pub entrytype: EntryType,
    pub name: String,
    pub size: u64,
    pub time_modified: u64,
    pub permissions: Permissions,

    pub paint: Paint,
    pub is_selected: bool,
}

impl DirEntry {

    pub fn from_entry(entry: Entry, paint_settings: &PaintSettings, is_selected: bool) -> DirEntry {
        let executable = is_partially_executable(&entry);
        let paint = paint_for(&entry.entrytype, &entry.name, executable, paint_settings);
        DirEntry {
            entrytype: entry.entrytype,
            name: entry.name,
            size: entry.size,
            time_modified: entry.time_modified,
            permissions: entry.permissions,
            paint,
            is_selected,
        }
    }

    pub fn is_symlink(&self) -> bool {
        self.entrytype == EntryType::Symlink
    }
    // pub fn is_regular(&self) -> bool {
    //     self.entrytype == EntryType::Regular
    // }
    pub fn is_dir(&self) -> bool {
        self.entrytype == EntryType::Directory
    }
}

// TODO: move all
pub type Millis = u128;

pub struct Notification {
    pub text: String,
    show_time_millis: Millis,
    start_time: SystemTime,
}

impl Notification {
    pub fn new(text: &str, show_time_millis: Millis) -> Notification {
        let text = text.to_string();
        Notification {
            text,
            show_time_millis,
            start_time: SystemTime::now(),
        }
    }

    pub fn has_finished(&self) -> bool {
        millis_since(self.start_time) > self.show_time_millis
    }
}

pub fn millis_since(time: SystemTime) -> Millis {
    let elapsed = SystemTime::now().duration_since(time);
    if elapsed.is_err() { return 0; } // _now_ is earlier than _time_ => assume 0
    elapsed.unwrap().as_millis()
}

pub fn is_partially_executable(entry: &Entry) -> bool {
    (entry.permissions.world % 2 == 1) ||
    (entry.permissions.group % 2 == 1) ||
    (entry.permissions.owner % 2 == 1)
}

pub fn get_additional_entry_info(entry: Option<&DirEntry>, path: &Option<PathBuf>)
        -> Option<String> {
    if let Some(entry) = entry {
        if entry.is_dir() { // get sub-entries count
            // if let Some(siblings) = right_column.siblings_ref() {
            //     let count = siblings.len().to_string();
            //     let text = "Entries inside: ".to_string() + &count;
            //     return Some(text);
            // }
        } else if entry.is_symlink() { // get the path the link points to
            if let Some(result) = get_symlink_target(path) {
                let text = "-> ".to_string() + &result;
                return Some(text);
            }
        }
    }
    None
}

pub fn get_symlink_target(path: &Option<PathBuf>) -> Option<String> {
    if let Some(path) = path {
        if is_symlink(path) {
            if let Some(resolved) = resolve_symlink(path) {
                return Some(path_to_str(&resolved).to_string());
            }
        }
    }
    None
}


pub fn into_direntries(entries: Vec<Entry>,
                   paint_settings: &PaintSettings,
                   selected: &Vec<PathBuf>,
                   parent_path: Option<&PathBuf>) -> Vec<DirEntry> {
    entries.into_iter().map(|entry| {
        let is_selected = is_selected(selected, &entry.name, parent_path);
        DirEntry::from_entry(entry, paint_settings, is_selected)
    }).collect()
}

pub fn is_selected(selected: &Vec<PathBuf>, name: &str, parent_path: Option<&PathBuf>) -> bool {
    if let Some(parent_path) = parent_path {
        return selected.iter().any(|item| item.ends_with(name) // check partially
                            && *item == parent_path.join(name)); // verify
    } else { false }
}


pub fn into_sorted_direntries(entries: Vec<Entry>,
                          paint_settings: &PaintSettings,
                          sorting_type: &SortingType,
                          selected: &Vec<PathBuf>,
                          parent_path: Option<&PathBuf>) -> Vec<DirEntry> {
    let entries = into_direntries(entries, paint_settings, selected, parent_path);
    sort(entries, sorting_type)
}

fn sort(mut entries: Vec<DirEntry>, sorting_type: &SortingType) -> Vec<DirEntry> {
    match sorting_type {
        SortingType::Lexicographically => entries.sort_by(
            |a, b| a.name.cmp(&b.name)),
        SortingType::TimeModified => entries.sort_by(
            |a, b| a.time_modified.cmp(&b.time_modified)),
        SortingType::Any => {},
    }
    entries
}

pub fn path_of_nth_entry_inside(n: usize, path: &PathBuf, entries: &Vec<DirEntry>) -> Option<PathBuf> {
    if entries.is_empty() { return None; }
    if n >= entries.len() { return None; }
    let name = entries[n].name.clone();
    let mut path = path.clone();
    path.push(name);
    Some(path)
}

pub fn nth_entry_inside(n: usize, entries: &Vec<DirEntry>) -> Option<&DirEntry> {
    if entries.is_empty() { return None; }
    if n >= entries.len() { return None; }
    Some(entries.get(n).unwrap())
}

pub fn index_of_entry_inside(path: &PathBuf, entries: &Vec<DirEntry>) -> Option<usize> {
    if is_root(path) { return Some(0); }
    let sought_name = path.file_name().unwrap().to_str().unwrap();
    for (index, DirEntry {name, ..}) in entries.iter().enumerate() {
        if sought_name == name { return Some(index); }
    }
    None
}

pub fn string_permissions_for_entry(entry: &Option<&DirEntry>) -> String {
    if let Some(entry) = entry {
        entry.permissions.string_representation()
    } else { "".to_string() }
}
