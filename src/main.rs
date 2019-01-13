extern crate pancurses;mod coloring;
use crate::coloring::*;

use std::collections::HashMap;
// use pancurses::{initscr, endwin, Input, noecho};
use pancurses::*;

use std::io;
use std::path::Path;
use std::fs::{self, DirEntry};

struct Settings {
    columns_ratio: Vec<u32>,
}

// struct Tab {
//     columns: Vec<Column>,
// }

// struct Column {
//     x_start: Coord,
//     x_end: Coord,
//     height: Coord,
// }

// impl Column {
//     fn draw(color_system: &mut ColorSystem, window: &Window, paint: Paint) {
//         color_system.set_paint(&window, paint);
//         window.mv(0, x);
//         window.addch(ACS_TTEE());
//         window.mv(1, x);
//         window.vline(ACS_VLINE(), self.height-2);
//         window.mv(self.height-1, x);
//         window.addch(ACS_BTEE());
//     }
// }


type Coord = i32;


struct System {
    window: Window,

    height: Coord,
    width: Coord,

    primary_paint: Paint,
    columns_count: u32,
    columns_coord: Vec<(Coord, Coord)>,
}

impl System {
    fn new(settings: Settings) -> Self {
        let window = System::setup();
        let (height, width) = System::get_height_width(&window);
        System {
            window,
            height: height,
            width: width,
            primary_paint: Paint{fg: Color::White, bg: Color::Black},
            columns_count: settings.columns_ratio.len() as u32,
            columns_coord: System::positions_from_ratio(settings.columns_ratio, width),
        }
    }

    fn positions_from_ratio(ratio: Vec<u32>, width: Coord) -> Vec<(Coord, Coord)> {
        let width = width as f32;
        let sum = ratio.iter().sum::<u32>() as f32;
        let mut pos: Coord = 0;
        let mut positions: Vec<(Coord, Coord)> = Vec::new();
        for r in ratio.into_iter() {
            let weight: Coord = ((r as f32 / sum) * width) as Coord;
            positions.push((pos, pos + weight));
            pos += weight + 1;
        }
        positions
    }

    fn clear(&self, color_system: &mut ColorSystem) {
        color_system.set_paint(&self.window, self.primary_paint);
        for y in 0..self.height {
            self.window.mv(y, 0);
            self.window.hline(' ', self.width);
        }
        self.window.refresh();
    }

    fn draw(&self, mut color_system: &mut ColorSystem) {
        self.draw_borders(&mut color_system);
        for (start, _end) in self.columns_coord.iter().skip(1) {
            self.draw_column(color_system, *start);
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

enum UserInput {
    Quit,
    Left,
    Right,
    Up,
    Down,
}

fn process_input(c: char) -> Option<UserInput> {
    match c {
        'q' => Some(UserInput::Quit),
        'h' => Some(UserInput::Left),
        'j' => Some(UserInput::Down),
        'k' => Some(UserInput::Up),
        'l' => Some(UserInput::Right),
        _   => None
    }
}


fn traverse_path(path: &Path, ident: u8) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let p = entry?;
        let name = p.file_name();
        if name == "debug" || name == ".git" {
            continue;
        }
        for _ in 0..ident {
            print!("|  ");
        }
        print!("*");
        // if p.file_type()?.is_dir() {
        //     print!("*");
        // }
        println!("{:?}", p.file_name());
        traverse_path(&p.path(), ident + 1);
        // println!("{:?}", p.path());
    }

    Ok(())
}

fn traverse(path: &str) -> io::Result<()> {
    traverse_path(Path::new(path), 0)
}


fn main() {
    // println!("started");
    // traverse("../..");

    let mut color_system = ColorSystem::new();
    let system = System::new(
        Settings {
            columns_ratio: vec![1,3,1],
        }
    );
    system.clear(&mut color_system);
    system.draw(&mut color_system);
    // system.set_paint(Paint{fg: Color::Blue, bg: Color::Cyan});
    // system.window.printw("yeap");
    // while system.window.getch().unwrap_or('w') != 'q' {}
    system.window.getch();
}
