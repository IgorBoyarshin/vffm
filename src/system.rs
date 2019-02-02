use pancurses::*;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::collections::HashMap;
use std::ops::RangeBounds;
use std::ffi::OsStr;
use std::process::Child;
use std::io::Read;
use std::time::SystemTime;

use crate::coloring::*;
use crate::filesystem::*;
use crate::input::*;


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
pub struct Settings {
    pub primary_paint: Paint,
    pub dir_paint: Paint,
    pub symlink_paint: Paint,
    pub file_paint: Paint,
    pub unknown_paint: Paint,
    pub preview_paint: Paint,

    pub columns_ratio: Vec<u32>,
    pub scrolling_gap: usize,
    pub copy_done_notification_delay_ms: Millis,
}

struct DisplaySettings {
    height: Coord,
    width:  Coord,
    columns_coord: Vec<(Coord, Coord)>,

    scrolling_gap: usize, // const
    column_effective_height: usize, // const
    entries_display_begin: i32, // const
}

struct Context {
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
    additional_entry_info: Option<String>,
}
//-----------------------------------------------------------------------------
pub struct System {
    window: Window,
    settings: Settings,
    display_settings: DisplaySettings,

    context: Context,

    sorting_type: SortingType,
    spawn_patterns: Vec<SpawnPattern>, // const

    notification_text: Option<String>,
    notification: Option<Notification>,

    transfers: Vec<Transfer>,
    potential_transfer_data: Option<PotentialTransfer>,

    selected: Vec<PathBuf>,
}

impl System {
    pub fn new(settings: Settings, starting_path: PathBuf) -> Self {
        let window = System::setup();
        System::set_drawing_delay(DrawingDelay::Regular);

        let sorting_type = SortingType::Lexicographically;
        let display_settings = System::generate_display_settings(
            &window, settings.scrolling_gap, &settings.columns_ratio);
        let context = System::generate_context(starting_path, &display_settings, &sorting_type);

        System {
            window,
            settings,
            display_settings,
            context,

            sorting_type,
            spawn_patterns: System::generate_spawn_patterns(),

            notification_text: None, // for CumulativeSize
            notification: None,      // for Transfers

            transfers: Vec::new(),
            potential_transfer_data: None,

            selected: Vec::new(),
        }
    }
//-----------------------------------------------------------------------------
    fn generate_context_for(&mut self, path: PathBuf) -> Context {
        System::generate_context(path, &self.display_settings, &self.sorting_type)
    }

    fn generate_context(starting_path: PathBuf, display_settings: &DisplaySettings,
            sorting_type: &SortingType) -> Context {
        let current_siblings = System::sort(collect_maybe_dir(&starting_path, None), sorting_type);
        let parent_siblings = System::sort(collect_siblings_of(&starting_path), sorting_type);
        let first_entry_path =
            System::path_of_nth_entry_inside(0, &starting_path, &current_siblings);
        let first_entry_ref = System::nth_entry_inside(0, &current_siblings);
        let parent_index = System::index_of_entry_inside(&starting_path, &parent_siblings).unwrap();
        let current_index = 0;
        let column_index = 2;
        let (begin, end) = display_settings.columns_coord[column_index];
        let column_width = (end - begin) as usize;
        let right_column = System::collect_right_column(&first_entry_path,
                        &sorting_type, display_settings.column_effective_height, column_width);

        let parent_siblings_shift = System::siblings_shift_for(
                display_settings.scrolling_gap, display_settings.column_effective_height,
                parent_index, parent_siblings.len(), None);
        let current_siblings_shift = System::siblings_shift_for(
                display_settings.scrolling_gap, display_settings.column_effective_height,
                current_index, current_siblings.len(), None);
        Context {
            parent_index,
            current_index,
            parent_siblings,
            current_permissions: System::string_permissions_for_path(&first_entry_path),
            additional_entry_info: System::get_additional_entry_info(first_entry_ref,
                                                     &first_entry_path, &right_column),
            current_siblings,
            right_column,
            // additional_entry_info: System::get_symlink_target(&first_entry_path),
            current_path: first_entry_path,
            parent_path: starting_path,

            parent_siblings_shift,
            current_siblings_shift,
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
    fn index_of_entry_inside(path: &PathBuf, entries: &Vec<Entry>) -> Option<usize> {
        if is_root(path) { return Some(0); }
        let sought_name = path.file_name().unwrap().to_str().unwrap();
        for (index, Entry {name, ..}) in entries.iter().enumerate() {
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
        System::collect_right_column(&self.context.current_path, &self.sorting_type,
                                     self.display_settings.column_effective_height, column_width)
    }

    fn collect_right_column(path_opt: &Option<PathBuf>, sorting_type: &SortingType,
            max_height: usize, max_width: usize) -> RightColumn {
        if let Some(path) = path_opt {
            if path.is_dir() { // resolved path
                return RightColumn::with_siblings(
                    System::collect_sorted_children_of(path_opt, sorting_type, max_height));
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
        add_to_apps(&mut apps_extensions, "vlc @", vec!["mkv", "avi", "mp4", "mp3"], external);
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
        for entry in self.context.current_siblings.iter() {
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
                    self.context.parent_path.join(dst_name)
                }).collect();
            for (src_path, dst_path) in data.src_paths.iter().zip(dst_paths.iter()) {
                let mut src = path_to_string(src_path);
                if is_dir(src_path) { src += "/."; } // so that cp works as we want

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
            if let Some(path) = self.context.current_path.as_ref() {
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
            if let Some(path) = self.context.current_path.as_ref() {
                self.potential_transfer_data = Some(PotentialTransfer::cut(vec![path.clone()]));
            }
        } else {
            let selected = self.selected.drain(..).collect();
            self.potential_transfer_data = Some(PotentialTransfer::cut(selected));
        }
    }

    pub fn remove_selected(&mut self) {
        if self.selected.is_empty() {
            if let Some(path) = self.context.current_path.as_ref() {
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

    pub fn get_cumulative_size(&mut self) {
        if self.inside_empty_dir() { return }
        let size = cumulative_size(self.context.current_path.as_ref().unwrap());
        self.notification_text = Some("Size: ".to_string() + &System::human_size(size));
    }
//-----------------------------------------------------------------------------
    pub fn sort_with(&mut self, new_sorting_type: SortingType) {
        self.sorting_type = new_sorting_type;
        self.update_current();
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
        System::string_permissions_for_path(&self.context.current_path)
    }

    fn get_additional_entry_info_for_current(&self) -> Option<String> {
        System::get_additional_entry_info(self.current_entry_ref(),
                        &self.context.current_path, &self.context.right_column)
    }

    fn get_additional_entry_info(entry: Option<&Entry>, path: &Option<PathBuf>,
                                 right_column: &RightColumn) -> Option<String> {
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

    // fn get_symlink_target_for_current(&self) -> Option<String> {
    //     System::get_symlink_target(&self.context.current_path)
    // }

    fn current_entry_ref(&self) -> Option<&Entry> {
        if self.context.current_path.is_some() { Some(self.unsafe_current_entry_ref()) }
        else                                   { None }
    }

    fn unsafe_current_entry_ref(&self) -> &Entry {
        self.context.current_siblings.get(self.context.current_index).unwrap()
    }

    fn unsafe_current_entry_mut(&mut self) -> &mut Entry {
        self.context.current_siblings.get_mut(self.context.current_index).unwrap()
    }

    // fn current_is_dir(&self) -> bool {
    //     if self.inside_empty_dir() { return false; }
    //     self.unsafe_current_entry_ref().is_dir()
    // }

    fn inside_empty_dir(&self) -> bool {
        self.context.current_path.is_none()
    }

    fn path_of_nth_entry_inside(n: usize, path: &PathBuf, entries: &Vec<Entry>) -> Option<PathBuf> {
        if entries.is_empty() { return None; }
        if n >= entries.len() { return None; }
        let name = entries[n].name.clone();
        let mut path = path.clone();
        path.push(name);
        Some(path)
    }

    fn nth_entry_inside(n: usize, entries: &Vec<Entry>) -> Option<&Entry> {
        if entries.is_empty() { return None; }
        if n >= entries.len() { return None; }
        Some(entries.get(n).unwrap())
    }

    // fn current_is_dir_or_symlink(&self) -> bool {
    //     if self.current_path.is_some() {
    //         System::is_dir_or_symlink(self.current_path.as_ref().unwrap())
    //     } else { false }
    // }

    // fn is_dir_or_symlink(path: &PathBuf) -> bool {
    //     !path.is_file()
    // }
//-----------------------------------------------------------------------------
    // Update all, affectively reloading everything
    fn update(&mut self) {
        if self.context.current_path.is_none() { return; }
        self.context = self.generate_context_for(self.context.current_path.as_ref().unwrap().clone());
    }

    // Update central column and right column
    pub fn update_current(&mut self) {
        self.context.current_siblings = self.collect_sorted_children_of_parent();
        let len = self.context.current_siblings.len();
        self.context.current_index =
            if self.context.current_siblings.is_empty() { 0 } // reset for future
            else if self.context.current_index >= len   { len - 1 } // update to valid
            else                                        { self.context.current_index }; // leave old
        self.context.current_path = System::path_of_nth_entry_inside(
            self.context.current_index, &self.context.parent_path, &self.context.current_siblings);
        self.context.right_column = self.collect_right_column_of_current();
        self.context.current_permissions = self.get_current_permissions();
        self.context.current_siblings_shift = self.recalculate_current_siblings_shift();
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
    fn collect_sorted_siblings_of_parent(&self) -> Vec<Entry> {
        System::sort(collect_siblings_of(&self.context.parent_path), &self.sorting_type)
    }

    // TODO: mb get rid of Option
    fn collect_sorted_children_of(path: &Option<PathBuf>, sorting_type: &SortingType,
                                  max_count: usize) -> Vec<Entry> {
        if let Some(path) = path {
            System::sort(collect_maybe_dir(&path, Some(max_count)), sorting_type)
        } else { Vec::new() }
    }

    // fn collect_sorted_children_of_current(&self) -> Vec<Entry> {
    //     System::collect_sorted_children_of(&self.current_path, &self.sorting_type)
    // }

    fn collect_sorted_children_of_parent(&self) -> Vec<Entry> {
        System::sort(collect_maybe_dir(&self.context.parent_path, None), &self.sorting_type)
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
            self.display_settings.scrolling_gap,
            self.display_settings.column_effective_height,
            self.context.parent_index,
            self.context.parent_siblings.len(),
            None)
    }

    fn recalculate_current_siblings_shift(&mut self) -> usize {
        System::siblings_shift_for(
            self.display_settings.scrolling_gap,
            self.display_settings.column_effective_height,
            self.context.current_index,
            self.context.current_siblings.len(),
            Some(self.context.current_siblings_shift))
    }

    fn update_current_entry_by_index(&mut self) {
        self.notification_text = None;
        self.update_last_part_of_current_path_by_index();
        self.context.current_permissions = self.get_current_permissions();
        self.context.right_column = self.collect_right_column_of_current();
        self.context.current_siblings_shift = self.recalculate_current_siblings_shift();
        self.context.additional_entry_info = self.get_additional_entry_info_for_current();
        if let Some(new_size) = maybe_size(self.context.current_path.as_ref().unwrap()) {
            self.unsafe_current_entry_mut().size = new_size;
        }
    }

    fn update_last_part_of_current_path_by_index(&mut self) {
        let name = self.unsafe_current_entry_ref().name.clone();
        self.context.current_path.as_mut().map(|path| {
            (*path).set_file_name(name); // try_pop + push(name)
        });
    }

    fn common_left_right(&mut self) {
        self.notification_text = None;
        self.context.current_permissions = self.get_current_permissions();
        self.context.right_column = self.collect_right_column_of_current();
        self.context.current_siblings = self.collect_sorted_children_of_parent();
        self.context.parent_siblings = self.collect_sorted_siblings_of_parent();
        self.context.additional_entry_info = self.get_additional_entry_info_for_current();
    }
//-----------------------------------------------------------------------------
    pub fn up(&mut self) {
        if self.inside_empty_dir() { return }
        if self.context.current_index > 0 {
            self.context.current_index -= 1;
            self.update_current_entry_by_index();
        }
    }

    pub fn down(&mut self) {
        if self.inside_empty_dir() { return }
        if self.context.current_index < self.context.current_siblings.len() - 1 {
            self.context.current_index += 1;
            self.update_current_entry_by_index();
        }
    }

    pub fn left(&mut self) {
        if !is_root(&self.context.parent_path) {
            if self.context.current_path.is_none() {
                self.context.current_path = Some(self.context.parent_path.clone());
            } else {
                self.context.current_path.as_mut().map(|path| path.pop());
            }
            self.context.parent_path.pop();

            self.context.current_index = self.context.parent_index;
            self.common_left_right();

            self.context.parent_index = System::index_of_entry_inside(
                &self.context.parent_path, &self.context.parent_siblings).unwrap();
            self.context.current_siblings_shift = self.context.parent_siblings_shift;
            self.context.parent_siblings_shift = self.recalculate_parent_siblings_shift();
        }
    }

    pub fn right(&mut self) {
        if self.inside_empty_dir() { return; }
        let current_path_ref = self.context.current_path.as_ref().unwrap();
        if current_path_ref.is_dir() { // Traverses symlinks. The resolved path points to a dir
            // Navigate inside
            // Deliberately use the not-resolved version, so the path contains the symlink
            self.context.parent_path = current_path_ref.to_path_buf();
            self.context.current_path = System::path_of_nth_entry_inside(
                0, current_path_ref, self.context.right_column.siblings_ref().unwrap());

            self.context.parent_index = self.context.current_index;
            self.context.current_index = 0;
            self.context.parent_siblings_shift = self.context.current_siblings_shift;
            self.context.current_siblings_shift = 0;
            self.common_left_right();
        } else { // Resolved path points to a file
            // Try to open with default app
            let path = maybe_resolve_symlink_recursively(current_path_ref);
            if let Some((app, args, is_external)) = self.spawn_rule_for(&path) {
                System::spawn_program(&app, args, is_external);
                self.update_current();
            }
        }
    }

    pub fn goto(&mut self, path: &str) {
        self.context.current_path = Some(PathBuf::from(path));
        self.notification_text = None;
        self.update();
    }
//-----------------------------------------------------------------------------
    fn list_entry(&self, cs: &mut ColorSystem, column_index: usize,
            y: usize, entry: &Entry, under_cursor: bool, selected: bool) {
        let paint = match entry.entrytype {
            EntryType::Regular => self.settings.file_paint,
            EntryType::Directory => self.settings.dir_paint,
            EntryType::Symlink => self.settings.symlink_paint,
            EntryType::Unknown => self.settings.unknown_paint,
        };
        let paint = System::maybe_selected_paint_from(paint, under_cursor);

        let y = y as Coord + self.display_settings.entries_display_begin;
        let (mut begin, end) = self.display_settings.columns_coord[column_index];
        if selected {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Red, Color::Default));
            self.window.mvaddch(y, begin + 1, ACS_CKBOARD());
            self.window.mvaddch(y, begin + 2, ' ');
            begin += 2;
        }
        let column_width = end - begin;
        let size = System::human_size(entry.size);
        let size_len = size.len();
        let name_len = System::chars_amount(&entry.name) as i32;
        let empty_space_length = column_width - name_len - size_len as i32;
        cs.set_paint(&self.window, paint);
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
            entries: &Vec<Entry>, cursor_index: Option<usize>, shift: usize, path_to_dir: Option<&PathBuf>) {
        for (index, entry) in entries.into_iter().enumerate()
                .skip(shift).take(self.display_settings.column_effective_height) {
            let under_cursor = match cursor_index {
                Some(i) => (i == index),
                None    => false,
            };
            let selected = if let Some(path_to_dir) = path_to_dir {
                self.is_selected_by_name(&entry.name, path_to_dir)
            } else { false };

            self.list_entry(&mut cs, column_index, index - shift, &entry, under_cursor, selected);
        }
    }

    fn is_selected_by_name(&self, name: &str, path: &PathBuf) -> bool {
        for item in self.selected.iter() {
            if item.ends_with(name) { // check partialy
                if path_to_str(item) == path_to_str(&path.join(name)) { // verify
                    return true;
                }
            }
        }

        false
    }

    fn maybe_index_of_selected(&self, path: &PathBuf) -> Option<usize> {
        for (index, item) in self.selected.iter().enumerate() {
            if path_to_str(item) == path_to_str(path) { return Some(index); }
        }
        None
    }

    pub fn select_under_cursor(&mut self) {
        if let Some(path) = self.context.current_path.as_ref() {
            if let Some(index) = self.maybe_index_of_selected(path) {
                self.selected.remove(index);
            } else {
                self.selected.push(path.clone());
            }
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.potential_transfer_data = None;
    }

    pub fn invert_selection(&self) {
        // TODO
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
        self.draw_current_permission(&mut cs, &mut bottom_bar);
        self.draw_current_size(&mut cs, &mut bottom_bar);
        self.maybe_draw_additional_info_for_current(&mut cs, &mut bottom_bar);
        self.draw_current_dir_siblings_count(&mut cs, &mut bottom_bar);
        self.draw_notification_text(&mut cs, &mut bottom_bar);
        self.update_and_draw_notification(&mut cs, &mut bottom_bar);
        self.maybe_draw_selection_warning(&mut cs, &mut bottom_bar);

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
        if path_to_str(&self.context.parent_path) == "/" {
            self.list_entries(&mut cs, column_index, &self.context.parent_siblings,
                  Some(self.context.parent_index), self.context.parent_siblings_shift,
                  None);
        } else {
            let grandparent = self.context.parent_path.parent().unwrap().to_path_buf();
            self.list_entries(&mut cs, column_index, &self.context.parent_siblings,
                  Some(self.context.parent_index), self.context.parent_siblings_shift,
                  Some(&grandparent));
        };
    }

    fn draw_middle_column(&self, mut cs: &mut ColorSystem) {
        let column_index = 1;
        if self.inside_empty_dir() {
            self.draw_empty_sign(&mut cs, column_index);
        } else {
            self.list_entries(&mut cs, column_index, &self.context.current_siblings,
              Some(self.context.current_index), self.context.current_siblings_shift,
              Some(&self.context.parent_path));
        }
    }

    fn draw_right_column(&self, mut cs: &mut ColorSystem) {
        let column_index = 2;
        if let Some(siblings) = self.context.right_column.siblings_ref() {
            // Have siblings (Some or None) => are sure to be in a dir or symlink
            if siblings.is_empty() {
                self.draw_empty_sign(&mut cs, column_index);
            } else {
                self.list_entries(&mut cs, column_index, siblings, None, 0,
                                  Some(self.context.current_path.as_ref().unwrap()));
            }
        } else if let Some(preview) = self.context.right_column.preview_ref() {
            let (begin, _) = self.display_settings.columns_coord[column_index];
            let y = self.display_settings.entries_display_begin;
            cs.set_paint(&self.window, self.settings.preview_paint);
            for (i, line) in preview.iter().enumerate() {
                mvprintw(&self.window, y + i as i32, begin + 1, line);
            }
        } // display nothing otherwise
    }

    fn draw_current_path(&self, cs: &mut ColorSystem) {
        if !self.inside_empty_dir() {
            let path = self.context.current_path.as_ref().unwrap().to_str().unwrap();
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            mvprintw(&self.window, 0, 0, path);
        }
    }

    fn draw_current_permission(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if self.context.current_path.is_some() {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            bar.draw_left(&self.window, &self.context.current_permissions, 2);
        }
    }

    fn draw_current_size(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if self.context.current_path.is_some() {
            let size = System::human_size(self.unsafe_current_entry_ref().size);
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::Blue, Color::Default));
            bar.draw_left(&self.window, &size, 2);
        }
    }

    fn maybe_draw_additional_info_for_current(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if let Some(info) = self.context.additional_entry_info.as_ref() {
            cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
            bar.draw_left(&self.window, &info, 2);
        }
    }

    fn draw_current_dir_siblings_count(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        let text = "Siblings = ".to_string() + &self.context.current_siblings.len().to_string();
        cs.set_paint(&self.window, Paint::with_fg_bg(Color::LightBlue, Color::Default));
        bar.draw_left(&self.window, &text, 2);
    }

    fn draw_notification_text(&self, cs: &mut ColorSystem, bar: &mut Bar) {
        if let Some(text) = self.notification_text.as_ref() {
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
//-----------------------------------------------------------------------------
    pub fn draw_available_matches(&self, cs: &mut ColorSystem,
            matches: &Vec<&Match>, completion_count: usize) {
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

    fn set_drawing_delay(drawing_delay: DrawingDelay) {
        half_delay(drawing_delay.ms() / 100); // expects argument in tens of a second
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
        self.context.right_column = self.collect_right_column_of_current();
        self.context.parent_siblings_shift = System::siblings_shift_for(
            self.display_settings.scrolling_gap, self.display_settings.column_effective_height,
            self.context.parent_index, self.context.parent_siblings.len(), None);
        self.context.current_siblings_shift = System::siblings_shift_for(
            self.display_settings.scrolling_gap, self.display_settings.column_effective_height,
            self.context.current_index, self.context.current_siblings.len(), None);
    }
}

impl Drop for System {
    fn drop(&mut self) {
        endwin();
        println!("Done");
    }
}
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
type Coord = i32;
