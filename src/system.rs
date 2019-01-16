use pancurses::*;

// mod coloring;
use crate::coloring::*;
use crate::filesystem::*;

use std::path::PathBuf;

type Coord = i32;

pub struct Settings {
    pub columns_ratio: Vec<u32>,
    pub dir_paint: Paint,
    pub symlink_paint: Paint,
    pub file_paint: Paint,
}

pub struct System {
    window: Window,

    height: Coord,
    width: Coord,

    primary_paint: Paint,
    columns_count: u32,
    columns_coord: Vec<(Coord, Coord)>,

    dir_paint: Paint,
    symlink_paint: Paint,
    file_paint: Paint,

    current_entry: Option<Entry>,
    current_siblings: Vec<Entry>,
    parent_siblings: Vec<Entry>,
    child_siblings: Vec<Entry>,

    current_path: Option<PathBuf>,
    parent_path: PathBuf,

    parent_index: usize,
    current_index: usize,
}

impl System {
    pub fn new(settings: Settings, starting_path: PathBuf) -> Self {
        let window = System::setup();
        let (height, width) = System::get_height_width(&window);
        let primary_paint =
            Paint{fg: Color::White, bg: Color::Black, bold: false, underlined: false};

        let first_entry = first_entry_inside(&starting_path);
        let first_entry_path = {
            if let Some(entry) = &first_entry {
                let name = entry.name.clone();
                let mut path = starting_path.clone();
                path.push(name);
                Some(path)
            } else { None }
        };

        System {
            window,
            height,
            width,
            primary_paint,
            dir_paint: settings.dir_paint,
            symlink_paint: settings.symlink_paint,
            file_paint: settings.file_paint,

            columns_count: settings.columns_ratio.len() as u32,
            columns_coord: System::positions_from_ratio(settings.columns_ratio, width),


            parent_index: index_inside(&starting_path),
            current_index: 0,

            current_siblings: collect_dir(&starting_path),
            parent_siblings: collect_siblings_of(&starting_path),
            child_siblings: System::collect_children(&first_entry_path),
            current_entry: first_entry,
            current_path: first_entry_path,

            parent_path: starting_path,
        }
    }

    fn collect_children(path: &Option<PathBuf>) -> Vec<Entry> {
        if let Some(path) = path {
            collect_dir(&path)
        } else { Vec::new() }
    }

    pub fn get(&self) -> Option<Input> {
        self.window.getch()
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

    // pub fn put_str(&self, cs: &mut ColorSystem, y: Coord, x: Coord, string: &str) {
    //     self.window.mvprintw(y, x, string);
    //     self.window.refresh();
    // }
    //
    // pub fn put_str_line(&self, cs: &mut ColorSystem, y: Coord, x: Coord, string: &str, length: i32) {
    //     self.window.mvprintw(y, x, string);
    //     let length = length - string.len() as i32;
    //     self.window.hline(' ', length);
    //     self.window.refresh();
    // }

    pub fn human_size(size: u64) -> String {
        if size < 1024 { return size.to_string() + " B"; }
        // Kilo
        let full = size / 1024;
        if full < 1024 {
            let mut string = full.to_string();
            let remainder = size % 1024;
            if remainder != 0 {
                string += ".";
                string += &(remainder * 10 / 1024).to_string();
            }

            return string + " K";
        }
        // Mega
        let size = size / 1024;
        let full = size / 1024;
        if full < 1024 {
            let mut string = full.to_string();
            let remainder = size % 1024;
            if remainder != 0 {
                string += ".";
                string += &(remainder * 10 / 1024).to_string();
            }

            return string + " M";
        }
        // Giga
        let size = size / 1024;
        let full = size / 1024;
        if full < 1024 {
            let mut string = full.to_string();
            let remainder = size % 1024;
            if remainder != 0 {
                string += ".";
                string += &(remainder * 10 / 1024).to_string();
            }

            return string + " G";
        }

        "<>".to_string()
    }

    fn list_entry(&self, cs: &mut ColorSystem, column_index: usize,
            entry_index: usize, entry: &Entry, selected: bool) {
        let paint = match entry.entrytype {
            EntryType::Regular => self.file_paint,
            EntryType::Directory => self.dir_paint,
            EntryType::Symlink => self.symlink_paint,
        };
        let paint = System::maybe_selected_paint_from(paint, selected);
        cs.set_paint(&self.window, paint);

        let (begin, end) = self.columns_coord[column_index];
        let column_width = end - begin;
        // let size = entry.size.to_string();
        let size = System::human_size(entry.size);
        let name_len = entry.name.len() as i32;
        let empty_space_length = column_width - name_len - size.len() as i32;
        let y = entry_index as Coord + 1;
        self.window.mvprintw(y, begin + 1, &entry.name);
        self.window.mv(y, begin + 1 + name_len);
        self.window.hline(' ', empty_space_length);
        self.window.mvprintw(y, begin + 1 + name_len + empty_space_length, size);
    }

    fn list_entries(&self, mut cs: &mut ColorSystem, column_index: usize,
            entries: &Vec<Entry>, selected_index: Option<usize>) {
        for (index, entry) in entries.into_iter().enumerate() {
            let selected = match selected_index {
                Some(i) => (i == index),
                None    => false,
            };
            self.list_entry(&mut cs, column_index, index, &entry, selected);
        }
        self.window.refresh();
    }

    fn maybe_selected_paint_from(paint: Paint, convert: bool) -> Paint {
        if convert {
            let Paint {fg, bg, bold: _, underlined} = paint;
            Paint {fg: bg, bg: fg, bold: true, underlined}
        } else {
            paint
        }
    }

    pub fn update_current_from_index(&mut self) {
        self.current_entry = Some(self.current_siblings[self.current_index].clone());
        let name = self.current_entry.clone().unwrap().name;
        self.current_path.as_mut().map(|path| {
            (*path).pop();
            (*path).push(name);
        });
        self.child_siblings = {
            if self.current_entry.as_ref().unwrap().entrytype == EntryType::Directory {
                if let Some(path) = &self.current_path {
                    collect_dir(&path)
                } else { Vec::new() }
            } else { Vec::new() }
        };
    }

    pub fn up(&mut self) {
        if self.current_entry.is_none() { return }
        if self.current_index > 0 {
            self.current_index -= 1;
            self.update_current_from_index();
        }
    }

    pub fn down(&mut self) {
        if self.current_entry.is_none() { return }
        if self.current_index < self.current_siblings.len() - 1 {
            self.current_index += 1;
            self.update_current_from_index();
        }
    }

    // current_entry: Option<Entry>,
    // current_siblings: Vec<Entry>,
    // parent_siblings: Vec<Entry>,
    // child_siblings: Vec<Entry>,
    pub fn left(&mut self) {
        // if !is_root(&self.parent_path) {
        //     self.current_path.as_mut().map(|path| path.pop());
        //     self.parent_path.pop();
        //
        //     self.current_index = self.parent_index;
        //     self.parent_index = index_inside(&self.parent_path);
        //
        //     self.child_siblings = self.current_siblings;
        // }
    }

    // pub fn fill_column(&self, index: usize, strings: Vec<String>) {
    //     let (begin, _end) = self.columns_coord[index];
    //     for (index, string) in strings.into_iter().enumerate() {
    //         self.put_str(index as Coord + 1, begin + 1, string.as_str());
    //     }
    //     self.window.refresh();
    // }

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
        self.list_entries(&mut cs, 0, &self.parent_siblings, Some(self.parent_index));
        // Current
        if self.current_siblings.is_empty() {
            // TODO: write <empty>
        } else {
            self.list_entries(&mut cs, 1, &self.current_siblings, Some(self.current_index));
        }
        // Next
        if self.current_entry.is_some() {
            self.list_entries(&mut cs, 2, &self.child_siblings, None);
        }

        if let Some(path) = self.current_path.clone() {
            cs.set_paint(&self.window, Paint{fg: Color::Red, bg: Color::Black, bold: true, underlined: false});
            self.window.mvprintw(20, 20, path.to_str().unwrap());
        }

        self.window.refresh();
    }

    fn draw_column(&self, color_system: &mut ColorSystem, x: Coord) {
        color_system.set_paint(&self.window, self.primary_paint);
        self.window.mv(0, x);
        self.window.addch(ACS_TTEE());
        self.window.mv(1, x);
        self.window.vline(ACS_VLINE(), self.height-2);
        self.window.mv(self.height-1, x);
        self.window.addch(ACS_BTEE());
    }

    fn draw_borders(&self, color_system: &mut ColorSystem) {
        color_system.set_paint(&self.window, self.primary_paint);

        self.window.mv(0, 0);
        self.window.addch(ACS_ULCORNER());
        self.window.hline(ACS_HLINE(), self.width-2);
        self.window.mv(0, self.width-1);
        self.window.addch(ACS_URCORNER());

        self.window.mv(self.height-1, 0);
        self.window.addch(ACS_LLCORNER());
        self.window.hline(ACS_HLINE(), self.width-2);
        self.window.mv(self.height-1, self.width-1);
        self.window.addch(ACS_LRCORNER());

        for y in 1..self.height-1 {
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
}

impl Drop for System {
    fn drop(&mut self) {
        endwin();
        println!("Done");
    }
}
