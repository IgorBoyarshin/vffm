use pancurses::*;

use crate::coloring::*;
use crate::filesystem::*;
use crate::input::*;

use std::path::PathBuf;

type Coord = i32;

pub struct Settings {
    pub columns_ratio: Vec<u32>,
    pub dir_paint: Paint,
    pub symlink_paint: Paint,
    pub file_paint: Paint,
    pub unknown_paint: Paint,

    pub cursor_vertical_gap: usize,
}

pub struct System {
    window: Window,

    height: Coord,
    width: Coord,

    primary_paint: Paint,
    columns_coord: Vec<(Coord, Coord)>,

    dir_paint: Paint,
    symlink_paint: Paint,
    file_paint: Paint,
    unknown_paint: Paint,

    current_siblings: Vec<Entry>,
    parent_siblings: Vec<Entry>,
    child_siblings: Vec<Entry>,

    current_path: Option<PathBuf>,
    parent_path: PathBuf,

    parent_index: usize,
    current_index: usize,

    current_permissions: String,

    cursor_vertical_gap: usize, // const
    max_entries_displayed: usize, // const
    entries_display_begin: i32, // const
    parent_siblings_shift: usize,
    current_siblings_shift: usize,

    sorting_type: SortingType,
}

impl System {
    pub fn new(settings: Settings, starting_path: PathBuf) -> Self {
        let window = System::setup();
        let (height, width) = System::get_height_width(&window);
        let primary_paint =
            Paint{fg: Color::White, bg: Color::Black, bold: false, underlined: false};

        let current_siblings = collect_maybe_dir(&starting_path);
        let parent_siblings = collect_siblings_of(&starting_path);
        let first_entry_path = System::path_of_first_entry_inside(&starting_path, &current_siblings);
        let parent_index = index_inside(&starting_path);
        let current_index = 0;
        let max_entries_displayed = height as usize - 4; // gap+border+border+gap

        let parent_siblings_shift = System::siblings_shift_for(
                settings.cursor_vertical_gap, max_entries_displayed,
                parent_index, parent_siblings.len(), None);
        let current_siblings_shift = System::siblings_shift_for(
                settings.cursor_vertical_gap, max_entries_displayed,
                current_index, current_siblings.len(), None);

        let sorting_type = SortingType::Any;

        System {
            window,
            height,
            width,

            primary_paint,
            dir_paint: settings.dir_paint,
            symlink_paint: settings.symlink_paint,
            file_paint: settings.file_paint,
            unknown_paint: settings.unknown_paint,

            columns_coord: System::positions_from_ratio(settings.columns_ratio, width),

            parent_index,
            current_index,
            current_siblings,
            parent_siblings,
            child_siblings: System::collect_sorted_children_of(&first_entry_path, &sorting_type),
            current_permissions: System::string_permissions_for_path(&first_entry_path),
            current_path: first_entry_path,
            parent_path: starting_path,

            cursor_vertical_gap: {
                let mut gap = settings.cursor_vertical_gap;
                while max_entries_displayed <= 2 * gap { // gap too large
                    gap -= 1;
                }
                gap
            },
            parent_siblings_shift,
            current_siblings_shift,
            max_entries_displayed,
            entries_display_begin: 2, // gap+border

            sorting_type,
        }
    }
//-----------------------------------------------------------------------------
    pub fn change_sorting_type(&mut self, new_sorting_type: SortingType) {
        self.sorting_type = new_sorting_type;
    }

    fn sort(mut entries: Vec<Entry>, sorting_type: &SortingType) -> Vec<Entry> {
        match sorting_type {
            SortingType::Lexicographically => entries.sort_by(|a, b| a.name.cmp(&b.name)),
            SortingType::TimeModified => entries.sort_by(|a, b| a.time_modified.cmp(&b.time_modified)),
            SortingType::Any => {},
        }
        entries
    }
//-----------------------------------------------------------------------------
    fn string_permissions_for_path(path: &Option<PathBuf>) -> String {
        if let Some(path) = path {
            permissions_of(path).string_representation()
        } else { "".to_string() }
    }

    fn get_current_permissions(&mut self) -> String {
        System::string_permissions_for_path(&self.current_path)
    }

    fn unsafe_current_entry_ref(&self) -> &Entry {
        &self.current_siblings[self.current_index]
    }

    fn current_is_dir(&self) -> bool {
        if self.inside_empty_dir() { return false; }
        self.unsafe_current_entry_ref().is_dir()
    }

    fn inside_empty_dir(&self) -> bool {
        self.current_path.is_none()
    }

    fn path_of_first_entry_inside(path: &PathBuf, entries: &Vec<Entry>) -> Option<PathBuf> {
        if entries.is_empty() { return None; }
        let name = entries[0].name.clone();
        let mut path = path.clone();
        path.push(name);
        Some(path)
    }
//-----------------------------------------------------------------------------
    fn update_last_part_of_current_path_by_index(&mut self) {
        let name = self.unsafe_current_entry_ref().name.clone();
        self.current_path.as_mut().map(|path| {
            (*path).pop();
            (*path).push(name);
        });
    }

    fn collect_sorted_siblings_of_parent(&self) -> Vec<Entry> {
        System::sort(collect_siblings_of(&self.parent_path), &self.sorting_type)
    }

    fn collect_sorted_children_of(path: &Option<PathBuf>, sorting_type: &SortingType) -> Vec<Entry> {
        if let Some(path) = path {
            System::sort(collect_maybe_dir(&path), sorting_type)
        } else { Vec::new() }
    }

    fn collect_sorted_children_of_current(&self) -> Vec<Entry> {
        System::collect_sorted_children_of(&self.current_path, &self.sorting_type)
    }

    fn collect_sorted_children_of_parent(&self) -> Vec<Entry> {
        System::sort(collect_maybe_dir(&self.parent_path), &self.sorting_type)
    }

    // The display is guaranteed to be able to contain 2*gap (accomplished in settings)
    fn siblings_shift_for(gap: usize, max: usize, index: usize,
                              len: usize, old_shift: Option<usize>) -> usize {
        let gap   = gap   as i32;
        let max   = max   as i32;
        let index = index as i32;
        let len   = len   as i32;

        if len <= max         { return 0; }
        if index < gap        { return 0; }
        if index >= len - gap { return (len - max) as usize; }

        if let Some(old_shift) = old_shift {
            let old_shift = old_shift as i32;

            let shift = index - gap;
            if shift < old_shift { return shift as usize; }
            let shift = index + 1 - max + gap;
            if shift > old_shift { return shift as usize; }

            old_shift as usize
        } else { // no requirements => let at the top of the screen after the gap
            let mut shift = index - gap;
            let left_at_bottom = len - shift - max;
            if left_at_bottom < 0 { shift += left_at_bottom; }
            shift as usize
        }
    }

    fn recalculate_parent_siblings_shift(&mut self) -> usize {
        System::siblings_shift_for(
            self.cursor_vertical_gap, self.max_entries_displayed,
            self.parent_index, self.parent_siblings.len(), None)
    }

    fn recalculate_current_siblings_shift(&mut self) -> usize {
        System::siblings_shift_for(
            self.cursor_vertical_gap, self.max_entries_displayed,
            self.current_index, self.current_siblings.len(), Some(self.current_siblings_shift))
    }

    fn common_up_down(&mut self) {
        self.update_last_part_of_current_path_by_index();
        self.current_permissions = self.get_current_permissions();
        self.child_siblings = self.collect_sorted_children_of_current();
        self.current_siblings_shift = self.recalculate_current_siblings_shift();
    }

    fn common_left_right(&mut self) {
        self.current_permissions = self.get_current_permissions();
        self.child_siblings = self.collect_sorted_children_of_current();
        self.current_siblings = self.collect_sorted_children_of_parent();
        self.parent_siblings = self.collect_sorted_siblings_of_parent();
    }
//-----------------------------------------------------------------------------
    pub fn up(&mut self) {
        if self.inside_empty_dir() { return }
        if self.current_index > 0 {
            self.current_index -= 1;
            self.common_up_down();
        }
    }

    pub fn down(&mut self) {
        if self.inside_empty_dir() { return }
        if self.current_index < self.current_siblings.len() - 1 {
            self.current_index += 1;
            self.common_up_down();
        }
    }

    pub fn left(&mut self) {
        if !is_root(&self.parent_path) {
            if self.current_path.is_none() {
                self.current_path = Some(self.parent_path.clone());
            } else {
                self.current_path.as_mut().map(|path| path.pop());
            }
            self.parent_path.pop();

            self.common_left_right();

            self.current_index = self.parent_index;
            self.parent_index = index_inside(&self.parent_path);
            self.current_siblings_shift = self.parent_siblings_shift;
            self.parent_siblings_shift = self.recalculate_parent_siblings_shift();
        }
    }

    pub fn right(&mut self) {
        if self.current_is_dir() {
            let current_path_ref = self.current_path.as_ref().unwrap();
            self.parent_path = current_path_ref.to_path_buf();
            self.current_path = System::path_of_first_entry_inside(
                current_path_ref, &self.child_siblings);

            self.common_left_right();

            self.parent_index = self.current_index;
            self.current_index = 0;
            self.parent_siblings_shift = self.current_siblings_shift;
            self.current_siblings_shift = 0;
        }
    }
//-----------------------------------------------------------------------------
    fn list_entry(&self, cs: &mut ColorSystem, column_index: usize,
            y: usize, entry: &Entry, selected: bool) {
        let paint = match entry.entrytype {
            EntryType::Regular => self.file_paint,
            EntryType::Directory => self.dir_paint,
            EntryType::Symlink => self.symlink_paint,
            EntryType::Unknown => self.unknown_paint,
        };
        let paint = System::maybe_selected_paint_from(paint, selected);
        cs.set_paint(&self.window, paint);

        let (begin, end) = self.columns_coord[column_index];
        let column_width = end - begin;
        let size = System::human_size(entry.size);
        let size_len = size.len();
        let name_len = entry.name.len() as i32;
        let empty_space_length = column_width - name_len - size_len as i32;
        let y = y as Coord + self.entries_display_begin;
        if empty_space_length < 1 {
            // everything doesn't fit => sacrifice Size and truncate the Name
            let text = System::truncate_string(&entry.name, column_width);
            let leftover = column_width - text.len() as i32;
            self.window.mvprintw(y, begin + 1, &text);
            self.window.mv(y, begin + 1 + text.len() as i32);
            self.window.hline(' ', leftover);
        } else { // everything fits OK
            self.window.mvprintw(y, begin + 1, &entry.name);
            self.window.mv(y, begin + 1 + name_len);
            self.window.hline(' ', empty_space_length);
            self.window.mvprintw(y, begin + 1 + name_len + empty_space_length, size);
        }
    }

    fn list_entries(&self, mut cs: &mut ColorSystem, column_index: usize,
            entries: &Vec<Entry>, selected_index: Option<usize>, shift: usize) {
        for (index, entry) in entries.into_iter().enumerate()
                .skip(shift).take(self.max_entries_displayed) {
            let selected = match selected_index {
                Some(i) => (i == index),
                None    => false,
            };
            self.list_entry(&mut cs, column_index, index - shift, &entry, selected);
        }
    }
//-----------------------------------------------------------------------------
    pub fn clear(&self, color_system: &mut ColorSystem) {
        color_system.set_paint(&self.window, self.primary_paint);
        for y in 0..self.height {
            self.window.mv(y, 0);
            self.window.hline(' ', self.width);
        }
        self.window.refresh();
    }

    pub fn draw(&self, mut cs: &mut ColorSystem) {
        self.draw_borders(&mut cs);

        // Previous
        let column_index = 0;
        self.list_entries(&mut cs, column_index, &self.parent_siblings,
                          Some(self.parent_index), self.parent_siblings_shift);

        // Current
        let column_index = 1;
        if self.inside_empty_dir() {
            self.draw_empty_sign(&mut cs, column_index);
        } else {
            self.list_entries(&mut cs, column_index, &self.current_siblings,
                          Some(self.current_index), self.current_siblings_shift);
        }

        // Next
        let column_index = 2;
        if !self.inside_empty_dir() {
            if self.current_is_dir() && self.child_siblings.is_empty() {
                self.draw_empty_sign(&mut cs, column_index);
            } else {
                self.list_entries(&mut cs, column_index, &self.child_siblings, None, 0);
            }
        }

        // Current path
        if !self.inside_empty_dir() {
            cs.set_paint(&self.window, Paint{fg: Color::LightBlue, bg: Color::Black,
                                                bold: false, underlined: false});
            self.window.mvprintw(0, 0,
                         self.current_path.as_ref().unwrap().to_str().unwrap());
        }

        // Current permissions
        if self.current_path.is_some() {
            cs.set_paint(&self.window, Paint{fg: Color::LightBlue, bg: Color::Black,
                                                bold: false, underlined: false});
            self.window.mvprintw(self.height - 1, 0, &self.current_permissions);
        }

        self.window.refresh();
    }

    pub fn draw_available_matches(&self, cs: &mut ColorSystem,
            matches: &Matches, completion_count: usize) {
        if matches.is_empty() { return; }
        cs.set_paint(&self.window, Paint{fg: Color::Green, bg: Color::Black,
                                            bold: false, underlined: false});
        let y = self.height - 2 - matches.len() as i32 - 1;
        self.window.mv(y, 0);
        self.window.hline(ACS_HLINE(), self.width);
        self.window.mv(self.height - 2, 0);
        self.window.hline(ACS_HLINE(), self.width);
        for (i, (combination, command)) in matches.iter().enumerate() {
            let y = y + 1 + i as i32;
            let max_len = max_combination_len() as i32;

            // Combination
            let (completed_part, uncompleted_part) = combination.split_at(completion_count);
            // Completed part
            cs.set_paint(&self.window, Paint{fg: Color::Green, bg: Color::Black,
                                                bold: true, underlined: false});
            self.window.mvprintw(y, 0, &completed_part);
            // Uncompleted part
            cs.set_paint(&self.window, Paint{fg: Color::Green, bg: Color::Black,
                                                bold: false, underlined: false});
            self.window.printw(&uncompleted_part);
            // Space till description
            let left = max_len - combination.len() as i32;
            self.window.hline(' ', left);

            // Command description
            cs.set_paint(&self.window, Paint{fg: Color::Green, bg: Color::Black,
                                                bold: false, underlined: false});
            let description = description_of(&command);
            self.window.mvprintw(y, max_len as i32, &description);
            // Space till end
            let left = self.width - max_len - description.len() as i32;
            self.window.hline(' ', left);
        }
    }

    fn draw_empty_sign(&self, cs: &mut ColorSystem, column_index: usize) {
        cs.set_paint(&self.window, Paint{fg: Color::Black, bg: Color::Red, bold: true, underlined: false});
        let (begin, _) = self.columns_coord[column_index];
        self.window.mvprintw(self.entries_display_begin, begin + 1, "empty");
    }

    fn draw_column(&self, color_system: &mut ColorSystem, x: Coord) {
        color_system.set_paint(&self.window, self.primary_paint);
        self.window.mv(1, x);
        self.window.addch(ACS_TTEE());
        self.window.mv(2, x);
        self.window.vline(ACS_VLINE(), self.height-4);
        self.window.mv(self.height-2, x);
        self.window.addch(ACS_BTEE());
    }

    fn draw_borders(&self, color_system: &mut ColorSystem) {
        color_system.set_paint(&self.window, self.primary_paint);

        self.window.mv(1, 0);
        self.window.addch(ACS_ULCORNER());
        self.window.hline(ACS_HLINE(), self.width-2);
        self.window.mv(1, self.width-1);
        self.window.addch(ACS_URCORNER());

        self.window.mv(self.height-2, 0);
        self.window.addch(ACS_LLCORNER());
        self.window.hline(ACS_HLINE(), self.width-2);
        self.window.mv(self.height-2, self.width-1);
        self.window.addch(ACS_LRCORNER());

        for y in 2..self.height-2 {
            self.window.mv(y, 0);
            self.window.addch(ACS_VLINE());
            self.window.mv(y, self.width-1);
            self.window.addch(ACS_VLINE());
        }

        // For columns
        for (start, _end) in self.columns_coord.iter().skip(1) {
            self.draw_column(color_system, *start);
        }
    }
//-----------------------------------------------------------------------------
    fn truncate_string(string: &str, max_length: i32) -> String {
        if string.len() > max_length as usize {
            let delimiter = "...";
            let leave_at_end = 5;
            let total_end_len = leave_at_end + delimiter.len();
            let start = max_length as usize - total_end_len;
            let mut new = string.clone().to_string();
            new.replace_range(start..(string.len() - leave_at_end), delimiter);
            new
        } else {
            string.clone().to_string()
        }
    }

    fn maybe_selected_paint_from(paint: Paint, convert: bool) -> Paint {
        if convert {
            let Paint {fg, bg, bold: _, underlined} = paint;
            Paint {fg: bg, bg: fg, bold: true, underlined}
        } else { paint }
    }

    fn positions_from_ratio(ratio: Vec<u32>, width: Coord) -> Vec<(Coord, Coord)> {
        let width = width as f32;
        let sum = ratio.iter().sum::<u32>() as f32;
        let mut pos: Coord = 0;
        let mut positions: Vec<(Coord, Coord)> = Vec::new();
        let last_index = ratio.len() - 1;
        for (index, r) in ratio.into_iter().enumerate() {
            let weight = ((r as f32 / sum) * width) as Coord;
            let end = if index == last_index {
                width as i32 - 2
            } else {
                pos + weight
            };
            positions.push((pos, end));
            pos += weight + 1;
        }
        positions
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
//-----------------------------------------------------------------------------
    fn setup() -> Window {
        let window = initscr();
        window.refresh();
        window.keypad(true);
        start_color();
        noecho();

        window
    }

    fn get_height_width(window: &Window) -> (i32, i32) {
        window.get_max_yx()
    }

    pub fn get(&self) -> Option<Input> {
        self.window.getch()
    }
}

impl Drop for System {
    fn drop(&mut self) {
        endwin();
        println!("Done");
    }
}
