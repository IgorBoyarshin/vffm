use pancurses::*;
use crate::coloring::*;
use crate::filesystem::*;
use crate::input::*;

use std::path::PathBuf;

use std::process::{Command, Stdio};

use std::collections::HashMap;

use std::ops::RangeBounds;


type Coord = i32;
//-----------------------------------------------------------------------------
fn printw(window: &Window, string: &str) -> i32 {
    // To avoid printw's substitution
    let string = string.to_string().replace("%", "%%");
    window.printw(string)
}

fn mvprintw(window: &Window, y: i32, x: i32, string: &str) -> i32 {
    window.mv(y, x);
    printw(window, string)
}
//-----------------------------------------------------------------------------
#[derive(Clone)]
struct SpawnRule {
    rule: String,
    is_external: bool,
}

impl SpawnRule {
    fn generate(&self, file_name: &str) -> (String, Vec<String>, bool) {
        let placeholder = "@";
        let mut args = Vec::new();
        let mut parts = self.rule.split_whitespace();
        let app = parts.next().unwrap();
        for arg in parts { // the rest
            if arg == placeholder { args.push(file_name.to_string()); }
            else                  { args.push(arg.to_string()); }
        }
        (app.to_string(), args, self.is_external)
    }
}

enum SpawnFile {
    Extension(String),
    ExactName(String),
}

struct SpawnPattern {
    file: SpawnFile,
    rule: SpawnRule,
}

impl SpawnPattern {
    fn new_ext(ext: &str, rule: &str, is_external: bool) -> SpawnPattern {
        SpawnPattern {
            file: SpawnFile::Extension(ext.to_string()),
            rule: SpawnRule{ rule: rule.to_string(), is_external },
        }
    }

    fn new_exact(name: &str, rule: &str, is_external: bool) -> SpawnPattern {
        SpawnPattern {
            file: SpawnFile::ExactName(name.to_string()),
            rule: SpawnRule{ rule: rule.to_string(), is_external },
        }
    }
}
//-----------------------------------------------------------------------------
struct RightColumn {
    siblings: Option<Vec<Entry>>,
    preview: Option<Vec<String>>,
}

impl RightColumn {
    fn with_siblings(siblings: Vec<Entry>) -> RightColumn {
        RightColumn {
            siblings: Some(siblings),
            preview: None,
        }
    }

    fn with_preview(preview: Vec<String>) -> RightColumn {
        RightColumn {
            siblings: None,
            preview: Some(preview),
        }
    }

    fn empty() -> RightColumn {
        RightColumn {
            siblings: None,
            preview: None,
        }
    }

    fn siblings_ref(&self) -> Option<&Vec<Entry>> {
        self.siblings.as_ref()
    }

    fn preview_ref(&self) -> Option<&Vec<String>> {
        self.preview.as_ref()
    }

    // fn holds_siblings(&self) -> bool {
    //     self.siblings.is_some()
    // }
    //
    // fn holds_preview(&self) -> bool {
    //     self.preview.is_some()
    // }
}
//-----------------------------------------------------------------------------
pub struct Settings {
    pub primary_paint: Paint,
    pub dir_paint: Paint,
    pub symlink_paint: Paint,
    pub file_paint: Paint,
    pub unknown_paint: Paint,
    pub preview_paint: Paint,

    pub columns_ratio: Vec<u32>,
    pub scrolling_gap: usize,
}

struct DisplaySettings {
    height: Coord,
    width:  Coord,
    columns_coord: Vec<(Coord, Coord)>,

    scrolling_gap: usize, // const
    column_effective_height: usize, // const
    entries_display_begin: i32, // const
}

pub struct System {
    window: Window,
    settings: Settings,
    display_settings: DisplaySettings,

    current_siblings: Vec<Entry>,
    parent_siblings: Vec<Entry>,
    right_column: RightColumn, // depends on display_settings

    current_path: Option<PathBuf>,
    parent_path: PathBuf,

    parent_index: usize,
    current_index: usize,

    parent_siblings_shift: usize, // depends on display_settings
    current_siblings_shift: usize, // depends on display_settings

    current_permissions: String,
    sorting_type: SortingType,
    spawn_patterns: Vec<SpawnPattern>, // const
}

impl System {
    pub fn new(settings: Settings, starting_path: PathBuf) -> Self {
        let window = System::setup();

        let sorting_type = SortingType::Any;

        let current_siblings = collect_maybe_dir(&starting_path);
        let parent_siblings = collect_siblings_of(&starting_path);
        let first_entry_path =
            System::path_of_first_entry_inside(&starting_path, &current_siblings);
        let parent_index = index_inside(&starting_path);
        let current_index = 0;

        let display_settings = System::generate_display_settings(
            &window, settings.scrolling_gap, &settings.columns_ratio);

        let right_column = System::collect_right_column(&first_entry_path,
                        &sorting_type, display_settings.column_effective_height);

        let parent_siblings_shift = System::siblings_shift_for(
                display_settings.scrolling_gap, display_settings.column_effective_height,
                parent_index, parent_siblings.len(), None);
        let current_siblings_shift = System::siblings_shift_for(
                display_settings.scrolling_gap, display_settings.column_effective_height,
                current_index, current_siblings.len(), None);

        System {
            window,
            settings,
            display_settings,

            parent_index,
            current_index,
            current_siblings,
            parent_siblings,
            right_column,
            current_permissions: System::string_permissions_for_path(&first_entry_path),
            current_path: first_entry_path,
            parent_path: starting_path,

            parent_siblings_shift,
            current_siblings_shift,

            sorting_type,

            spawn_patterns: System::generate_spawn_patterns(),
        }
    }
//-----------------------------------------------------------------------------
    fn generate_display_settings(
            window: &Window, scrolling_gap: usize, columns_ratio: &Vec<u32>)
            -> DisplaySettings {
        let (height, width) = System::get_height_width(window);
        let column_effective_height = height as usize - 4; // gap+border+border+gap
        let scrolling_gap = System::resize_scrolling_gap_until_fits(
            scrolling_gap, column_effective_height);
        let columns_coord = System::positions_from_ratio(columns_ratio, width);
        DisplaySettings {
            height,
            width,
            columns_coord,
            scrolling_gap,
            column_effective_height,
            entries_display_begin: 2, // gap + border
        }
    }

    fn resize_scrolling_gap_until_fits(mut gap: usize, column_effective_height: usize) -> usize {
        while 2 * gap >= column_effective_height { gap -= 1; } // gap too large
        gap
    }

    fn collect_right_column_of_current(&self) -> RightColumn {
        System::collect_right_column(&self.current_path, &self.sorting_type,
                                     self.display_settings.column_effective_height)
    }

    fn collect_right_column(path_opt: &Option<PathBuf>, sorting_type: &SortingType,
            max_height: usize) -> RightColumn {
        if let Some(path) = path_opt {
            if System::is_dir_or_symlink(path) {
                return RightColumn::with_siblings(
                    System::collect_sorted_children_of(path_opt, sorting_type));
            } else { // not a dir
                if let Some(preview) = System::read_preview_of(path, max_height) {
                    return RightColumn::with_preview(preview);
                }
            }
        }
        RightColumn::empty()
    }

    fn read_preview_of(path: &PathBuf, max_height: usize) -> Option<Vec<String>> {
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if System::is_previewable(file_name) {
            let max_per_line = 100;
            Some(read_lines(path, max_height, max_height as u64 * max_per_line))
        } else { None }
    }

    fn is_previewable(file_name: &str) -> bool {
        for exact_name in System::text_exact_names() {
            if file_name == exact_name { return true; }
        }
        for ext in System::text_extensions() {
            if file_name.ends_with(ext) { return true; }
        }
        false
    }

    fn text_extensions() -> Vec<&'static str> {
        vec!["txt", "cpp", "h", "rs", "lock", "toml", "zsh", "java", "py",
            "sh", "md", "log", "yml", "tex", "nb", "js", "ts", "html", "css", "json"]
    }

    fn text_exact_names() -> Vec<&'static str> {
        vec!["Makefile", ".gitignore"]
    }

    fn generate_spawn_patterns() -> Vec<SpawnPattern> {
        let add_to_apps = |apps: &mut HashMap<String, (Vec<String>, bool)>,
                app: &str, spawn_files: Vec<&'static str>, is_external: bool| {
            let mut vec = Vec::new();
            for spawn_file in spawn_files {
                vec.push(spawn_file.to_string());
            }
            apps.insert(app.to_string(), (vec, is_external));
        };

        let mut apps_extensions  = HashMap::new();
        let mut apps_exact_names = HashMap::new();
        let external = true; // for convenience and readability
        let not_external = !external;

        add_to_apps(&mut apps_extensions, "vim @", System::text_extensions(), not_external);
        add_to_apps(&mut apps_exact_names, "vim @", System::text_exact_names(), not_external);
        add_to_apps(&mut apps_extensions, "vlc @", vec!["mkv", "avi", "mp4", "mp3"], external);

        let mut patterns: Vec<SpawnPattern> = Vec::new();
        for (app, (extensions, is_external)) in apps_extensions.into_iter() {
            for ext in extensions {
                patterns.push(SpawnPattern::new_ext(&ext, &app, is_external));
            }
        }
        for (app, (names, is_external)) in apps_exact_names.into_iter() {
            for name in names {
                patterns.push(SpawnPattern::new_exact(&name, &app, is_external));
            }
        }

        patterns
    }

    fn spawn_rule_for(&self, file_name: &str, full_path: &str) -> Option<(String, Vec<String>, bool)> {
        for SpawnPattern { file, rule } in self.spawn_patterns.iter() {
            match file {
                SpawnFile::Extension(ext) => if file_name.to_ascii_lowercase()
                                                    .ends_with(ext.as_str()) {
                    return Some(rule.generate(full_path));
                },
                SpawnFile::ExactName(name) => if file_name == name {
                    return Some(rule.generate(full_path));
                }
            }
        }
        None
    }

    fn spawn(app: &str, args: Vec<String>, external: bool) {
        if external {
            Command::new(app).args(args)
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .spawn().expect("failed to execute process");
        } else {
            Command::new(app).args(args)
                .status().expect("failed to execute process");
        }
    }
//-----------------------------------------------------------------------------
    pub fn sort_with(&mut self, new_sorting_type: SortingType) {
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

    fn current_entry_ref(&self) -> Option<&Entry> {
        if self.current_path.is_some() { Some(self.unsafe_current_entry_ref()) }
        else { None }
    }

    fn unsafe_current_entry_ref(&self) -> &Entry {
        &self.current_siblings[self.current_index]
    }

    // fn current_is_dir(&self) -> bool {
    //     if self.inside_empty_dir() { return false; }
    //     self.unsafe_current_entry_ref().is_dir()
    // }

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

    // fn current_is_dir_or_symlink(&self) -> bool {
    //     if self.current_path.is_some() {
    //         System::is_dir_or_symlink(self.current_path.as_ref().unwrap())
    //     } else { false }
    // }

    fn is_dir_or_symlink(path: &PathBuf) -> bool {
        !path.is_file()
    }
//-----------------------------------------------------------------------------
    fn update_last_part_of_current_path_by_index(&mut self) {
        let name = self.unsafe_current_entry_ref().name.clone();
        self.current_path.as_mut().map(|path| {
            (*path).set_file_name(name); // try_pop and then push(name)
        });
    }

    fn collect_sorted_siblings_of_parent(&self) -> Vec<Entry> {
        System::sort(collect_siblings_of(&self.parent_path), &self.sorting_type)
    }

    // TODO: mb get rid of Option
    fn collect_sorted_children_of(path: &Option<PathBuf>, sorting_type: &SortingType) -> Vec<Entry> {
        if let Some(path) = path {
            System::sort(collect_maybe_dir(&path), sorting_type)
        } else { Vec::new() }
    }

    // fn collect_sorted_children_of_current(&self) -> Vec<Entry> {
    //     System::collect_sorted_children_of(&self.current_path, &self.sorting_type)
    // }

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
            self.display_settings.scrolling_gap, self.display_settings.column_effective_height,
            self.parent_index, self.parent_siblings.len(), None)
    }

    fn recalculate_current_siblings_shift(&mut self) -> usize {
        System::siblings_shift_for(
            self.display_settings.scrolling_gap, self.display_settings.column_effective_height,
            self.current_index, self.current_siblings.len(), Some(self.current_siblings_shift))
    }

    fn common_up_down(&mut self) {
        self.update_last_part_of_current_path_by_index();
        self.current_permissions = self.get_current_permissions();
        self.right_column = self.collect_right_column_of_current();
        self.current_siblings_shift = self.recalculate_current_siblings_shift();
    }

    fn common_left_right(&mut self) {
        self.current_permissions = self.get_current_permissions();
        self.right_column = self.collect_right_column_of_current();
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
        if let Some(current_entry) = self.current_entry_ref() {
            let current_path_ref = self.current_path.as_ref().unwrap();
            if current_entry.is_dir() || current_entry.is_symlink() {
                // Navigate inside
                self.parent_path = current_path_ref.to_path_buf();
                self.current_path = System::path_of_first_entry_inside(
                    current_path_ref, self.right_column.siblings_ref().unwrap());

                self.common_left_right();

                self.parent_index = self.current_index;
                self.current_index = 0;
                self.parent_siblings_shift = self.current_siblings_shift;
                self.current_siblings_shift = 0;
            } else if current_entry.is_regular() {
                // Try to open with default app
                let file_name = current_entry.name.as_str();
                let full_path = current_path_ref.to_str().unwrap();
                if let Some((app, args, is_external)) = self.spawn_rule_for(file_name, full_path) {
                    System::spawn(&app, args, is_external);
                }
            }
        }
    }
//-----------------------------------------------------------------------------
    fn draw_current_size(&self, cs: &mut ColorSystem) {
        if self.current_path.is_some() {
            let size = System::human_size(self.unsafe_current_entry_ref().size);
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Blue, Color::Default));
            mvprintw(&self.window, self.display_settings.height - 1, 12, &size);
        }
    }

    fn draw_current_path(&self, cs: &mut ColorSystem) {
        if !self.inside_empty_dir() {
            let path = self.current_path.as_ref().unwrap().to_str().unwrap();
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            mvprintw(&self.window, 0, 0, path);
        }
    }

    fn draw_current_permission(&self, cs: &mut ColorSystem) {
        if self.current_path.is_some() {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            mvprintw(&self.window, self.display_settings.height - 1, 0, &self.current_permissions);
        }
    }

    fn draw_left_column(&self, mut cs: &mut ColorSystem) {
        let column_index = 0;
        self.list_entries(&mut cs, column_index, &self.parent_siblings,
              Some(self.parent_index), self.parent_siblings_shift);
    }

    fn draw_middle_column(&self, mut cs: &mut ColorSystem) {
        let column_index = 1;
        if self.inside_empty_dir() {
            self.draw_empty_sign(&mut cs, column_index);
        } else {
            self.list_entries(&mut cs, column_index, &self.current_siblings,
              Some(self.current_index), self.current_siblings_shift);
        }
    }

    fn draw_right_column(&self, mut cs: &mut ColorSystem) {
        let column_index = 2;
        if let Some(siblings) = self.right_column.siblings_ref() {
            // Have siblings (Some or None) => are sure to be in a dir or symlink
            if siblings.is_empty() {
                self.draw_empty_sign(&mut cs, column_index);
            } else {
                self.list_entries(&mut cs, column_index, siblings, None, 0);
            }
        } else if let Some(preview) = self.right_column.preview_ref() {
            let (begin, end) = self.display_settings.columns_coord[column_index];
            let column_width = end - begin;
            let y = self.display_settings.entries_display_begin;
            cs.set_paint(&self.window, self.settings.preview_paint);
            for (i, line) in preview.iter().enumerate() {
                let line = System::maybe_truncate(line.trim_end(), column_width as usize);
                mvprintw(&self.window, y + i as i32, begin + 1, &line);
            }
        } // display nothing otherwise
    }


    fn list_entry(&self, cs: &mut ColorSystem, column_index: usize,
            y: usize, entry: &Entry, selected: bool) {
        let paint = match entry.entrytype {
            EntryType::Regular => self.settings.file_paint,
            EntryType::Directory => self.settings.dir_paint,
            EntryType::Symlink => self.settings.symlink_paint,
            EntryType::Unknown => self.settings.unknown_paint,
        };
        let paint = System::maybe_selected_paint_from(paint, selected);
        cs.set_paint(&self.window, paint);

        let (begin, end) = self.display_settings.columns_coord[column_index];
        let column_width = end - begin;
        let size = System::human_size(entry.size);
        let size_len = size.len();
        let name_len = System::chars_amount(&entry.name) as i32;
        let empty_space_length = column_width - name_len - size_len as i32;
        let y = y as Coord + self.display_settings.entries_display_begin;
        if empty_space_length < 1 {
            // everything doesn't fit => sacrifice Size and truncate the Name
            let name = System::truncate_with_delimiter(&entry.name, column_width);
            let name_len = System::chars_amount(&name) as i32;
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
            entries: &Vec<Entry>, selected_index: Option<usize>, shift: usize) {
        for (index, entry) in entries.into_iter().enumerate()
                .skip(shift).take(self.display_settings.column_effective_height) {
            let selected = match selected_index {
                Some(i) => (i == index),
                None    => false,
            };
            self.list_entry(&mut cs, column_index, index - shift, &entry, selected);
        }
    }
//-----------------------------------------------------------------------------
    pub fn clear(&self, cs: &mut ColorSystem) {
        cs.set_paint(&self.window, self.settings.primary_paint);
        for y in 0..self.display_settings.height {
            self.window.mv(y, 0);
            self.window.hline(' ', self.display_settings.width);
        }
        self.window.refresh();
    }

    pub fn draw(&self, mut cs: &mut ColorSystem) {
        self.draw_borders(&mut cs);

        self.draw_left_column(&mut cs);
        self.draw_middle_column(&mut cs);
        self.draw_right_column(&mut cs);

        self.draw_current_path(&mut cs);
        self.draw_current_permission(&mut cs);
        self.draw_current_size(&mut cs);

        self.window.refresh();
    }

    pub fn draw_available_matches(&self, cs: &mut ColorSystem,
            matches: &Matches, completion_count: usize) {
        if matches.is_empty() { return; }

        // Borders
        cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default));
        let y = self.display_settings.height - 2 - matches.len() as i32 - 1;
        self.window.mv(y, 0);
        self.window.hline(ACS_HLINE(), self.display_settings.width);
        self.window.mv(self.display_settings.height - 2, 0);
        self.window.hline(ACS_HLINE(), self.display_settings.width);

        let max_len = max_combination_len() as i32;
        for (i, (combination, command)) in matches.iter().enumerate() {
            let y = y + 1 + i as i32;

            // Combination
            let (completed_part, uncompleted_part) = combination.split_at(completion_count);
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default).bold());
            mvprintw(&self.window, y, 0, &completed_part);
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default));
            printw(  &self.window,       &uncompleted_part);

            // Space till description
            let left = max_len - combination.len() as i32;
            self.window.hline(' ', left);

            // Command description
            let description = description_of(&command);
            mvprintw(&self.window, y, max_len as i32, &description);

            // Space till end
            let left = self.display_settings.width - max_len - description.len() as i32;
            self.window.hline(' ', left);
        }
    }

    fn draw_empty_sign(&self, cs: &mut ColorSystem, column_index: usize) {
        cs.set_paint(&self.window, Paint::with_fg_bg(Color::Black, Color::Red).bold());
        let (begin, _) = self.display_settings.columns_coord[column_index];
        mvprintw(&self.window, self.display_settings.entries_display_begin, begin + 1, "empty");
    }

    fn draw_column(&self, color_system: &mut ColorSystem, x: Coord) {
        color_system.set_paint(&self.window, self.settings.primary_paint);
        self.window.mv(1, x);
        self.window.addch(ACS_TTEE());
        self.window.mv(2, x);
        self.window.vline(ACS_VLINE(), self.display_settings.height-4);
        self.window.mv(self.display_settings.height-2, x);
        self.window.addch(ACS_BTEE());
    }

    fn draw_borders(&self, color_system: &mut ColorSystem) {
        color_system.set_paint(&self.window, self.settings.primary_paint);
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

        // For columns
        for (start, _end) in self.display_settings.columns_coord.iter().skip(1) {
            self.draw_column(color_system, *start);
        }
    }
//-----------------------------------------------------------------------------
    fn maybe_truncate(string: &str, max_length: usize) -> String {
        let mut result = String::new();
        let string = string.replace("\t", "    "); // assume tab_size=4
        let mut chars = string.chars().take(max_length);
        while let Some(c) = chars.next() {
            result.push(c);
        }
        result
    }

    fn truncate_with_delimiter(string: &str, max_length: i32) -> String {
        let chars_amount = System::chars_amount(&string);
        if chars_amount > max_length as usize {
            let delimiter = "...";
            let leave_at_end = 5;
            let total_end_len = leave_at_end + delimiter.len();
            let start = max_length as usize - total_end_len;
            let end = chars_amount - leave_at_end;
            System::replace_range_with(string, start..end, delimiter)
        } else {
            string.clone().to_string()
        }
    }

    // Does not validate the range
    // May implement in the future: https://crates.io/crates/unicode-segmentation
    fn replace_range_with<R>(string: &str, chars_range: R, replacement: &str) -> String
            where R: RangeBounds<usize> {
        use std::ops::Bound::*;
        let start = match chars_range.start_bound() {
            Unbounded   => 0,
            Included(n) => *n,
            Excluded(n) => *n + 1,
        };
        let chars_count = string.chars().count(); // TODO: improve
        let end = match chars_range.end_bound() {
            Unbounded   => chars_count,
            Included(n) => *n + 1,
            Excluded(n) => *n,
        };

        let mut chars = string.chars();
        let mut result = String::new();
        for _ in 0..start { result.push(chars.next().unwrap()); } // push first part
        result.push_str(replacement); // push the replacement
        let mut chars = chars.skip(end - start); // skip this part in the original
        while let Some(c) = chars.next() { result.push(c); } // push the rest
        result
    }

    fn maybe_selected_paint_from(paint: Paint, convert: bool) -> Paint {
        if convert {
            let Paint {fg, mut bg, bold: _, underlined} = paint;
            if bg == Color::Default { bg = Color:: Black; }
            Paint {fg: bg, bg: fg, bold: true, underlined}
        } else { paint }
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

    fn chars_amount(string: &str) -> usize {
        string.chars().count()
    }
//-----------------------------------------------------------------------------
    fn setup() -> Window {
        let window = initscr();
        window.refresh();
        window.keypad(true);
        start_color();
        use_default_colors();
        noecho();

        window
    }

    fn get_height_width(window: &Window) -> (i32, i32) {
        window.get_max_yx()
    }

    pub fn get(&self) -> Option<Input> {
        self.window.getch()
    }

    pub fn resize(&mut self) {
        self.display_settings = System::generate_display_settings(
            &self.window, self.settings.scrolling_gap, &self.settings.columns_ratio);
        self.right_column = self.collect_right_column_of_current();
        self.parent_siblings_shift = System::siblings_shift_for(
                self.display_settings.scrolling_gap, self.display_settings.column_effective_height,
                self.parent_index, self.parent_siblings.len(), None);
        self.current_siblings_shift = System::siblings_shift_for(
                self.display_settings.scrolling_gap, self.display_settings.column_effective_height,
                self.current_index, self.current_siblings.len(), None);
    }
}

impl Drop for System {
    fn drop(&mut self) {
        endwin();
        println!("Done");
    }
}
