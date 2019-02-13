use crate::direntry::*;
use crate::input::*;
use crate::coloring::*;
use crate::drawing::*;
use crate::spawn::*;
use crate::filesystem::*;
use std::path::PathBuf;

#[derive(Clone)]
pub struct RightColumn {
    siblings: Option<Vec<DirEntry>>,
    preview: Option<Vec<String>>,
}

impl RightColumn {
    pub fn collect(path_opt: &Option<PathBuf>,
                   paint_settings: &PaintSettings,
                   sorting_type: &SortingType,
                   include_hidden: bool,
                   max_height: usize, max_width: usize,
                   selected: &Vec<PathBuf>) -> RightColumn {
        if let Some(path) = path_opt {
            if path.is_dir() { // resolved path
                return RightColumn::with_siblings(
                    into_sorted_direntries(
                        collect_maybe_dir(&path, Some(max_height), include_hidden),
                        paint_settings, sorting_type, selected, Some(&path)));
            } else { // resolved path is a regular file
                let path = maybe_resolve_symlink_recursively(path);
                if let Some(preview) = read_preview_of(&path, max_height) {
                    let truncated_preview: Vec<String> = preview.into_iter()
                        .map(|line| maybe_truncate(line.trim_end(), max_width))
                        .collect();
                    return RightColumn::with_preview(truncated_preview);
                }
            }
        }
        RightColumn::empty()
    }

    pub fn with_siblings(siblings: Vec<DirEntry>) -> RightColumn {
        RightColumn {
            siblings: Some(siblings),
            preview: None,
        }
    }

    pub fn with_preview(preview: Vec<String>) -> RightColumn {
        RightColumn {
            siblings: None,
            preview: Some(preview),
        }
    }

    pub fn empty() -> RightColumn {
        RightColumn {
            siblings: None,
            preview: None,
        }
    }

    pub fn siblings_ref(&self) -> Option<&Vec<DirEntry>> {
        self.siblings.as_ref()
    }

    pub fn siblings_mut(&mut self) -> Option<&mut Vec<DirEntry>> {
        self.siblings.as_mut()
    }

    pub fn preview_ref(&self) -> Option<&Vec<String>> {
        self.preview.as_ref()
    }
}

pub fn read_preview_of(path: &PathBuf, max_height: usize) -> Option<Vec<String>> {
    let file_name = path.file_name().unwrap().to_str().unwrap();
    if is_previewable(file_name) {
        let max_per_line = 100;
        Some(read_lines(path, max_height, max_height as u64 * max_per_line))
    } else { None }
}

pub fn is_previewable(file_name: &str) -> bool {
    for exact_name in text_exact_names() {
        if file_name == exact_name { return true; }
    }
    for ext in text_extensions() {
        if file_name.ends_with(ext) { return true; }
    }
    false
}
