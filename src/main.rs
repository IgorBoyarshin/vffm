extern crate pancurses;

use std::collections::HashMap;
use pancurses::{initscr, endwin, Input, noecho};
use pancurses::*;

use std::io;
use std::path::Path;
use std::fs::{self, DirEntry};

struct Settings {
    columns_ratio: Vec<u8>,
}

struct Tab {
    columns: Vec<Column>,
}

struct Column {

}


#[derive(PartialEq, Eq, Hash, Copy, Clone)]
enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Purple,
    Cyan,
    White,
    RGB(i16, i16, i16),
}

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
struct Paint {
    fg: Color,
    bg: Color,
}

type ColorComponent = i16;
type RGB = (ColorComponent, ColorComponent, ColorComponent);
type ColorId = i16;
type PaintId = i16;
type Coord = i32;

fn get_rgb(color: Color) -> RGB {
    match color {
        Color::RGB(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (1000, 0, 0),
        Color::Green => (0, 1000, 0),
        Color::Yellow => (1000, 1000, 0),
        Color::Blue => (0, 0, 1000),
        Color::Purple => (500, 0, 500),
        Color::Cyan => (0, 1000, 1000),
        Color::White => (1000, 1000, 1000),
    }
}

enum Attr {
    Bold, Underlined,
}

enum Mode {
    On, Off,
}

struct System {
    window: Window,

    next_colorid_to_use: ColorId,
    next_paintid_to_use: PaintId,

    colors: HashMap<Color, ColorId>,
    paints: HashMap<Paint, PaintId>,
    height: i32,
    width: i32,

    primary_paint: Paint,
}

impl System {
    fn new() -> Self {
        let window = System::setup();
        let (height, width) = System::get_height_width(&window);
        System {
            window,
            // apparently previous ones are reserved for colors and so
            // attributes conflict with them when invoked, so start with 8
            next_colorid_to_use: 8,
            next_paintid_to_use: 1,
            colors: HashMap::new(),
            paints: HashMap::new(),
            height: height,
            width: width,
            primary_paint: Paint{fg: Color::White, bg: Color::Black},
        }
    }

    fn clear(&mut self) {
        self.set_paint(self.primary_paint);
        for y in 0..self.height {
            self.window.mv(y, 0);
            self.window.hline(' ', self.width);
        }
        self.window.refresh();
    }

    fn draw(&mut self) {
        self.draw_borders();
        self.draw_column(4);
        self.draw_column(5);
        self.window.refresh();
    }

    fn draw_column(&mut self, x: Coord) {
        self.set_paint(self.primary_paint);
        self.window.mv(0, x);
        self.window.addch(ACS_TTEE());
        self.window.mv(1, x);
        self.window.vline(ACS_VLINE(), self.height-2);
        self.window.mv(self.height-1, x);
        self.window.addch(ACS_BTEE());
    }

    fn draw_borders(&mut self) {
        self.set_paint(self.primary_paint);

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

    fn get_maybe_add_paint(&mut self, paint: Paint) -> PaintId {
        if !self.paints.contains_key(&paint) {
            self.paints.insert(paint, self.next_paintid_to_use);
            let fg = self.get_maybe_add_color(paint.fg);
            let bg = self.get_maybe_add_color(paint.bg);
            init_pair(self.next_paintid_to_use, fg, bg);
            self.next_paintid_to_use += 1;
        }
        *self.paints.get(&paint).unwrap()
    }

    fn get_maybe_add_color(&mut self, color: Color) -> ColorId {
        if !self.colors.contains_key(&color) {
            self.colors.insert(color, self.next_colorid_to_use);
            let (r, g, b) = get_rgb(color);
            init_color(self.next_colorid_to_use, r, g, b);
            self.next_colorid_to_use += 1;
        }
        *self.colors.get(&color).unwrap()
    }

    fn set_paint(&mut self, paint: Paint) {
        let paint_id = self.get_maybe_add_paint(paint);
        self.window.attron(ColorPair(paint_id as u8));
    }

    fn set_attr(&mut self, attr: Attr, mode: Mode) {
        let attr = match attr {
            Attr::Bold       => A_BOLD,
            Attr::Underlined => A_UNDERLINE,
        };
        match mode {
            Mode::On => self.window.attron(attr),
            Mode::Off => self.window.attroff(attr),
        };
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
        for i in 0..ident {
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

    let mut system = System::new();
    system.clear();
    system.draw();
    // system.set_paint(Paint{fg: Color::Blue, bg: Color::Cyan});
    // system.window.printw("yeap");
    system.window.getch();
}
