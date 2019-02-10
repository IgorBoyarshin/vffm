use pancurses::{Window, initscr, start_color, use_default_colors, noecho, half_delay, endwin, curs_set, nocbreak,
    ACS_CKBOARD, ACS_VLINE, ACS_HLINE, ACS_TTEE, ACS_BTEE,
    ACS_LLCORNER, ACS_LRCORNER, ACS_ULCORNER, ACS_URCORNER};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::collections::{HashMap, HashSet};
use std::ops::RangeBounds;
use std::ffi::OsStr;
use std::process::Child;
use std::time::SystemTime;

use crate::coloring::*;
use crate::filesystem::*;
use crate::input::*;


//-----------------------------------------------------------------------------
#[derive(Clone)]
enum InputMode {
    Search(SearchTools),
    ChangeName(ChangeNameTools),
    Command(CommandTools),
}

#[derive(Clone)]
struct SearchTools {
    query: String,
    cursor_index: Option<usize>, // None if not if focus
    current_siblings_backup: Vec<DirEntry>,
}

#[derive(Clone)]
struct ChangeNameTools {
    new_name: String,
    cursor_index: usize,
}

#[derive(Clone)]
struct CommandTools {
    text: String,
    cursor_index: usize,
}

pub struct Settings {
    pub paint_settings: PaintSettings,
    pub primary_paint: Paint,
    pub preview_paint: Paint,

    pub columns_ratio: Vec<u32>,
    pub scrolling_gap: usize,
    pub copy_done_notification_delay_ms: Millis,
}

pub struct PaintSettings {
    pub dir_paint: Paint,
    pub symlink_paint: Paint,
    pub file_paint: Paint,
    pub unknown_paint: Paint,
}

struct DisplaySettings {
    height: Coord,
    width:  Coord,
    columns_coord: Vec<(Coord, Coord)>,

    scrolling_gap: usize, // const
    column_effective_height: usize, // const
    entries_display_begin: Coord, // const
}

#[derive(Clone)]
struct Context {
    current_siblings: Vec<DirEntry>,
    parent_siblings: Vec<DirEntry>,
    right_column: RightColumn, // depends on display_settings

    current_path: Option<PathBuf>,
    parent_path: PathBuf,

    parent_index: usize,
    current_index: usize,

    parent_siblings_shift: usize, // depends on display_settings
    current_siblings_shift: usize, // depends on display_settings

    current_permissions: String,
    additional_entry_info: Option<String>,
    cumulative_size_text: Option<String>,

    input_mode: Option<InputMode>,
}
//-----------------------------------------------------------------------------
pub struct System {
    window: Window,
    settings: Settings,
    display_settings: DisplaySettings,

    sorting_type: SortingType,
    spawn_patterns: Vec<SpawnPattern>, // const

    notification: Option<Notification>,

    transfers: Vec<Transfer>,
    potential_transfer_data: Option<PotentialTransfer>,

    selected: Vec<PathBuf>,

    tabs: Vec<Tab>,
    current_tab_index: usize,
}

impl System {
    pub fn new(settings: Settings, starting_path: PathBuf) -> Self {
        let window = System::setup();
        System::set_drawing_delay(DrawingDelay::Regular);

        let selected = Vec::new();
        let sorting_type = SortingType::Lexicographically;
        let display_settings = System::generate_display_settings(
            &window, settings.scrolling_gap, &settings.columns_ratio);
        let context = System::generate_context(starting_path, &display_settings,
                               &settings.paint_settings, &sorting_type, &selected);

        System {
            window,
            settings,
            display_settings,

            sorting_type,
            spawn_patterns: System::generate_spawn_patterns(),

            notification: None,      // for Transfers

            transfers: Vec::new(),
            potential_transfer_data: None,

            selected,

            tabs: vec![Tab { name: System::tab_name_from_path(&context.parent_path), context }],
            current_tab_index: 0,
        }
    }
//-----------------------------------------------------------------------------
    fn generate_context_for(&mut self, path: PathBuf) -> Context {
        System::generate_context(path, &self.display_settings,
            &self.settings.paint_settings, &self.sorting_type, &self.selected)
    }

    fn generate_context(parent_path: PathBuf,
                        display_settings: &DisplaySettings,
                        paint_settings: &PaintSettings,
                        sorting_type: &SortingType,
                        selected: &Vec<PathBuf>) -> Context {
        let current_siblings =
            System::into_sorted_direntries(
                collect_maybe_dir(&parent_path, None),
                paint_settings, sorting_type,
                selected, Some(&parent_path));
        let parent_siblings =
            System::into_sorted_direntries(
               collect_siblings_of(&parent_path),
               paint_settings, sorting_type, selected,
               System::maybe_parent(&parent_path).as_ref());
        let first_entry_path = System::path_of_nth_entry_inside(0, &parent_path, &current_siblings);
        let first_entry_ref = System::nth_entry_inside(0, &current_siblings);
        let parent_index = System::index_of_entry_inside(&parent_path, &parent_siblings).unwrap();
        let current_index = 0;
        let column_index = 2;
        let (begin, end) = display_settings.columns_coord[column_index];
        let column_width = (end - begin) as usize;
        let right_column =
            System::collect_right_column(
                &first_entry_path, paint_settings, sorting_type,
                display_settings.column_effective_height, column_width, selected);
        let parent_siblings_shift =
            System::siblings_shift_for(
                display_settings.scrolling_gap,
                display_settings.column_effective_height,
                parent_index, parent_siblings.len(), None);
        let current_siblings_shift =
            System::siblings_shift_for(
                display_settings.scrolling_gap,
                display_settings.column_effective_height,
                current_index, current_siblings.len(), None);
        Context {
            parent_index,
            current_index,
            parent_siblings,
            current_permissions: System::string_permissions_for_entry(&first_entry_ref),
            additional_entry_info: System::get_additional_entry_info(first_entry_ref, &first_entry_path),
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
//-----------------------------------------------------------------------------
    fn index_of_entry_inside(path: &PathBuf, entries: &Vec<DirEntry>) -> Option<usize> {
        if is_root(path) { return Some(0); }
        let sought_name = path.file_name().unwrap().to_str().unwrap();
        for (index, DirEntry {name, ..}) in entries.iter().enumerate() {
            if sought_name == name { return Some(index); }
        }
        None
    }

    fn resize_scrolling_gap_until_fits(mut gap: usize, column_effective_height: usize) -> usize {
        while 2 * gap >= column_effective_height { gap -= 1; } // gap too large
        gap
    }
//-----------------------------------------------------------------------------
    fn collect_right_column_of_current(&self) -> RightColumn {
        let column_index = 2;
        let (begin, end) = self.display_settings.columns_coord[column_index];
        let column_width = (end - begin) as usize;
        let current_path = &self.context_ref().current_path;
        System::collect_right_column(current_path, &self.settings.paint_settings, &self.sorting_type,
                     self.display_settings.column_effective_height, column_width, &self.selected)
    }

    fn collect_right_column(path_opt: &Option<PathBuf>,
            paint_settings: &PaintSettings, sorting_type: &SortingType,
            max_height: usize, max_width: usize,
            selected: &Vec<PathBuf>) -> RightColumn {
        if let Some(path) = path_opt {
            if path.is_dir() { // resolved path
                return RightColumn::with_siblings(
                    System::into_sorted_direntries(
                        collect_maybe_dir(&path, Some(max_height)),
                        paint_settings, sorting_type, selected, Some(&path)));
            } else { // resolved path is a regular file
                let path = maybe_resolve_symlink_recursively(path);
                if let Some(preview) = System::read_preview_of(&path, max_height) {
                    let truncated_preview: Vec<String> = preview.into_iter()
                        .map(|line| System::maybe_truncate(line.trim_end(), max_width))
                        .collect();
                    return RightColumn::with_preview(truncated_preview);
                }
            }
        }
        RightColumn::empty()
    }
//-----------------------------------------------------------------------------
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
        add_to_apps(&mut apps_extensions, "vlc @", vec!["mkv", "avi", "mp4", "mp3", "m4b"], external);
        add_to_apps(&mut apps_extensions, "zathura @", vec!["pdf", "djvu"], external);
        add_to_apps(&mut apps_extensions, "rifle_sxiv @", vec!["jpg", "jpeg", "png"], external);

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

    fn spawn_rule_for(&self, full_path: &PathBuf) -> Option<(String, Vec<String>, bool)> {
        let file_name = full_path.file_name().unwrap().to_str().unwrap();
        let full_path = full_path.to_str().unwrap();
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

    fn execute_command_from(path: &PathBuf, command: &str) {
        let (app, args) = System::split_into_app_and_args(command);
        Command::new(app).args(args)
            .stderr(Stdio::null()).stdout(Stdio::piped())
            .current_dir(path)
            .spawn().expect("failed to execute process");
    }

    fn spawn_process_async<S: AsRef<OsStr>>(app: &str, args: Vec<S>) -> Child {
        Command::new(app).args(args)
            .stderr(Stdio::null()).stdout(Stdio::piped())
            .spawn().expect("failed to execute process")
    }

    fn spawn_process_wait<S: AsRef<OsStr>>(app: &str, args: Vec<S>) {
        Command::new(app).args(args)
            .stderr(Stdio::null()).stdout(Stdio::null())
            .status().expect("failed to execute process");
    }

    fn spawn_program<S: AsRef<OsStr>>(app: &str, args: Vec<S>, is_external: bool) {
        if is_external {
            Command::new(app).args(args)
                .stderr(Stdio::null()).stdout(Stdio::null())
                .spawn().expect("failed to execute process");
        } else {
            Command::new(app).args(args)
                .status().expect("failed to execute process");
        }
    }
//-----------------------------------------------------------------------------
    fn current_contains(&self, name: &str) -> bool {
        for entry in self.context_ref().current_siblings.iter() {
            if entry.name == name { return true; }
        }
        false
    }

    fn cut(src: &str, dst: &str) {
        System::spawn_process_async("mv", vec![src, dst]);
    }

    fn yank(src: &str, dst: &str) {
        System::spawn_process_async("cp", vec!["-a", src, dst]);
        // System::spawn_process_async("rsync", vec!["-a", "-v", "-h", src, dst]);
    }

    pub fn paste_into_current(&mut self) {
        if let Some(data) = self.potential_transfer_data.as_ref() {
            let dst_paths: Vec<PathBuf> = data.src_paths.iter()
                .map(|src_path| {
                    let mut dst_name = file_name(src_path);
                    while self.current_contains(&dst_name) { dst_name += "_"; }
                    self.context_ref().parent_path.join(dst_name)
                }).collect();
            for (src_path, dst_path) in data.src_paths.iter().zip(dst_paths.iter()) {
                let mut src = path_to_string(src_path);
                match data.transfer_type {
                    TransferType::Cut  => if is_dir(src_path) { src += "/"; },
                    TransferType::Yank => if is_dir(src_path) { src += "/."; },
                }

                let dst = path_to_string(dst_path);
                match data.transfer_type {
                    TransferType::Cut  => System::cut(&src, &dst),
                    TransferType::Yank => System::yank(&src, &dst),
                }
            }
            self.transfers.push(self.potential_transfer_data.take().unwrap().with_dst_paths(dst_paths));
            System::set_drawing_delay(DrawingDelay::Transfering);
            self.update_current();
        }
    }

    // TODO: mb merge with cut_selected
    pub fn yank_selected(&mut self) {
        if self.selected.is_empty() {
            if let Some(path) = self.context_ref().current_path.as_ref() {
                self.potential_transfer_data = Some(PotentialTransfer::yank(vec![path.clone()]));
            }
        } else {
            let selected = self.selected.drain(..).collect();
            self.potential_transfer_data = Some(PotentialTransfer::yank(selected));
        }
    }

    // TODO: mb merge with yank_selected
    pub fn cut_selected(&mut self) {
        if self.selected.is_empty() {
            if let Some(path) = self.context_ref().current_path.as_ref() {
                self.potential_transfer_data = Some(PotentialTransfer::cut(vec![path.clone()]));
            }
        } else {
            let selected = self.selected.drain(..).collect();
            self.potential_transfer_data = Some(PotentialTransfer::cut(selected));
        }
    }

    pub fn remove_selected(&mut self) {
        if self.selected.is_empty() {
            if let Some(path) = self.context_ref().current_path.as_ref() {
                System::remove(path);
            }
        } else {
            for item in self.selected.drain(..) {
                System::remove(&item);
            }
        }
        self.update_current();
    }

    fn remove(path: &PathBuf) {
        if path.is_dir() {
            System::spawn_process_wait("rm", vec!["-r", "-f", path_to_str(path)]);
        } else if path.is_file() {
            System::spawn_process_wait("rm", vec!["-f", path_to_str(path)]);
        } else { // is symlink
            System::spawn_process_wait("unlink", vec![path_to_str(path)]);
        }
    }

    fn rename(path: &PathBuf, new_name: &str) {
        let new_path = path.parent().unwrap().join(new_name);
        System::spawn_process_wait("mv", vec![path_to_str(path), path_to_str(&new_path)]);
    }

    fn split_into_app_and_args(text: &str) -> (&str, Vec<String>) {
        let mut parts = text.split_whitespace();
        let app = parts.next().unwrap();
        let mut args = Vec::new();
        for part in parts {
            if part.starts_with("-") && !part.starts_with("--") {
                // Assume valid format like -fLaG
                for c in part.chars().skip(1) {
                    let mut arg = "-".to_string();
                    arg.push(c);
                    args.push(arg);
                }
            } else {
                args.push(part.to_string());
            }
        }
        (app, args)
    }

    pub fn get_cumulative_size(&mut self) {
        if self.inside_empty_dir() { return }
        let size = cumulative_size(self.context_ref().current_path.as_ref().unwrap());
        self.context_mut().cumulative_size_text = Some("Size: ".to_string() + &System::human_size(size));
    }
//-----------------------------------------------------------------------------
    fn maybe_sync_search_backup_selection_for_current_siblings(&mut self) {
        if self.doing_search() {
            for (name, selected) in self.context_ref().current_siblings.iter()
                    .map(|entry| (entry.name.clone(), entry.is_selected))
                    .collect::<Vec<(String, bool)>>() {
                self.sync_backup_selection_for_search(&name, selected);
            }
        }
    }

    fn sync_backup_selection_for_search(&mut self, name: &str, selected: bool) {
        if let Some(InputMode::Search(SearchTools {current_siblings_backup, ..})) =
                &mut self.context_mut().input_mode {
            for entry in current_siblings_backup.iter_mut() {
                if entry.name == name {
                    entry.is_selected = selected;
                    break;
                }
            }
        }
    }

    fn doing_search(&self) -> bool {
        if let Some(InputMode::Search(_)) = self.context_ref().input_mode { true }
        else { false }
    }

    fn collect_entries_that_match(orig: &Vec<DirEntry>, pattern: &str) -> Vec<DirEntry> {
        orig.iter().filter(|e| System::contains_pattern(&e.name, &pattern)).map(|e| e.clone()).collect()
    }

    pub fn start_changing_current_name(&mut self) {
        if self.inside_empty_dir() { return; }
        let old_name = self.unsafe_current_entry_ref().name.clone();
        let old_name_len = old_name.len();
        self.context_mut().input_mode = Some(InputMode::ChangeName(ChangeNameTools {
            new_name: old_name,
            cursor_index: old_name_len,
        }));
        System::reveal_cursor();
    }

    pub fn start_command(&mut self) {
        self.context_mut().input_mode = Some(InputMode::Command(CommandTools {
            text: String::new(),
            cursor_index: 0,
        }));
        System::reveal_cursor();
    }

    // XXX: Expects that it is not possible to directly switch from one mode to another
    pub fn start_search(&mut self) {
        if let Some(InputMode::Search(search_tools)) = self.context_mut().input_mode.as_mut() {
            // Continue previously started search
            search_tools.cursor_index = Some(search_tools.query.len());
        } else { // create a new search instance
            self.context_mut().input_mode = Some(InputMode::Search(SearchTools {
                query: "".to_string(),
                cursor_index: Some(0),
                current_siblings_backup: self.context_ref().current_siblings.clone(),
            }));
        }
        System::reveal_cursor();
    }

    fn reset_input_mode(&mut self) {
        self.context_mut().input_mode = None;
        System::hide_cursor();
    }

    fn reset_input_mode_and_restore(&mut self) {
        if let Some(InputMode::Search(search_tools)) = self.context_mut().input_mode.as_mut() {
            self.context_mut().current_siblings =
                search_tools.current_siblings_backup.drain(..).collect();
        } // other modes don't need any restoration
        self.reset_input_mode();
    }

    pub fn cancel_input(&mut self) {
        let was_doing_search = match self.context_ref().input_mode {
            Some(InputMode::Search(_)) => true,
            _ => false,
        };
        self.reset_input_mode_and_restore();
        if was_doing_search {
            self.update_current_without_siblings();
        }
    }

    pub fn confirm_input(&mut self) {
        if let Some(InputMode::Search(search_tools)) = self.context_mut().input_mode.as_mut() {
            search_tools.cursor_index = None;
        } else if let Some(InputMode::ChangeName(ChangeNameTools {new_name, ..})) =
                self.context_ref().input_mode.as_ref() {
            System::rename(self.context_ref().current_path.as_ref().unwrap(), &new_name);
            self.update_current();
            self.context_mut().input_mode = None;
        } else if let Some(InputMode::Command(CommandTools {text, ..})) =
                self.context_ref().input_mode.as_ref() {
            System::execute_command_from(&self.context_ref().parent_path, text);
            // Don't update because the process is async and probably
            // hasn't finished yet => no use updating.
            // self.update_current();
            self.context_mut().input_mode = None;
        }
        System::hide_cursor();
    }

    pub fn move_input_cursor_left(&mut self) {
        if let Some(InputMode::ChangeName(ChangeNameTools {cursor_index, ..})) =
                self.context_mut().input_mode.as_mut() {
            if *cursor_index >= 1 {
                *cursor_index -= 1;
            }
        } else if let Some(InputMode::Command(CommandTools {cursor_index, ..})) =
                self.context_mut().input_mode.as_mut() {
            if *cursor_index >= 1 {
                *cursor_index -= 1;
            }
        }
    }

    pub fn move_input_cursor_right(&mut self) {
        if let Some(InputMode::ChangeName(ChangeNameTools {cursor_index, new_name})) =
                self.context_mut().input_mode.as_mut() {
            if *cursor_index + 1 <= new_name.len() { // allow one after end of text
                *cursor_index += 1;
            }
        } else if let Some(InputMode::Command(CommandTools {cursor_index, text})) =
                self.context_mut().input_mode.as_mut() {
            if *cursor_index + 1 <= text.len() { // allow one after end of text
                *cursor_index += 1;
            }
        }
    }

    // Could have been terminated already upon this call => system would have no context
    pub fn inside_input_mode(&self) -> bool {
        if !self.have_context() { return false; }
        match self.context_ref().input_mode.as_ref() {
            Some(InputMode::Search(search_tools)) => search_tools.cursor_index.is_some(),
            Some(InputMode::ChangeName(_)) => true,
            Some(InputMode::Command(_)) => true,
            _ => false,
        }
    }

    // !!! The fact that new chars are inserted at the end is used as an optimization while
    // performing an insertion step
    pub fn insert_input(&mut self, c: char) {
        if System::valid_input(c) {
            match self.context_mut().input_mode.as_mut() {
                Some(InputMode::Search(search_tools)) => {
                    search_tools.query.push(c);
                    search_tools.cursor_index.as_mut().map(|index| *index += 1);
                    // Optimization: search only among the last known matches because the new ones
                    // must be a subset of them due to appending chars to the end of query
                    let pattern = search_tools.query.clone();
                    self.context_mut().current_siblings.retain(|entry|
                        System::contains_pattern(&entry.name, &pattern));
                    self.update_current_without_siblings();
                },
                Some(InputMode::ChangeName(ChangeNameTools {cursor_index, new_name})) => {
                    // Trusts that the cursor index is valid
                    new_name.insert(*cursor_index, c);
                    *cursor_index += 1;
                },
                Some(InputMode::Command(CommandTools {cursor_index, text})) => {
                    // Trusts that the cursor index is valid
                    text.insert(*cursor_index, c);
                    *cursor_index += 1;
                },
                _ => {},
            }
        }
    }

    pub fn remove_input_before_cursor(&mut self) {
        match self.context_mut().input_mode.as_mut() {
            Some(InputMode::Search(SearchTools {query, cursor_index, current_siblings_backup})) => {
                if query.len() > 0 {
                    query.pop();
                    cursor_index.as_mut().map(|index| *index -= 1);

                    let pattern = query.clone();
                    self.context_mut().current_siblings = System::collect_entries_that_match(
                        &current_siblings_backup, &pattern);
                    self.update_current_without_siblings();
                }
            },
            Some(InputMode::ChangeName(ChangeNameTools {cursor_index, new_name})) => {
                // Trusts that the cursor index is valid
                if *cursor_index > 0 {
                    *cursor_index -= 1;
                    new_name.remove(*cursor_index);
                }
            },
            Some(InputMode::Command(CommandTools {cursor_index, text})) => {
                // Trusts that the cursor index is valid
                if *cursor_index > 0 {
                    *cursor_index -= 1;
                    text.remove(*cursor_index);
                }
            },
            _ => {},
        }
    }

    pub fn remove_input_under_cursor(&mut self) {
        match self.context_mut().input_mode.as_mut() {
            Some(InputMode::ChangeName(ChangeNameTools {cursor_index, new_name})) => {
                new_name.remove(*cursor_index);
            },
            Some(InputMode::Command(CommandTools {cursor_index, text})) => {
                text.remove(*cursor_index);
            },
            _ => {},
        }
    }

    fn contains_pattern(string: &str, pattern: &str) -> bool {
        let pattern_lowercase = pattern.to_lowercase();
        let case_sensitive = pattern_lowercase != pattern;
        if case_sensitive { string               .contains( pattern          ) }
        else              { string.to_lowercase().contains(&pattern_lowercase) }
    }

    fn valid_input(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '!' || c == '@' || c == '#' || c == '$' ||
            c == '%' || c == '^' || c == '&' || c == '*' || c == '(' || c == ')' || c == '-' ||
            c == '=' || c == '+' || c == '.' || c == ',' || c == '?' || c == '"' || c == '\'' ||
            c == '[' || c == ']' || c == '{' || c == '}' || c == '<' || c == '>' || c == '~' ||
            c == '\\' || c == '|' || c == ':' || c == ';' || c == '/' || c == ' '
    }
//-----------------------------------------------------------------------------
    fn update_current_tab_name(&mut self) {
        let parent_path = &self.context_ref().parent_path;
        self.current_tab_mut().name = System::tab_name_from_path(parent_path);
    }

    fn tab_name_from_path(path: &PathBuf) -> String {
        if path == &PathBuf::from("/") { "/".to_string() }
        else { osstr_to_str(path.file_name().unwrap()).to_string() }
    }

    // Returns whether it was the last Tab (perhaps whether we should terminate)
    pub fn close_tab(&mut self) -> bool {
        self.tabs.remove(self.current_tab_index);
        if self.tabs.is_empty() {
            self.current_tab_index = 0;
        } else if self.current_tab_index >= self.tabs.len() {
            self.current_tab_index = self.tabs.len() - 1
        }
        self.tabs.is_empty()
    }

    pub fn new_tab(&mut self) {
        const MAX_TABS: usize = 8;
        if self.tabs.len() + 1 > MAX_TABS { return; }
        // The trick here is that we create a copy of the current tab and refer
        // to the copy as to the old tab, and start reigning in the new one
        self.tabs.push(self.current_tab_ref().clone());
        self.reset_input_mode_and_restore(); // for the new tab
    }

    pub fn next_tab(&mut self) {
        if self.current_tab_index == self.tabs.len() - 1 {
            self.current_tab_index = 0;
        } else {
            self.current_tab_index += 1;
        }
    }

    pub fn previous_tab(&mut self) {
        if self.current_tab_index == 0 {
            self.current_tab_index = self.tabs.len() - 1;
        } else {
            self.current_tab_index -= 1;
        }
    }
//-----------------------------------------------------------------------------
    pub fn sort_with(&mut self, new_sorting_type: SortingType) {
        self.sorting_type = new_sorting_type;
        self.update_current();
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
//-----------------------------------------------------------------------------
    fn string_permissions_for_entry(entry: &Option<&DirEntry>) -> String {
        if let Some(entry) = entry {
            entry.permissions.string_representation()
        } else { "".to_string() }
    }

    fn get_current_permissions(&mut self) -> String {
        System::string_permissions_for_entry(&self.current_entry_ref())
    }

    fn get_additional_entry_info_for_current(&self) -> Option<String> {
        let context = self.context_ref();
        System::get_additional_entry_info(self.current_entry_ref(), &context.current_path)
    }

    fn get_additional_entry_info(entry: Option<&DirEntry>, path: &Option<PathBuf>)
            -> Option<String> {
        if let Some(entry) = entry {
            if entry.is_dir() { // get sub-entries count
                // if let Some(siblings) = right_column.siblings_ref() {
                //     let count = siblings.len().to_string();
                //     let text = "Entries inside: ".to_string() + &count;
                //     return Some(text);
                // }
            } else if entry.is_symlink() { // get the path the link points to
                if let Some(result) = System::get_symlink_target(path) {
                    let text = "-> ".to_string() + &result;
                    return Some(text);
                }
            }
        }
        None
    }

    fn get_symlink_target(path: &Option<PathBuf>) -> Option<String> {
        if let Some(path) = path {
            if is_symlink(path) {
                if let Some(resolved) = resolve_symlink(path) {
                    return Some(path_to_str(&resolved).to_string());
                }
            }
        }
        None
    }

    fn current_entry_ref(&self) -> Option<&DirEntry> {
        if self.context_ref().current_path.is_some() {
            Some(self.unsafe_current_entry_ref())
        } else { None }
    }

    // fn current_entry_mut(&mut self) -> Option<&mut DirEntry> {
    //     if self.context_ref().current_path.is_some() {
    //         Some(self.unsafe_current_entry_mut())
    //     } else { None }
    // }

    fn unsafe_current_entry_ref(&self) -> &DirEntry {
        self.context_ref().current_siblings.get(self.context_ref().current_index).unwrap()
    }

    fn unsafe_current_entry_mut(&mut self) -> &mut DirEntry {
        let index = self.context_ref().current_index;
        self.context_mut().current_siblings.get_mut(index).unwrap()
    }

    fn current_tab_ref(&self) -> &Tab {
        self.tabs.get(self.current_tab_index).unwrap()
    }

    fn current_tab_mut(&mut self) -> &mut Tab {
        self.tabs.get_mut(self.current_tab_index).unwrap()
    }

    fn context_ref(&self) -> &Context {
        &self.current_tab_ref().context
    }

    fn context_mut(&mut self) -> &mut Context {
        &mut self.current_tab_mut().context
    }

    fn have_context(&self) -> bool {
        !self.tabs.is_empty()
    }

    fn inside_empty_dir(&self) -> bool {
        self.context_ref().current_path.is_none()
    }

    fn path_of_nth_entry_inside(n: usize, path: &PathBuf, entries: &Vec<DirEntry>) -> Option<PathBuf> {
        if entries.is_empty() { return None; }
        if n >= entries.len() { return None; }
        let name = entries[n].name.clone();
        let mut path = path.clone();
        path.push(name);
        Some(path)
    }

    fn nth_entry_inside(n: usize, entries: &Vec<DirEntry>) -> Option<&DirEntry> {
        if entries.is_empty() { return None; }
        if n >= entries.len() { return None; }
        Some(entries.get(n).unwrap())
    }
//-----------------------------------------------------------------------------
    // Update all, affectively reloading everything
    fn update(&mut self) {
        if self.context_ref().current_path.is_none() { return; }
        let current_path = self.context_ref().current_path.as_ref().unwrap().clone();
        self.current_tab_mut().context = self.generate_context_for(current_path);
        self.update_current_tab_name();
    }

    // Update central column and right column
    pub fn update_current(&mut self) {
        self.context_mut().current_siblings = self.collect_sorted_children_of_parent();
        self.update_current_without_siblings();
    }

    pub fn update_current_without_siblings(&mut self) {
        let len = self.context_ref().current_siblings.len();
        self.context_mut().current_index =
            if self.context_ref().current_siblings.is_empty() { 0 } // reset for future
            else if self.context_ref().current_index >= len   { len - 1 } // update to valid
            else                                              { self.context_ref().current_index }; // leave old
        self.context_mut().current_path = System::path_of_nth_entry_inside(
            self.context_ref().current_index, &self.context_ref().parent_path,
            &self.context_ref().current_siblings);
        self.context_mut().right_column = self.collect_right_column_of_current();
        self.context_mut().current_permissions = self.get_current_permissions();
        self.context_mut().current_siblings_shift = self.recalculate_current_siblings_shift();
        self.context_mut().additional_entry_info = self.get_additional_entry_info_for_current();
        if let Some(path) = self.context_ref().current_path.as_ref() {
            if let Some(new_size) = maybe_size(path) {
                self.unsafe_current_entry_mut().size = new_size;
            }
        }
    }

    fn update_transfer_progress(&mut self) {
        let mut finished_some = false;
        for transfer in self.transfers.iter_mut() {
            let dst_sizes: Vec<Size> = transfer.dst_paths.iter()
                .zip(transfer.dst_sizes.iter())
                .map(|(path, &size)|
                     if let Some(size) = size { size } else { cumulative_size(path) })
                .collect();
            for (index, &size) in dst_sizes.iter().enumerate() {
                let finished_this_one = size == transfer.src_sizes[index];
                if finished_this_one { // then cache for later
                    transfer.dst_sizes   [index] = Some(size);
                }
            }
            let src_cumulative_size: Size = transfer.src_sizes.iter().sum();
            let dst_cumulative_size: Size =          dst_sizes.iter().sum();

            if src_cumulative_size == dst_cumulative_size { // finished
                // Can remove this transfer now. Do it after this loop with retain()
                let text = match transfer.transfer_type {
                    TransferType::Cut  => "Done moving!",
                    TransferType::Yank => "Done copying!",
                };
                self.notification = Some(Notification::new(text, 3000));
                finished_some = true;
            } else { // partially finished
                let percentage = (100 * dst_cumulative_size / src_cumulative_size) as u32;
                let text = match transfer.transfer_type {
                    TransferType::Cut  => format!("Moving...({}% done)", percentage),
                    TransferType::Yank => format!("Copying...({}% done)", percentage),
                };
                self.notification = Some(Notification::new(&text, 3000));
                System::set_drawing_delay(DrawingDelay::Transfering);
            }
        }

        // TODO: mb remove
        if finished_some || !self.transfers.is_empty() { self.update_current(); }

        // Leave only the ones with uncompleted (None) sizes left
        self.transfers.retain(|t| !t.dst_sizes.iter().all(|&size| size.is_some()));

        // Slow down the pace
        if self.transfers.is_empty() { System::set_drawing_delay(DrawingDelay::Regular); }
    }
//-----------------------------------------------------------------------------
    fn into_direntries(entries: Vec<Entry>, paint_settings: &PaintSettings,
            selected: &Vec<PathBuf>, parent_path: Option<&PathBuf>) -> Vec<DirEntry> {
        entries.into_iter().map(|entry| {
            let is_selected = System::is_selected(selected, &entry.name, parent_path);
            DirEntry::from_entry(entry, paint_settings, is_selected)
        }).collect()
    }

    fn is_selected(selected: &Vec<PathBuf>, name: &str, parent_path: Option<&PathBuf>) -> bool {
        if let Some(parent_path) = parent_path {
            return selected.iter().any(|item| item.ends_with(name) // check partially
                                && *item == parent_path.join(name)); // verify
        } else { false }
    }

    fn maybe_parent(path: &PathBuf) -> Option<PathBuf> {
        if path_to_str(path) == "/" { None }
        else { Some(path.parent().unwrap().to_path_buf()) }
    }

    fn into_sorted_direntries(entries: Vec<Entry>,
                              paint_settings: &PaintSettings,
                              sorting_type: &SortingType,
                              selected: &Vec<PathBuf>,
                              parent_path: Option<&PathBuf>) -> Vec<DirEntry> {
        let entries = System::into_direntries(entries, paint_settings, selected, parent_path);
        System::sort(entries, sorting_type)
    }

    fn collect_sorted_siblings_of_parent(&self) -> Vec<DirEntry> {
        let grandparent = System::maybe_parent(&self.context_ref().parent_path);
        System::into_sorted_direntries(
            collect_siblings_of(&self.context_ref().parent_path),
            &self.settings.paint_settings, &self.sorting_type,
            &self.selected, grandparent.as_ref())
    }

    fn collect_sorted_children_of_parent(&self) -> Vec<DirEntry> {
        System::into_sorted_direntries(
            collect_maybe_dir(&self.context_ref().parent_path, None),
            &self.settings.paint_settings, &self.sorting_type,
            &self.selected, Some(&self.context_ref().parent_path)) // TODO: CHECK
    }

    // The display is guaranteed to be able to contain 2*gap (accomplished in settings)
    fn siblings_shift_for(gap: usize, max: usize, index: usize,
                              len: usize, old_shift: Option<usize>) -> usize {
        let gap   = gap   as Coord;
        let max   = max   as Coord;
        let index = index as Coord;
        let len   = len   as Coord;

        if len <= max         { return 0; }
        if index < gap        { return 0; }
        if index >= len - gap { return (len - max) as usize; }

        if let Some(old_shift) = old_shift {
            let old_shift = old_shift as Coord;

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
            self.display_settings.scrolling_gap,
            self.display_settings.column_effective_height,
            self.context_ref().parent_index,
            self.context_ref().parent_siblings.len(),
            None)
    }

    fn recalculate_current_siblings_shift(&mut self) -> usize {
        System::siblings_shift_for(
            self.display_settings.scrolling_gap,
            self.display_settings.column_effective_height,
            self.context_ref().current_index,
            self.context_ref().current_siblings.len(),
            Some(self.context_ref().current_siblings_shift))
    }

    fn update_current_entry_by_index(&mut self) {
        self.context_mut().cumulative_size_text = None;
        self.update_last_part_of_current_path_by_index();
        self.context_mut().current_permissions = self.get_current_permissions();
        self.context_mut().right_column = self.collect_right_column_of_current();
        self.context_mut().current_siblings_shift = self.recalculate_current_siblings_shift();
        self.context_mut().additional_entry_info = self.get_additional_entry_info_for_current();
        if let Some(path) = self.context_ref().current_path.as_ref() {
            if let Some(new_size) = maybe_size(path) {
                self.unsafe_current_entry_mut().size = new_size;
            }
        }
    }

    fn update_last_part_of_current_path_by_index(&mut self) {
        let name = self.unsafe_current_entry_ref().name.clone();
        self.context_mut().current_path.as_mut().map(|path| {
            (*path).set_file_name(name); // try_pop + push(name)
        });
    }

    fn common_left_right(&mut self) {
        self.context_mut().cumulative_size_text = None;
        self.context_mut().right_column = self.collect_right_column_of_current();
        self.context_mut().current_siblings = self.collect_sorted_children_of_parent();
        self.context_mut().parent_siblings = self.collect_sorted_siblings_of_parent();
        self.context_mut().additional_entry_info = self.get_additional_entry_info_for_current();
        self.context_mut().current_permissions = self.get_current_permissions();
        self.reset_input_mode();
        self.update_current_tab_name();
    }
//-----------------------------------------------------------------------------
    pub fn up(&mut self) {
        if self.inside_empty_dir() { return }
        if self.context_ref().current_index > 0 {
            self.context_mut().current_index -= 1;
            self.update_current_entry_by_index();
        }
    }

    pub fn down(&mut self) {
        if self.inside_empty_dir() { return }
        if self.context_ref().current_index < self.context_ref().current_siblings.len() - 1 {
            self.context_mut().current_index += 1;
            self.update_current_entry_by_index();
        }
    }

    pub fn left(&mut self) {
        if !is_root(&self.context_ref().parent_path) {
            if self.context_ref().current_path.is_none() {
                self.context_mut().current_path = Some(self.context_ref().parent_path.clone());
            } else {
                self.context_mut().current_path.as_mut().map(|path| path.pop());
            }
            self.context_mut().parent_path.pop();

            self.context_mut().current_index = self.context_ref().parent_index;
            self.common_left_right();

            self.context_mut().parent_index = System::index_of_entry_inside(
                &self.context_ref().parent_path, &self.context_ref().parent_siblings).unwrap();
            self.context_mut().current_siblings_shift = self.context_ref().parent_siblings_shift;
            self.context_mut().parent_siblings_shift = self.recalculate_parent_siblings_shift();
        }
    }

    pub fn right(&mut self) {
        if self.inside_empty_dir() { return; }
        // Have to resort to cloning so that Rust does not complain about immutable reference:
        let current_path = self.context_ref().current_path.as_ref().unwrap().clone();
        if current_path.is_dir() { // Traverses symlinks. The resolved path points to a dir
            // Navigate inside
            // Deliberately use the not-resolved version, so the path contains the symlink
            self.context_mut().parent_path = current_path.to_path_buf();
            self.context_mut().current_path = System::path_of_nth_entry_inside(
                0, &current_path, self.context_ref().right_column.siblings_ref().unwrap());

            self.context_mut().parent_index = self.context_ref().current_index;
            self.context_mut().current_index = 0;
            self.context_mut().parent_siblings_shift = self.context_ref().current_siblings_shift;
            self.context_mut().current_siblings_shift = 0;
            self.common_left_right();
        } else { // Resolved path points to a file
            // Try to open with default app
            let path = maybe_resolve_symlink_recursively(&current_path);
            if let Some((app, args, is_external)) = self.spawn_rule_for(&path) {
                System::spawn_program(&app, args, is_external);
                self.update_current();
                self.window.clear(); // Otherwise the screen is not restored correctly. Need to invalidate
            }
        }
    }

    pub fn goto(&mut self, path: &str) {
        self.context_mut().current_path = Some(PathBuf::from(path));
        self.context_mut().cumulative_size_text = None;
        self.update();
    }

    pub fn go_home(&mut self) {
        if self.inside_empty_dir() { return }
        self.context_mut().current_index = 0;
        self.update_current_entry_by_index();
    }

    pub fn go_end(&mut self) {
        if self.inside_empty_dir() { return }
        self.context_mut().current_index = self.context_ref().current_siblings.len() - 1;
        self.update_current_entry_by_index();
    }
//-----------------------------------------------------------------------------
    fn list_entry(&self, cs: &mut ColorSystem, column_index: usize,
            y: usize, entry: &DirEntry, under_cursor: bool, selected: bool) {
        let paint = System::maybe_selected_paint_from(entry.paint, under_cursor);

        let y = y as Coord + self.display_settings.entries_display_begin;
        let (mut begin, end) = self.display_settings.columns_coord[column_index];
        if selected {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Red, Color::Default));
            self.window.mvaddch(y, begin + 1, ACS_CKBOARD());
            self.window.mvaddch(y, begin + 2, ' ');
            // cs.set_paint(&self.window, Paint::with_fg_bg(Color::Yellow, Color::Default));
            // self.window.mvaddch(y, begin + 2, ACS_CKBOARD());
            begin += 2;
        }
        let column_width = end - begin;
        let size = System::human_size(entry.size);
        let size_len = size.len();
        let name_len = System::chars_amount(&entry.name) as Coord;
        let empty_space_length = column_width - name_len - size_len as Coord;
        cs.set_paint(&self.window, paint);
        if empty_space_length < 1 {
            // everything doesn't fit => sacrifice Size and truncate the Name
            let name = System::truncate_with_delimiter(&entry.name, column_width);
            let name_len = System::chars_amount(&name) as Coord;
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
//-----------------------------------------------------------------------------
    fn maybe_index_of_selected(path: &PathBuf, selected: &Vec<PathBuf>) -> Option<usize> {
        selected.iter().enumerate()
            .find(|(_, item)| *item == path)
            .map(|(index, _)| index)
    }

    fn maybe_index_of_self_selected(&self, path: &PathBuf) -> Option<usize> {
        System::maybe_index_of_selected(path, &self.selected)
    }

    pub fn select_under_cursor(&mut self) {
        if let Some(path) = self.context_ref().current_path.as_ref() {
            if let Some(index) = self.maybe_index_of_self_selected(path) {
                // Unselect
                self.selected.remove(index);
                assert!(self.unsafe_current_entry_mut().is_selected);
                self.unsafe_current_entry_mut().is_selected = false;
            } else {
                // Select
                self.selected.push(path.clone());
                assert!(!self.unsafe_current_entry_mut().is_selected);
                self.unsafe_current_entry_mut().is_selected = true;
            }

            if self.doing_search() {
                let selected = self.unsafe_current_entry_ref().is_selected;
                let name = self.unsafe_current_entry_ref().name.clone();
                self.sync_backup_selection_for_search(&name, selected);
            }

            self.down(); // for user convenience
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.potential_transfer_data = None;

        // Unselect all siblings
        self.context_mut().parent_siblings.iter_mut().for_each(|e| e.is_selected = false);
        self.context_mut().current_siblings.iter_mut().for_each(|e| e.is_selected = false);
        if let Some(siblings) = self.context_mut().right_column.siblings_mut() {
            siblings.iter_mut().for_each(|e| e.is_selected = false);
        }

        self.maybe_sync_search_backup_selection_for_current_siblings();
    }

    pub fn invert_selection(&mut self) {
        let parent_path = self.context_ref().parent_path.clone();

        // Remove those that were among current siblings and were selected
        let to_remove = self.context_ref().current_siblings.iter()
            .filter(|e| e.is_selected)
            .map(|e| parent_path.join(&e.name))
            .collect::<HashSet<_>>();
        self.selected.retain(|path| !to_remove.contains(path));

        // Flip selection
        self.context_mut().current_siblings.iter_mut()
            .for_each(|e| e.is_selected = !e.is_selected);

        // Add new selected ones to the selected list
        let mut to_add = self.context_ref().current_siblings.iter()
            .filter(|e| e.is_selected)
            .map(|e| parent_path.join(&e.name))
            .collect();
        self.selected.append(&mut to_add);

        // Sync
        self.maybe_sync_search_backup_selection_for_current_siblings();
    }
//-----------------------------------------------------------------------------
    fn clear(&self, cs: &mut ColorSystem) {
        cs.set_paint(&self.window, self.settings.primary_paint);
        for y in 0..self.display_settings.height {
            self.window.mv(y, 0);
            self.window.hline(' ', self.display_settings.width);
        }
    }

    pub fn draw(&mut self, mut cs: &mut ColorSystem) {
        self.clear(&mut cs);

        self.update_transfer_progress();

        self.draw_borders(&mut cs);
        self.draw_left_column(&mut cs);
        self.draw_middle_column(&mut cs);
        self.draw_right_column(&mut cs);

        self.draw_current_path(&mut cs);

        let mut bottom_bar = Bar::with_y_and_width(
            self.display_settings.height - 1, self.display_settings.width);
        self.maybe_draw_input_mode(&mut cs, &mut bottom_bar);
        self.draw_current_permission(&mut cs, &mut bottom_bar);
        self.draw_current_size(&mut cs, &mut bottom_bar);
        self.maybe_draw_additional_info_for_current(&mut cs, &mut bottom_bar);
        self.draw_current_dir_siblings_count(&mut cs, &mut bottom_bar);
        self.draw_cumulative_size_text(&mut cs, &mut bottom_bar);
        self.update_and_draw_notification(&mut cs, &mut bottom_bar);
        self.maybe_draw_selection_warning(&mut cs, &mut bottom_bar);

        let mut top_bar = Bar::with_y_and_width(0, self.display_settings.width);
        self.draw_tabs(&mut cs, &mut top_bar);

        self.maybe_draw_input_mode_cursor();

        self.window.refresh();
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

    fn draw_left_column(&self, mut cs: &mut ColorSystem) {
        let column_index = 0;
        let context = self.context_ref();
        self.list_entries(&mut cs, column_index, &context.parent_siblings,
                Some(context.parent_index), context.parent_siblings_shift);
    }

    fn draw_middle_column(&self, mut cs: &mut ColorSystem) {
        let column_index = 1;
        if self.inside_empty_dir() {
            self.draw_empty_sign(&mut cs, column_index);
        } else {
            let context = self.context_ref();
            self.list_entries(&mut cs, column_index, &context.current_siblings,
                Some(context.current_index), context.current_siblings_shift);
        }
    }

    fn draw_right_column(&self, mut cs: &mut ColorSystem) {
        let column_index = 2;
        if let Some(siblings) = self.context_ref().right_column.siblings_ref() {
            // Have siblings (Some or None) => are sure to be in a dir or symlink
            if siblings.is_empty() {
                self.draw_empty_sign(&mut cs, column_index);
            } else {
                self.list_entries(&mut cs, column_index, siblings, None, 0);
            }
        } else if let Some(preview) = self.context_ref().right_column.preview_ref() {
            let (begin, _) = self.display_settings.columns_coord[column_index];
            let y = self.display_settings.entries_display_begin;
            cs.set_paint(&self.window, self.settings.preview_paint);
            for (i, line) in preview.iter().enumerate() {
                mvprintw(&self.window, y + i as Coord, begin + 1, line);
            }
        } // display nothing otherwise
    }

    fn draw_current_path(&self, cs: &mut ColorSystem) {
        if !self.inside_empty_dir() {
            let path = self.context_ref().current_path.as_ref().unwrap().to_str().unwrap();
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            mvprintw(&self.window, 0, 0, path);
        }
    }

    fn maybe_draw_input_mode(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        match self.context_ref().input_mode.as_ref() {
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

    fn maybe_draw_input_mode_cursor(&self) {
        match self.context_ref().input_mode.as_ref() {
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

    fn draw_current_permission(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if self.context_ref().current_path.is_some() {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            bar.draw_left(&self.window, &self.context_ref().current_permissions, 2);
        }
    }

    fn draw_current_size(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if self.context_ref().current_path.is_some() {
            let size = System::human_size(self.unsafe_current_entry_ref().size);
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Blue, Color::Default));
            bar.draw_left(&self.window, &size, 2);
        }
    }

    fn maybe_draw_additional_info_for_current(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if let Some(info) = self.context_ref().additional_entry_info.as_ref() {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            bar.draw_left(&self.window, &info, 2);
        }
    }

    fn draw_current_dir_siblings_count(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        let count = self.context_ref().current_siblings.len().to_string();
        let text = "Siblings = ".to_string() + &count;
        cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
        bar.draw_left(&self.window, &text, 2);
    }

    fn draw_cumulative_size_text(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if let Some(text) = self.context_ref().cumulative_size_text.as_ref() {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default));
            bar.draw_left(&self.window, text, 2);
        }
    }

    fn update_and_draw_notification(&mut self, cs: &mut ColorSystem, bar: &mut Bar) {
        if let Some(notification) = self.notification.as_ref() {
            if notification.has_finished() {
                self.notification = None;
            } else {
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::Green, Color::Default));
                bar.draw_right(&self.window, &notification.text, 2);
            }
        }
    }

    fn maybe_draw_selection_warning(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if !self.selected.is_empty() {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Red, Color::Default).bold());
            bar.draw_left(&self.window, "Selection not empty", 2);
        }
    }

    fn draw_tabs(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if self.tabs.len() == 1 { return; }
        for (index, tab) in self.tabs.iter().enumerate().rev() {
            if index == self.current_tab_index {
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default).bold());
            } else {
                cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            }
            let text = "<".to_string() + &index.to_string() + ":" + &tab.name + &">".to_string();
            bar.draw_right(&self.window, &text, 0);
        }
    }
//-----------------------------------------------------------------------------
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
//-----------------------------------------------------------------------------
    fn maybe_truncate(string: &str, max_length: usize) -> String {
        let mut result = String::new();
        let string = string.replace("\r", "^M").replace("\t", "    "); // assume tab_size=4
        let mut chars = string.chars().take(max_length);
        while let Some(c) = chars.next() {
            result.push(c);
        }
        result
    }

    fn truncate_with_delimiter(string: &str, max_length: Coord) -> String {
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
                width as Coord - 2
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
        System::hide_cursor();
        nocbreak();
        window.timeout(0);

        window
    }

    fn hide_cursor() {
        curs_set(0);
    }

    fn reveal_cursor() {
        curs_set(1);
    }

    fn set_drawing_delay(drawing_delay: DrawingDelay) {
        half_delay(drawing_delay.ms() / 100); // expects argument in tens of a second
    }

    fn get_height_width(window: &Window) -> (Coord, Coord) {
        window.get_max_yx()
    }

    pub fn get(&self) -> Option<Input> {
        use pancurses::Input as PInput;
        match self.window.getch() {
            Some(PInput::Character('\t'))   => Some(Input::Tab),
            Some(PInput::Character('\x1B')) => Some(Input::Escape), // \e === \x1B
            Some(PInput::Character('\x7f')) => Some(Input::Backspace),
            Some(PInput::Character('\x0a')) => Some(Input::Enter),
            Some(PInput::Character(c))      => Some(Input::Char(c)),
            Some(PInput::KeyBTab)           => Some(Input::ShiftTab),
            Some(PInput::KeyResize)         => Some(Input::EventResize),
            Some(PInput::KeyLeft)           => Some(Input::Left),
            Some(PInput::KeyRight)          => Some(Input::Right),
            Some(PInput::KeyDC)             => Some(Input::Delete),
            None                            => None,
            _                               => Some(Input::Unknown),
        }
    }

    pub fn resize(&mut self) {
        self.display_settings = System::generate_display_settings(
            &self.window, self.settings.scrolling_gap, &self.settings.columns_ratio);
        self.context_mut().right_column = self.collect_right_column_of_current();
        self.context_mut().parent_siblings_shift = System::siblings_shift_for(
            self.display_settings.scrolling_gap, self.display_settings.column_effective_height,
            self.context_ref().parent_index, self.context_ref().parent_siblings.len(), None);
        self.context_mut().current_siblings_shift = System::siblings_shift_for(
            self.display_settings.scrolling_gap, self.display_settings.column_effective_height,
            self.context_ref().current_index, self.context_ref().current_siblings.len(), None);
    }
}

impl Drop for System {
    fn drop(&mut self) {
        endwin();
        println!("Done");
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

fn millis_since(time: SystemTime) -> Millis {
    let elapsed = SystemTime::now().duration_since(time);
    if elapsed.is_err() { return 0; } // _now_ is earlier than _time_ => assume 0
    elapsed.unwrap().as_millis()
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
#[derive(Clone)]
struct RightColumn {
    siblings: Option<Vec<DirEntry>>,
    preview: Option<Vec<String>>,
}

impl RightColumn {
    fn with_siblings(siblings: Vec<DirEntry>) -> RightColumn {
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

    fn siblings_ref(&self) -> Option<&Vec<DirEntry>> {
        self.siblings.as_ref()
    }

    fn siblings_mut(&mut self) -> Option<&mut Vec<DirEntry>> {
        self.siblings.as_mut()
    }

    fn preview_ref(&self) -> Option<&Vec<String>> {
        self.preview.as_ref()
    }
}
//-----------------------------------------------------------------------------
enum DrawingDelay {
    Transfering,
    Regular,
}

impl DrawingDelay {
    fn ms(&self) -> i32 {
        match self {
            DrawingDelay::Transfering => 1000,
            DrawingDelay::Regular => 5000,
        }
    }
}
//-----------------------------------------------------------------------------
struct Bar {
    y: Coord,
    ready_left: Coord, // x Coord of the first not-taken cell from the left
    ready_right: Coord, // x Coord of the first taken cell after all not-talen
}

impl Bar {
    fn draw_left(&mut self, window: &Window, text: &str, padding: Coord) {
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

    fn draw_right(&mut self, window: &Window, text: &str, padding: Coord) {
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

    fn with_y_and_width(y: Coord, width: Coord) -> Bar {
        Bar {
            y,
            ready_left: 0,
            ready_right: width,
        }
    }
}
//-----------------------------------------------------------------------------
enum TransferType {
    Yank,
    Cut,
}

struct PotentialTransfer {
    src_paths: Vec<PathBuf>,
    src_sizes: Vec<Size>,
    transfer_type: TransferType,
}

struct Transfer {
    src_sizes: Vec<Size>,
    dst_paths: Vec<PathBuf>,
    dst_sizes: Vec<Option<Size>>,
    transfer_type: TransferType,
}

impl PotentialTransfer {
    fn cut(src_paths: Vec<PathBuf>) -> PotentialTransfer {
        PotentialTransfer::new(src_paths, TransferType::Cut)
    }

    fn yank(src_paths: Vec<PathBuf>) -> PotentialTransfer {
        PotentialTransfer::new(src_paths, TransferType::Yank)
    }

    fn new(src_paths: Vec<PathBuf>, transfer_type: TransferType) -> PotentialTransfer {
        let src_sizes: Vec<Size> = src_paths.iter().map(|path| cumulative_size(&path)).collect();
        PotentialTransfer {
            src_paths,
            src_sizes,
            transfer_type,
        }
    }

    fn with_dst_paths(self, dst_paths: Vec<PathBuf>) -> Transfer {
        let amount = self.src_sizes.len();
        Transfer {
            src_sizes: self.src_sizes,
            dst_sizes: vec![None; amount],
            dst_paths,
            transfer_type: self.transfer_type,
        }
    }
}
//-----------------------------------------------------------------------------
type Millis = u128;

struct Notification {
    text: String,
    show_time_millis: Millis,
    start_time: SystemTime,
}

impl Notification {
    fn new(text: &str, show_time_millis: Millis) -> Notification {
        let text = text.to_string();
        Notification {
            text,
            show_time_millis,
            start_time: SystemTime::now(),
        }
    }

    fn has_finished(&self) -> bool {
        millis_since(self.start_time) > self.show_time_millis
    }
}
//-----------------------------------------------------------------------------
#[derive(Clone)]
struct DirEntry {
    entrytype: EntryType,
    name: String,
    size: u64,
    time_modified: u64,
    permissions: Permissions,

    paint: Paint,
    is_selected: bool,
}

impl DirEntry {
    fn is_partially_executable(entry: &Entry) -> bool {
        (entry.permissions.world % 2 == 1) ||
        (entry.permissions.group % 2 == 1) ||
        (entry.permissions.owner % 2 == 1)
    }

    fn from_entry(entry: Entry, paint_settings: &PaintSettings, is_selected: bool) -> DirEntry {
        let executable = DirEntry::is_partially_executable(&entry);
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

    fn is_symlink(&self) -> bool {
        self.entrytype == EntryType::Symlink
    }
    // fn is_regular(&self) -> bool {
    //     self.entrytype == EntryType::Regular
    // }
    fn is_dir(&self) -> bool {
        self.entrytype == EntryType::Directory
    }
}

fn paint_for(entrytype: &EntryType, name: &str,
        executable: bool, paint_settings: &PaintSettings) -> Paint {
    match entrytype {
        EntryType::Directory => paint_settings.dir_paint,
        EntryType::Symlink   => paint_settings.symlink_paint,
        EntryType::Unknown   => paint_settings.unknown_paint,
        EntryType::Regular   =>
            if let Some(paint) = maybe_paint_by_name(name) { paint }
            else if executable { Paint::with_fg_bg(Color::Green, Color::Default).bold() }
            else { paint_settings.file_paint },
    }
}

fn maybe_paint_by_name(name: &str) -> Option<Paint> {
    if      name.ends_with(".cpp")  { return Some(Paint::with_fg_bg(Color::Red,    Color::Default)       ) }
    else if name.ends_with(".java") { return Some(Paint::with_fg_bg(Color::Red,    Color::Default)       ) }
    else if name.ends_with(".rs")   { return Some(Paint::with_fg_bg(Color::Red,    Color::Default)       ) }
    else if name.ends_with(".h")    { return Some(Paint::with_fg_bg(Color::Red,    Color::Default)       ) }
    else if name.ends_with(".pdf")  { return Some(Paint::with_fg_bg(Color::Yellow, Color::Default).bold()) }
    else if name.ends_with(".djvu") { return Some(Paint::with_fg_bg(Color::Yellow, Color::Default).bold()) }
    else if name.ends_with(".mp3")  { return Some(Paint::with_fg_bg(Color::Yellow, Color::Default)       ) }
    else if name.ends_with(".webm") { return Some(Paint::with_fg_bg(Color::Yellow, Color::Default)       ) }
    else if name.ends_with(".png")  { return Some(Paint::with_fg_bg(Color::Purple, Color::Default)       ) }
    else if name.ends_with(".gif")  { return Some(Paint::with_fg_bg(Color::Purple, Color::Default)       ) }
    else if name.ends_with(".jpg")  { return Some(Paint::with_fg_bg(Color::Purple, Color::Default)       ) }
    else if name.ends_with(".jpeg") { return Some(Paint::with_fg_bg(Color::Purple, Color::Default)       ) }
    else if name.ends_with(".mkv")  { return Some(Paint::with_fg_bg(Color::Purple, Color::Default).bold()) }
    else if name.ends_with(".avi")  { return Some(Paint::with_fg_bg(Color::Purple, Color::Default).bold()) }
    else if name.ends_with(".mp4")  { return Some(Paint::with_fg_bg(Color::Purple, Color::Default).bold()) }
    None
}
//-----------------------------------------------------------------------------
#[derive(Clone)]
struct Tab {
    name: String,
    context: Context,
}
//-----------------------------------------------------------------------------
type Coord = i32;
