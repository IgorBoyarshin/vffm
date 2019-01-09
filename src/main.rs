extern crate pancurses;

use std::collections::HashMap;
use pancurses::{initscr, endwin, Input, noecho};
use pancurses::*;

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

struct System {
    window: Window,

    next_colorid_to_use: ColorId,
    next_paintid_to_use: PaintId,

    colors: HashMap<Color, ColorId>,
    paints: HashMap<Paint, PaintId>,
}

impl System {
    fn new(window: Window) -> Self {
        System {
            window,
            next_colorid_to_use: 1, // for whatever reason library count from 1
            next_paintid_to_use: 1,
            colors: HashMap::new(),
            paints: HashMap::new(),
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
}


fn setup() -> Window {
    let window = initscr();
    window.refresh();
    window.keypad(true);
    start_color();
    noecho();

    window
}

fn main() {
    println!("working");
    let mut system = System::new(setup());
    system.set_paint(Paint{fg: Color::Green, bg: Color::Cyan});
    system.window.printw("yeap");
    system.set_paint(Paint{fg: Color::Red, bg: Color::White});
    system.window.printw("nope");

    system.window.refresh();
    system.window.getch();

    endwin();
}
