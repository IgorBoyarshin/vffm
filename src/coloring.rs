use std::collections::HashMap;
use pancurses::{Window, init_pair, init_color, A_BOLD, A_UNDERLINE, ColorPair};
use crate::filesystem::{EntryType};


type ColorComponent = i16;
type RGB = (ColorComponent, ColorComponent, ColorComponent);
type ColorId = i16;
type PaintId = i16;

pub struct PaintSettings {
    pub dir_paint: Paint,
    pub symlink_paint: Paint,
    pub file_paint: Paint,
    pub unknown_paint: Paint,
    pub executable_paint: Paint,
}

pub fn maybe_selected_paint_from(paint: Paint, convert: bool) -> Paint {
    if convert {
        let Paint {fg, mut bg, bold: _, underlined} = paint;
        if bg == Color::Default { bg = Color:: Black; }
        Paint {fg: bg, bg: fg, bold: true, underlined}
    } else { paint }
}

pub fn paint_for(entrytype: &EntryType, name: &str,
        executable: bool, paint_settings: &PaintSettings) -> Paint {
    match entrytype {
        EntryType::Directory => paint_settings.dir_paint,
        EntryType::Symlink   => paint_settings.symlink_paint,
        EntryType::Unknown   => paint_settings.unknown_paint,
        EntryType::Regular   =>
            if let Some(paint) = maybe_paint_for_name(name) { paint }
            else if executable { paint_settings.executable_paint }
            else               { paint_settings.file_paint },
    }
}

fn maybe_paint_for_name(name: &str) -> Option<Paint> {
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
    LightBlue,
    Yellow,
    Blue,
    Purple,
    Cyan,
    White,
    Grey,
    Default, // Transparent
    // RGB(i16, i16, i16),
}

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct Paint {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub underlined: bool,
}

impl Paint {
    pub fn with_fg_bg(fg: Color, bg: Color) -> Paint {
        Paint {
            fg,
            bg,
            bold: false,
            underlined: false,
        }
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    // pub fn underlined(mut self) -> Self {
    //     self.underlined = true;
    //     self
    // }
}

fn get_rgb(color: Color) -> RGB {
    match color {
        // Color::RGB(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (1000, 0, 0),
        Color::Green => (0, 1000, 0),
        Color::LightBlue => (433, 735, 966),
        Color::Yellow => (1000, 1000, 0),
        Color::Blue => (0, 0, 1000),
        Color::Purple => (850, 0, 750),
        Color::Cyan => (0, 1000, 1000),
        Color::White => (1000, 1000, 1000),
        Color::Grey => (400, 400, 400),
        Color::Default => panic!("Color::Default not handled in get_maybe_add_color()"),
    }
}

pub struct ColorSystem {
    next_colorid_to_use: ColorId,
    next_paintid_to_use: PaintId,

    colors: HashMap<Color, ColorId>,
    paints: HashMap<Paint, PaintId>,
}

impl ColorSystem {
    pub fn new() -> ColorSystem {
        ColorSystem {
            // apparently [0..7] are reserved for default colors and so
            // attributes conflict with them when invoked, so start with 8
            next_colorid_to_use: 8,
            next_paintid_to_use: 1,
            colors: HashMap::new(),
            paints: HashMap::new(),
        }
    }

    // pub fn finalize(window: &Window) {
    //     const ID: u8 = 1; // don't care about id since presumably not gonna be using them anymore
    //     init_pair(ID as i16, -1, -1);
    //     window.attron(ColorPair(ID));
    // }

    pub fn set_paint(&mut self, window: &Window, paint: Paint) {
        let paint_id = self.get_maybe_add_paint(paint);
        window.attron(ColorPair(paint_id as u8));

        if paint.bold { self.set_attr(&window, Attr::Bold, Mode::On);
        } else        { self.set_attr(&window, Attr::Bold, Mode::Off); }

        if paint.underlined { self.set_attr(&window, Attr::Underlined, Mode::On);
        } else              { self.set_attr(&window, Attr::Underlined, Mode::Off); }
    }

    pub fn set_attr(&mut self, window: &Window, attr: Attr, mode: Mode) {
        let attr = match attr {
            Attr::Bold       => A_BOLD,
            Attr::Underlined => A_UNDERLINE,
        };
        match mode {
            Mode::On  =>  window.attron(attr),
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
        if color == Color::Default { return -1; }
        if !self.colors.contains_key(&color) {
            self.colors.insert(color, self.next_colorid_to_use);
            let (r, g, b) = get_rgb(color);
            init_color(self.next_colorid_to_use, r, g, b);
            self.next_colorid_to_use += 1;
        }
        *self.colors.get(&color).unwrap()
    }
}
