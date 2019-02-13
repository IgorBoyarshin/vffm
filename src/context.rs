use crate::direntry::*;
use crate::filesystem::*;
use crate::right_column::*;
use crate::input_mode::*;
use crate::drawing::*;
use crate::coloring::*;
use crate::input::*;

use std::path::PathBuf;

#[derive(Clone)]
pub struct Context {
    pub current_siblings: Vec<DirEntry>,
    pub parent_siblings: Vec<DirEntry>,
    pub right_column: RightColumn, // depends on display_settings

    pub current_path: Option<PathBuf>,
    pub parent_path: PathBuf,

    pub parent_index: usize,
    pub current_index: usize,

    pub parent_siblings_shift: usize, // depends on display_settings
    pub current_siblings_shift: usize, // depends on display_settings

    pub current_permissions: String,
    pub additional_entry_info: Option<String>,
    pub cumulative_size_text: Option<String>,

    pub input_mode: Option<InputMode>,
}

impl Context {
    pub fn generate(parent_path: PathBuf,
                display_settings: &DisplaySettings,
                paint_settings: &PaintSettings,
                sorting_type: &SortingType,
                include_hidden: bool,
                selected: &Vec<PathBuf>) -> Context {
        let current_siblings =
            into_sorted_direntries(
                collect_maybe_dir(&parent_path, None, include_hidden),
                paint_settings, sorting_type,
                selected, Some(&parent_path));
        let parent_siblings =
            into_sorted_direntries(
               collect_siblings_of(&parent_path, include_hidden),
               paint_settings, sorting_type, selected,
               maybe_parent(&parent_path).as_ref());
        let first_entry_path = path_of_nth_entry_inside(0, &parent_path, &current_siblings);
        let first_entry_ref = nth_entry_inside(0, &current_siblings);
        let parent_index = index_of_entry_inside(&parent_path, &parent_siblings).unwrap();
        let current_index = 0;
        let column_index = 2;
        let (begin, end) = display_settings.columns_coord[column_index];
        let column_width = (end - begin) as usize;
        let right_column =
            RightColumn::collect(
                &first_entry_path, paint_settings, sorting_type, include_hidden,
                display_settings.column_effective_height, column_width, selected);
        let parent_siblings_shift =
            siblings_shift_for(
                display_settings.scrolling_gap,
                display_settings.column_effective_height,
                parent_index, parent_siblings.len(), None);
        let current_siblings_shift =
            siblings_shift_for(
                display_settings.scrolling_gap,
                display_settings.column_effective_height,
                current_index, current_siblings.len(), None);
        Context {
            parent_index,
            current_index,
            parent_siblings,
            current_permissions: string_permissions_for_entry(&first_entry_ref),
            additional_entry_info: get_additional_entry_info(first_entry_ref, &first_entry_path),
            current_siblings,
            right_column,
            current_path: first_entry_path,
            parent_path,

            parent_siblings_shift,
            current_siblings_shift,
            cumulative_size_text: None, // for CumulativeSize

            input_mode: None,
        }
    }
}
