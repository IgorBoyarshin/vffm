use crate::direntry::*;

#[derive(Clone)]
pub enum InputMode {
    Search(SearchTools),
    ChangeName(ChangeNameTools),
    Command(CommandTools),
}

#[derive(Clone)]
pub struct SearchTools {
    pub query: String,
    pub cursor_index: Option<usize>, // None if not if focus
    pub current_siblings_backup: Vec<DirEntry>,
}

#[derive(Clone)]
pub struct ChangeNameTools {
    pub new_name: String,
    pub cursor_index: usize,
}

#[derive(Clone)]
pub struct CommandTools {
    pub text: String,
    pub cursor_index: usize,
}
