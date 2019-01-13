use std::collections::HashMap;
use pancurses::*;

// pub static mut color_system: ColorSystem = ColorSystem {
//     // apparently previous ones are reserved for colors and so
//     // attributes conflict with them when invoked, so start with 8
//     next_colorid_to_use: 8,
//     next_paintid_to_use: 1,
//     colors: HashMap::new(),
//     paints: HashMap::new(),
// };


type ColorComponent = i16;
type RGB = (ColorComponent, ColorComponent, ColorComponent);
type ColorId = i16;
type PaintId = i16;


pub enum Attr {
    Bold, Underlined,
}

pub enum Mode {
    On, Off,
}

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub enum Color {
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
pub struct Paint {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub underlined: bool,
}

impl Paint {
    pub fn new(fg: Color, bg: Color) -> Paint {
        Paint {
            fg,
            bg,
            bold: false,
            underlined: false,
        }
    }

    pub fn new_full(fg: Color, bg: Color, bold: bool, underlined: bool) -> Paint {
        Paint {
            fg,
            bg,
            bold,
            underlined,
        }
    }
}

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
//-----------------------------------------------------------------------------
pub struct ColorSystem {
    next_colorid_to_use: ColorId,
    next_paintid_to_use: PaintId,

    colors: HashMap<Color, ColorId>,
    paints: HashMap<Paint, PaintId>,
}

impl ColorSystem {
    pub fn new() -> ColorSystem {
        ColorSystem {
            // apparently previous ones are reserved for colors and so
            // attributes conflict with them when invoked, so start with 8
            next_colorid_to_use: 8,
            next_paintid_to_use: 1,
            colors: HashMap::new(),
            paints: HashMap::new(),
        }
    }

    pub fn set_paint(&mut self, window: &Window, paint: Paint) {
        let paint_id = self.get_maybe_add_paint(paint);
        window.attron(ColorPair(paint_id as u8));
        if paint.bold {
            self.set_attr(&window, Attr::Bold, Mode::On);
        } else {
            self.set_attr(&window, Attr::Bold, Mode::Off);
        }
        if paint.underlined {
            self.set_attr(&window, Attr::Underlined, Mode::On);
        } else {
            self.set_attr(&window, Attr::Underlined, Mode::Off);
        }
    }

    pub fn set_attr(&mut self, window: &Window, attr: Attr, mode: Mode) {
        let attr = match attr {
            Attr::Bold       => A_BOLD,
            Attr::Underlined => A_UNDERLINE,
        };
        match mode {
            Mode::On =>  window.attron(attr),
            Mode::Off => window.attroff(attr),
        };
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
}
