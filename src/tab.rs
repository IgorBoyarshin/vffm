use crate::context::*;
use std::path::PathBuf;
use crate::filesystem::*;

#[derive(Clone)]
pub struct Tab {
    pub name: String,
    pub context: Context,
}

pub fn tab_name_from_path(path: &PathBuf) -> String {
    if path == &PathBuf::from("/") { "/".to_string() }
    else { osstr_to_str(path.file_name().unwrap()).to_string() }
}
