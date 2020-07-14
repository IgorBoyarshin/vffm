use pancurses::{Window, initscr, start_color, use_default_colors, noecho,
    half_delay, endwin, curs_set, nocbreak};
use std::path::PathBuf;
use std::collections::{HashSet};
// use std::collections::{HashMap};

use crate::coloring::*;
use crate::utils::*;
use crate::right_column::*;
use crate::filesystem::*;
use crate::input::*;
use crate::input_mode::*;
use crate::direntry::*;
use crate::spawn::*;
use crate::drawing::*;
use crate::context::*;
use crate::tab::*;
use crate::notification::*;
//-----------------------------------------------------------------------------
pub struct Settings {
    pub paint_settings: PaintSettings,
    pub primary_paint: Paint,
    pub preview_paint: Paint,

    pub columns_ratio: Vec<u32>,
    pub scrolling_gap: usize,
    pub copy_done_notification_delay_ms: Millis,
}

//-----------------------------------------------------------------------------
pub struct System {
    // window: Window,
    settings: Settings,
    renderer: Renderer,

    sorting_type: SortingType,
    spawn_patterns: Vec<SpawnPattern>, // const

    notification: Option<Notification>,

    transfers: Vec<Transfer>,
    potential_transfer_data: Option<PotentialTransfer>,

    selected: Vec<PathBuf>,

    tabs: Vec<Tab>,
    current_tab_index: usize,

    show_hidden: bool,
}

impl System {
    pub fn new(settings: Settings, starting_path: PathBuf) -> Self {
        let window = System::setup();
        System::set_drawing_delay(DrawingDelay::Regular);

        let show_hidden = true; // TODO: move into Settings
        let selected = Vec::new();
        let sorting_type = SortingType::Lexicographically;
        let display_settings = DisplaySettings::generate(
            &window, settings.scrolling_gap, &settings.columns_ratio);
        let context = Context::generate(starting_path, &display_settings,
                               &settings.paint_settings, &sorting_type,
                               show_hidden, &selected);

        System {
            // window,
            settings,
            renderer: Renderer::new(window, display_settings),
            // display_settings,

            sorting_type,
            spawn_patterns: generate_spawn_patterns(),

            notification: None,      // for Transfers

            transfers: Vec::new(),
            potential_transfer_data: None,

            selected,

            tabs: vec![Tab { name: tab_name_from_path(&context.parent_path), context }],
            current_tab_index: 0,
            show_hidden,
        }
    }
//-----------------------------------------------------------------------------
    fn generate_context_for(&mut self, parent_path: PathBuf) -> Context {
        Context::generate(parent_path, &self.renderer.display_settings,
            &self.settings.paint_settings, &self.sorting_type, self.show_hidden, &self.selected)
    }

//-----------------------------------------------------------------------------
//-----------------------------------------------------------------------------
    fn collect_right_column_of_current(&self) -> RightColumn {
        let column_index = 2;
        let (begin, end) = self.renderer.display_settings.columns_coord[column_index];
        let column_width = (end - begin) as usize;
        let current_path = &self.context_ref().current_path;
        RightColumn::collect(current_path, &self.settings.paint_settings,
                             &self.sorting_type, self.show_hidden,
                             self.renderer.display_settings.column_effective_height,
                             column_width, &self.selected)
    }

//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
    fn current_contains(&self, name: &str) -> bool {
        for entry in self.context_ref().current_siblings.iter() {
            if entry.name == name { return true; }
        }
        false
    }

    fn cut(src: &str, dst: &str) {
        spawn_process_async("mv", vec![src, dst]);
    }

    fn yank(src: &str, dst: &str) {
        spawn_process_async("cp", vec!["-a", src, dst]);
        // Don't use rsync because it generates modified target names which makes
        // it impossible to track target's size until the transfer has finished.
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
            spawn_process_wait("rm", vec!["-r", "-f", path_to_str(path)]);
        } else if path.is_file() {
            spawn_process_wait("rm", vec!["-f", path_to_str(path)]);
        } else { // is symlink
            spawn_process_wait("unlink", vec![path_to_str(path)]);
        }
    }

    fn rename(path: &PathBuf, new_name: &str) {
        let new_path = path.parent().unwrap().join(new_name);
        spawn_process_wait("mv", vec![path_to_str(path), path_to_str(&new_path)]);
    }

    pub fn get_cumulative_size(&mut self) {
        if self.inside_empty_dir() { return }
        let size = cumulative_size(self.context_ref().current_path.as_ref().unwrap());
        self.context_mut().cumulative_size_text = Some("Size: ".to_string() + &human_size(size));
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.update();
    }
//-----------------------------------------------------------------------------
    fn maybe_sync_search_backup_selection_for_current_siblings(&mut self) {
        if self.doing_search() {
            for (name, selected) in self.context_ref().current_siblings.iter()
                    .map(|entry| (entry.name.clone(), entry.is_selected))
                    .collect::<Vec<(String, bool)>>() {
                self.sync_search_backup_selection_for(&name, selected);
            }
        }
    }

    fn sync_search_backup_selection_for(&mut self, name: &str, selected: bool) {
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
            execute_command_from(&self.context_ref().parent_path, text);
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
        // XXX: This is a TODO
        // Not currently implemented because be can only add/remove chars at the
        // end of the search query as an optimization
        // } else if let Some(InputMode::Search(SearchTools {cursor_index, ..})) = 
        //         self.context_mut().input_mode.as_mut() {
        //     if let Some(cursor_index) = cursor_index {
        //         if *cursor_index >= 1 {
        //             *cursor_index -= 1;
        //         }
        //     }
        // }
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
        // XXX: This is a TODO
        // Not currently implemented because be can only add/remove chars at the
        // end of the search query as an optimization
        }
        // } else if let Some(InputMode::Search(SearchTools {cursor_index, query, ..})) = 
        //         self.context_mut().input_mode.as_mut() {
        //     if let Some(cursor_index) = cursor_index {
        //         if *cursor_index + 1 <= query.len() { // allow one after end of text
        //             *cursor_index += 1;
        //         }
        //     }
        // }
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
        if pattern.is_empty() { return true; }
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
        self.current_tab_mut().name = tab_name_from_path(parent_path);
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
        if self.tabs.len() > 1 { self.update_current(); }
    }

    pub fn previous_tab(&mut self) {
        if self.current_tab_index == 0 {
            self.current_tab_index = self.tabs.len() - 1;
        } else {
            self.current_tab_index -= 1;
        }
        if self.tabs.len() > 1 { self.update_current(); }
    }
//-----------------------------------------------------------------------------
    pub fn sort_with(&mut self, new_sorting_type: SortingType) {
        self.sorting_type = new_sorting_type;
        self.update_current();
    }

//-----------------------------------------------------------------------------
    fn get_current_permissions(&mut self) -> Option<String> {
        string_permissions_for_entry(&self.current_entry_ref())
    }

    fn get_additional_entry_info_for_current(&self) -> Option<String> {
        let context = self.context_ref();
        get_additional_entry_info(self.current_entry_ref(), &context.current_path)
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

//-----------------------------------------------------------------------------
    // Update all, affectively reloading everything
    fn update(&mut self) {
        let parent_path = self.context_ref().parent_path.clone();
        self.current_tab_mut().context = self.generate_context_for(parent_path);
        self.update_current_tab_name();
    }

    // Update central column and right column
    pub fn update_current(&mut self) {
        let new_siblings = self.collect_sorted_children_of_parent();
        if let Some(InputMode::Search(search_tools)) = self.context_ref().input_mode.as_ref() {
            let pattern = search_tools.query.clone();
            self.context_mut().current_siblings = System::collect_entries_that_match(
                &new_siblings, &pattern);
        }
        if let Some(InputMode::Search(search_tools)) = self.context_mut().input_mode.as_mut() {
            search_tools.current_siblings_backup = new_siblings;
        } else {
            self.context_mut().current_siblings = new_siblings;
        }
        self.update_current_without_siblings();
    }

    pub fn update_current_without_siblings(&mut self) {
        let len = self.context_ref().current_siblings.len();
        self.context_mut().current_index =
            if self.context_ref().current_siblings.is_empty() { 0 } // reset for future
            else if self.context_ref().current_index >= len   { len - 1 } // update to valid
            else                                              { self.context_ref().current_index }; // leave old
        self.context_mut().current_path = path_of_nth_entry_inside(
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

    fn update_notification(&mut self) {
        if let Some(notification) = self.notification.as_ref() {
            if notification.has_finished() {
                self.notification = None;
            }
        }
    }
//-----------------------------------------------------------------------------
    fn collect_sorted_siblings_of_parent(&self) -> Vec<DirEntry> {
        let grandparent = maybe_parent(&self.context_ref().parent_path);
        into_sorted_direntries(
            collect_siblings_of(&self.context_ref().parent_path, self.show_hidden),
            &self.settings.paint_settings, &self.sorting_type,
            &self.selected, grandparent.as_ref())
    }

    fn collect_sorted_children_of_parent(&self) -> Vec<DirEntry> {
        into_sorted_direntries(
            collect_maybe_dir(&self.context_ref().parent_path, None, self.show_hidden),
            &self.settings.paint_settings, &self.sorting_type,
            &self.selected, Some(&self.context_ref().parent_path)) // TODO: CHECK
    }

    fn recalculate_parent_siblings_shift(&mut self) -> usize {
        siblings_shift_for(
            self.renderer.display_settings.scrolling_gap,
            self.renderer.display_settings.column_effective_height,
            self.context_ref().parent_index,
            self.context_ref().parent_siblings.len(),
            None)
    }

    fn recalculate_current_siblings_shift(&mut self) -> usize {
        siblings_shift_for(
            self.renderer.display_settings.scrolling_gap,
            self.renderer.display_settings.column_effective_height,
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

            self.context_mut().parent_index = index_of_entry_inside(
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
            self.context_mut().current_path = path_of_nth_entry_inside(
                0, &current_path, self.context_ref().right_column.siblings_ref().unwrap());

            self.context_mut().parent_index = self.context_ref().current_index;
            self.context_mut().current_index = 0;
            self.context_mut().parent_siblings_shift = self.context_ref().current_siblings_shift;
            self.context_mut().current_siblings_shift = 0;
            self.common_left_right();
        } else { // Resolved path points to a file
            // Try to open with default app
            let path = maybe_resolve_symlink_recursively(&current_path);
            if let Some((app, args, is_external)) = spawn_rule_for(&path, &self.spawn_patterns) {
                spawn_program(&app, args, is_external);
                self.update_current();
                self.renderer.invalidate(); // Otherwise the screen is not restored correctly
            }
        }
    }

    pub fn goto(&mut self, path: &str) {
        self.context_mut().parent_path = PathBuf::from(path);
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
    pub fn draw(&mut self, mut cs: &mut ColorSystem) {
        self.renderer.clear(&mut cs, self.settings.primary_paint);

        self.update_transfer_progress();
        self.update_notification();

        self.renderer.draw_borders(&mut cs, self.settings.primary_paint);
        self.renderer.draw_left_column(&mut cs, &self.context_ref().parent_siblings,
            self.context_ref().parent_index, self.context_ref().parent_siblings_shift);
        self.renderer.draw_middle_column(&mut cs, self.inside_empty_dir(),
            &self.context_ref().current_siblings,
            self.context_ref().current_index,
            self.context_ref().current_siblings_shift);
        self.renderer.draw_right_column(&mut cs, &self.context_ref().right_column,
            self.settings.preview_paint);

        let mut bottom_bar = Bar::with_y_and_width(
            self.renderer.display_settings.height - 1, self.renderer.display_settings.width);
        self.renderer.maybe_draw_input_mode(&mut cs, &mut bottom_bar, &self.context_ref().input_mode);
        self.renderer.draw_current_permission(&mut cs, &mut bottom_bar,
            &self.context_ref().current_permissions);
        self.renderer.draw_current_size(&mut cs, &mut bottom_bar,
            self.current_entry_ref().map(|e| e.size));
        self.renderer.maybe_draw_additional_info_for_current(&mut cs, &mut bottom_bar,
            &self.context_ref().additional_entry_info);
        self.renderer.draw_current_dir_siblings_count(&mut cs, &mut bottom_bar,
            &self.context_ref().current_siblings);
        self.renderer.draw_cumulative_size_text(&mut cs, &mut bottom_bar,
            &self.context_ref().cumulative_size_text);
        self.renderer.draw_notification(&mut cs, &mut bottom_bar, &self.notification);
        self.renderer.maybe_draw_selection_warning(&mut cs, &mut bottom_bar, self.selected.is_empty());

        let mut top_bar = Bar::with_y_and_width(0, self.renderer.display_settings.width);
        self.renderer.draw_current_path(&mut cs, &mut top_bar, self.inside_empty_dir(),
            &self.context_ref().parent_path, &self.context_ref().current_path);
        self.renderer.draw_tabs(&mut cs, &mut top_bar, &self.tabs, self.current_tab_index);

        self.renderer.maybe_draw_input_mode_cursor(&self.context_ref().input_mode);

        self.renderer.refresh();
    }

    pub fn draw_available_matches(&self, cs: &mut ColorSystem,
            matches: &Vec<Match>, completion_count: usize) {
        self.renderer.draw_available_matches(cs, matches, completion_count);
    }
//-----------------------------------------------------------------------------
    pub fn select_under_cursor(&mut self) {
        if let Some(path) = self.context_ref().current_path.as_ref() {
            if let Some(index) = self.selected.iter().position(|item| item == path) {
                // Unselect
                self.selected.remove(index);
                assert!(self.unsafe_current_entry_mut().is_selected);
                self.unsafe_current_entry_mut().is_selected = false;
            } else {
                // Select
                let path = path.clone();
                self.selected.push(path);
                assert!(!self.unsafe_current_entry_mut().is_selected);
                self.unsafe_current_entry_mut().is_selected = true;
            }

            if self.doing_search() {
                let selected = self.unsafe_current_entry_ref().is_selected;
                let name = self.unsafe_current_entry_ref().name.clone();
                self.sync_search_backup_selection_for(&name, selected);
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
//-----------------------------------------------------------------------------
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

    pub fn resize(&mut self) {
        self.renderer.display_settings = DisplaySettings::generate(
            &self.renderer.window, self.settings.scrolling_gap, &self.settings.columns_ratio);
        self.context_mut().right_column = self.collect_right_column_of_current();
        self.context_mut().parent_siblings_shift = siblings_shift_for(
            self.renderer.display_settings.scrolling_gap,
            self.renderer.display_settings.column_effective_height,
            self.context_ref().parent_index, self.context_ref().parent_siblings.len(), None);
        self.context_mut().current_siblings_shift = siblings_shift_for(
            self.renderer.display_settings.scrolling_gap,
            self.renderer.display_settings.column_effective_height,
            self.context_ref().current_index, self.context_ref().current_siblings.len(), None);
    }

    pub fn get(&self) -> Option<Input> {
        use pancurses::Input as PInput;
        match self.renderer.getch() {
            Some(PInput::Character('\t'))   => Some(Input::Tab),
            Some(PInput::Character('\x1B')) => Some(Input::Escape), // \e === \x1B
            Some(PInput::Character('\x7f')) => Some(Input::Backspace),
            Some(PInput::KeyBackspace)      => Some(Input::Backspace),
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
}

impl Drop for System {
    fn drop(&mut self) {
        // ColorSystem::finalize(&self.renderer.window);
        endwin();
        // spawn_process_wait("tput", vec!["reset"]);
        // std::process::Command::new("tput").args(vec!["reset"])
            // .stderr(Stdio::null()).stdout(Stdio::null())
            // .status().expect("failed to execute process");
        println!("Done");
    }
}
//-----------------------------------------------------------------------------

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
