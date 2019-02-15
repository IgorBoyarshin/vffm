use crate::direntry::*;
use crate::filesystem::*;
use crate::right_column::*;
use crate::input_mode::*;
use crate::input::*;
use crate::tab::*;
use crate::notification::*;
use crate::coloring::*;
use crate::utils::*;

use std::path::PathBuf;
use pancurses::{Window,
    ACS_CKBOARD, ACS_VLINE, ACS_HLINE, ACS_TTEE, ACS_BTEE,
    ACS_LLCORNER, ACS_LRCORNER, ACS_ULCORNER, ACS_URCORNER};


pub type Coord = i32;
//-----------------------------------------------------------------------------
pub struct Bar {
    y: Coord,
    ready_left: Coord, // x Coord of the first not-taken cell from the left
    ready_right: Coord, // x Coord of the first taken cell after all not-taken from the left
}

impl Bar {
    pub fn with_y_and_width(y: Coord, width: Coord) -> Bar {
        Bar {
            y,
            ready_left: 0,
            ready_right: width,
        }
    }

    pub fn draw_left(&mut self, window: &Window, text: &str, padding: Coord) {
        let len = text.len();
        let free = self.free_space();
        if len > free {
            let mut copy = text.to_string().clone();
            copy.truncate(free);
            mvprintw(window, self.y, self.ready_left, &copy);
            self.ready_left += free as Coord + padding;
        } else {
            mvprintw(window, self.y, self.ready_left, &text);
            self.ready_left += len as Coord + padding;
        }
    }

    pub fn draw_right(&mut self, window: &Window, text: &str, padding: Coord) {
        let len = text.len();
        let free = self.free_space();
        if len > free {
            let mut copy = text.to_string().clone();
            copy.truncate(free);
            mvprintw(window, self.y, self.ready_right - free as Coord, &copy);
            self.ready_right -= free as Coord + padding;
        } else {
            mvprintw(window, self.y, self.ready_right - len as Coord, &text);
            self.ready_right -= len as Coord + padding;
        }
    }

    fn free_space(&self) -> usize {
        (self.ready_right - self.ready_left + 1) as usize
    }
}
//-----------------------------------------------------------------------------
pub struct DisplaySettings {
    pub height: Coord,
    pub width:  Coord,
    pub columns_coord: Vec<(Coord, Coord)>,
    pub scrolling_gap: usize,
    pub column_effective_height: usize,
    entries_display_begin: Coord,
}

impl DisplaySettings {
    pub fn generate(window: &Window, scrolling_gap: usize, columns_ratio: &Vec<u32>)
            -> DisplaySettings {
        let (height, width) = DisplaySettings::get_height_width(window);
        let column_effective_height = height as usize - 4; // gap+border+border+gap
        let scrolling_gap = DisplaySettings::resize_scrolling_gap_until_fits(
            scrolling_gap, column_effective_height);
        let columns_coord = DisplaySettings::positions_from_ratio(columns_ratio, width);

        DisplaySettings {
            height,
            width,
            columns_coord,
            scrolling_gap,
            column_effective_height,
            entries_display_begin: 2, // gap + border
        }
    }

    fn get_height_width(window: &Window) -> (Coord, Coord) {
        window.get_max_yx()
    }

    fn resize_scrolling_gap_until_fits(mut gap: usize, column_effective_height: usize) -> usize {
        while 2 * gap >= column_effective_height { gap -= 1; } // gap too large
        gap
    }

    fn positions_from_ratio(ratio: &Vec<u32>, width: Coord) -> Vec<(Coord, Coord)> {
        let width = width as f32;
        let sum = ratio.iter().sum::<u32>() as f32;
        let mut pos: Coord = 0;
        let mut positions: Vec<(Coord, Coord)> = Vec::new();
        let last_index = ratio.len() - 1;
        for (index, r) in ratio.iter().enumerate() {
            let weight = ((*r as f32 / sum) * width) as Coord;
            let end = if index == last_index {
                width as Coord - 2
            } else {
                pos + weight
            };
            positions.push((pos, end));
            pos += weight + 1;
        }
        positions
    }
}
//-----------------------------------------------------------------------------
fn printw(window: &Window, string: &str) -> Coord {
    // To avoid printw's substitution
    let string = string.to_string().replace("%", "%%");
    window.printw(string)
}

fn mvprintw(window: &Window, y: Coord, x: Coord, string: &str) -> Coord {
    window.mv(y, x);
    printw(window, string)
}
//-----------------------------------------------------------------------------
pub struct Renderer {
    pub window: Window,
    pub display_settings: DisplaySettings,
}

impl Renderer {
    pub fn new(window: Window, display_settings: DisplaySettings) -> Renderer {
        Renderer {
            window,
            display_settings,
        }
    }

    pub fn invalidate(&self) {
        self.window.clear();
    }

    pub fn refresh(&self) {
        self.window.refresh();
    }

    pub fn getch(&self) -> Option<pancurses::Input> {
        self.window.getch()
    }

    pub fn clear(&self, cs: &mut ColorSystem, clear_paint: Paint) {
        cs.set_paint(&self.window, clear_paint);
        for y in 0..self.display_settings.height {
            self.window.mv(y, 0);
            self.window.hline(' ', self.display_settings.width);
        }
    }

    pub fn draw_borders(&self, color_system: &mut ColorSystem, paint: Paint) {
        color_system.set_paint(&self.window, paint);
        let (width, height) = (self.display_settings.width, self.display_settings.height);

        self.window.mv(1, 0);
        self.window.addch(ACS_ULCORNER());
        self.window.hline(ACS_HLINE(), width - 2);
        self.window.mv(1, width - 1);
        self.window.addch(ACS_URCORNER());

        self.window.mv(height - 2, 0);
        self.window.addch(ACS_LLCORNER());
        self.window.hline(ACS_HLINE(), width - 2);
        self.window.mv(height - 2, width-1);
        self.window.addch(ACS_LRCORNER());

        for y in 2..height-2 {
            self.window.mv(y, 0);
            self.window.addch(ACS_VLINE());
            self.window.mv(y, width - 1);
            self.window.addch(ACS_VLINE());
        }

        for (start, _end) in self.display_settings.columns_coord.iter().skip(1) {
            self.draw_column_border(color_system, paint, *start);
        }
    }

    pub fn draw_left_column(&self, mut cs: &mut ColorSystem,
            siblings: &Vec<DirEntry>, index: usize, shift: usize) {
        const COLUMN_INDEX: usize = 0;
        self.list_entries(&mut cs, COLUMN_INDEX, siblings, Some(index), shift);
    }

    pub fn draw_middle_column(&self, mut cs: &mut ColorSystem, inside_empty_dir: bool,
                              siblings: &Vec<DirEntry>, index: usize, shift: usize) {
        const COLUMN_INDEX: usize = 1;
        if inside_empty_dir {
            self.draw_empty_sign(&mut cs, COLUMN_INDEX);
        } else {
            self.list_entries(&mut cs, COLUMN_INDEX, siblings, Some(index), shift);
        }
    }

    pub fn draw_right_column(&self, mut cs: &mut ColorSystem, right_column: &RightColumn, preview_paint: Paint) {
        const COLUMN_INDEX: usize = 2;
        if let Some(siblings) = right_column.siblings_ref() {
            // Have siblings (Some or None) => are sure to be inside a dir or symlink
            if siblings.is_empty() {
                self.draw_empty_sign(&mut cs, COLUMN_INDEX);
            } else {
                self.list_entries(&mut cs, COLUMN_INDEX, siblings, None, 0);
            }
        } else if let Some(preview) = right_column.preview_ref() {
            let (begin, _) = self.display_settings.columns_coord[COLUMN_INDEX];
            let y = self.display_settings.entries_display_begin;
            cs.set_paint(&self.window, preview_paint);
            for (i, line) in preview.iter().enumerate() {
                mvprintw(&self.window, y + i as Coord, begin + 1, line);
            }
        }
    }

    pub fn draw_current_path(&self, cs: &mut ColorSystem, bar: &mut Bar,
            inside_empty_dir: bool, parent_path: &PathBuf, current_path: &Option<PathBuf>) {
        cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
        if inside_empty_dir {
            let text = path_to_string(parent_path) + "/<?>";
            bar.draw_left(&self.window, &text, 2);
        } else {
            let path = current_path.as_ref().unwrap().to_str().unwrap();
            bar.draw_left(&self.window, path, 2);
        }
    }

    pub fn maybe_draw_input_mode(&self, cs: &mut ColorSystem, bar: &mut Bar, input_mode: &Option<InputMode>) {
        match input_mode.as_ref() {
            Some(InputMode::Search(SearchTools {query, ..})) => {
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default).bold());
                bar.draw_left(&self.window, "/", 0);
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Purple, Color::Default));
                bar.draw_left(&self.window, query, 2);
            },
            Some(InputMode::ChangeName(ChangeNameTools {new_name, ..})) => {
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default).bold());
                bar.draw_left(&self.window, "change to:", 0);
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Purple, Color::Default));
                bar.draw_left(&self.window, new_name, 2);
            },
            Some(InputMode::Command(CommandTools {text, ..})) => {
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default).bold());
                bar.draw_left(&self.window, ":> ", 0);
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Purple, Color::Default));
                bar.draw_left(&self.window, text, 2);
            },
            _ => {},
        }
    }

    pub fn maybe_draw_input_mode_cursor(&self, input_mode: &Option<InputMode>) {
        match input_mode.as_ref() {
            Some(InputMode::Search(SearchTools {cursor_index, ..})) => {
                if let Some(index) = cursor_index {
                    const PREFIX_LEN: i32 = "/".len() as i32;
                    self.window.mv(self.display_settings.height - 1, PREFIX_LEN + *index as Coord);
                }
            },
            Some(InputMode::ChangeName(ChangeNameTools {cursor_index, ..})) => {
                const PREFIX_LEN: i32 = "change to:".len() as i32;
                self.window.mv(self.display_settings.height - 1, PREFIX_LEN + *cursor_index as Coord);
            }
            Some(InputMode::Command(CommandTools {cursor_index, ..})) => {
                const PREFIX_LEN: i32 = ":> ".len() as i32;
                self.window.mv(self.display_settings.height - 1, PREFIX_LEN + *cursor_index as Coord);
            }
            _ => {},
        }
    }

    pub fn draw_current_permission(&self, cs: &mut ColorSystem,
            bar: &mut Bar, current_permissions: &Option<String>) {
        if let Some(permissions) = current_permissions {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            bar.draw_left(&self.window, permissions, 2);
        }
    }

    pub fn draw_current_size(&self, cs: &mut ColorSystem, bar: &mut Bar, size: Option<u64>) {
        if let Some(size) = size {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Blue, Color::Default));
            bar.draw_left(&self.window, &human_size(size), 2);
        }
    }

    pub fn maybe_draw_additional_info_for_current(&self, cs: &mut ColorSystem,
            bar: &mut Bar, additional_entry_info: &Option<String>) {
        if let Some(info) = additional_entry_info {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            bar.draw_left(&self.window, &info, 2);
        }
    }

    pub fn draw_current_dir_siblings_count(&self, cs: &mut ColorSystem,
            bar: &mut Bar, current_siblings: &Vec<DirEntry>) {
        let count = current_siblings.len().to_string();
        let text = "Siblings = ".to_string() + &count;
        cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
        bar.draw_left(&self.window, &text, 2);
    }

    pub fn draw_cumulative_size_text(&self, cs: &mut ColorSystem,
            bar: &mut Bar, cumulative_size_text: &Option<String>) {
        if let Some(text) = cumulative_size_text.as_ref() {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default));
            bar.draw_left(&self.window, text, 2);
        }
    }

    // returns whether to assign None to notification
    pub fn draw_notification(&mut self, cs: &mut ColorSystem,
            bar: &mut Bar, notification: &Option<Notification>) {
        if let Some(notification) = notification.as_ref() {
            if !notification.has_finished() {
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default));
                bar.draw_right(&self.window, &notification.text, 2);
            }
        }
    }

    pub fn maybe_draw_selection_warning(&self, cs: &mut ColorSystem, bar: &mut Bar, selection_empty: bool) {
        if !selection_empty {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Red, Color::Default).bold());
            bar.draw_left(&self.window, "Selection not empty", 2);
        }
    }

    pub fn draw_tabs(&self, cs: &mut ColorSystem, bar: &mut Bar, tabs: &Vec<Tab>, current_index: usize) {
        if tabs.len() == 1 { return; }
        for (index, tab) in tabs.iter().enumerate().rev() {
            if index == current_index {
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default).bold());
            } else {
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            }
            let text = "<".to_string() + &index.to_string() + ":" + &tab.name + &">".to_string();
            bar.draw_right(&self.window, &text, 0);
        }
    }

    pub fn draw_available_matches(&self, cs: &mut ColorSystem,
            matches: &Vec<Match>, completion_count: usize) {
        if matches.is_empty() { return; }

        // Borders
        cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default));
        let y = self.display_settings.height - 2 - matches.len() as Coord - 1;
        self.window.mv(y, 0);
        self.window.hline(ACS_HLINE(), self.display_settings.width);
        self.window.mv(self.display_settings.height - 2, 0);
        self.window.hline(ACS_HLINE(), self.display_settings.width);

        let max_len = max_combination_len() as Coord;
        for (i, (combination, command)) in matches.iter().enumerate() {
            if let Combination::Str(combination) = combination {
                let y = y + 1 + i as Coord;

                // Combination
                let (completed_part, uncompleted_part) = combination.split_at(completion_count);
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default).bold());
                mvprintw(&self.window, y, 0, &completed_part);
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default));
                printw(  &self.window,       &uncompleted_part);

                // Space till description
                let left = max_len - combination.len() as Coord;
                self.window.hline(' ', left);

                // Command description
                let description = description_of(&command);
                mvprintw(&self.window, y, max_len as Coord, &description);

                // Space till end
                let left = self.display_settings.width - max_len - description.len() as Coord;
                self.window.hline(' ', left);
            }
        }
    }

    fn draw_empty_sign(&self, cs: &mut ColorSystem, column_index: usize) {
        let (begin, _) = self.display_settings.columns_coord[column_index];
        const EMPTY_TEXT: &str = "empty";
        cs.set_paint(&self.window, Paint::with_fg_bg(Color::Black, Color::Red).bold());
        mvprintw(&self.window, self.display_settings.entries_display_begin, begin + 1, EMPTY_TEXT);
    }

    fn draw_column_border(&self, color_system: &mut ColorSystem, paint: Paint, x: Coord) {
        color_system.set_paint(&self.window, paint);
        self.window.mv(1, x);
        self.window.addch(ACS_TTEE());
        self.window.mv(2, x);
        self.window.vline(ACS_VLINE(), self.display_settings.height-4);
        self.window.mv(self.display_settings.height-2, x);
        self.window.addch(ACS_BTEE());
    }

    pub fn list_entry(&self, cs: &mut ColorSystem, column_index: usize,
            y: usize, entry: &DirEntry, under_cursor: bool, selected: bool) {
        let paint = maybe_selected_paint_from(entry.paint, under_cursor);

        let y = y as Coord + self.display_settings.entries_display_begin;
        let (mut begin, end) = self.display_settings.columns_coord[column_index];
        if selected {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Red, Color::Default));
            self.window.mvaddch(y, begin + 1, ACS_CKBOARD());
            self.window.mvaddch(y, begin + 2, ' ');
            begin += 2;
        }
        let column_width = end - begin;
        let size = human_size(entry.size);
        let size_len = size.len();
        let name_len = chars_amount(&entry.name) as Coord;
        let empty_space_length = column_width - name_len - size_len as Coord;
        cs.set_paint(&self.window, paint);
        if empty_space_length < 1 {
            // everything doesn't fit => sacrifice Size and truncate the Name
            let name = truncate_with_delimiter(&entry.name, column_width);
            let name_len = chars_amount(&name) as Coord;
            let leftover = column_width - name_len;
            mvprintw(&self.window, y, begin + 1, &name);
            self.window.mv(y, begin + 1 + name_len);
            self.window.hline(' ', leftover);
        } else { // everything fits OK
            mvprintw(&self.window, y, begin + 1, &entry.name);
            self.window.mv(y, begin + 1 + name_len);
            self.window.hline(' ', empty_space_length);
            mvprintw(&self.window, y, begin + 1 + name_len + empty_space_length, &size);
        }
    }

    fn list_entries(&self, mut cs: &mut ColorSystem, column_index: usize,
            entries: &Vec<DirEntry>, cursor_index: Option<usize>, shift: usize) {
        for (index, entry) in entries.into_iter().enumerate()
                .skip(shift).take(self.display_settings.column_effective_height) {
            let under_cursor = match cursor_index {
                Some(i) => (i == index),
                None    => false,
            };
            self.list_entry(&mut cs, column_index, index - shift,
                            &entry, under_cursor, entry.is_selected);
        }
    }
}
