extern crate pancurses;
mod coloring;
use crate::coloring::*;

mod system;
use crate::system::*;

mod filesystem;
use crate::filesystem::*;

// use std::path::Path;
// use std::fs::{self};

fn main() {
    let mut color_system = ColorSystem::new();
    let mut starting_path = absolute_pathbuf();
    starting_path.pop();
    starting_path.pop();
    starting_path.pop();
    let system = System::new(
        Settings {
            columns_ratio: vec![2,3,2],
            dir_paint: Paint {fg: Color::Cyan, bg: Color::Black, bold: true, underlined: false},
            symlink_paint: Paint {fg: Color::Yellow, bg: Color::Black, bold: true, underlined: false},
            file_paint: Paint {fg: Color::White, bg: Color::Black, bold: false, underlined: false},
        },
        starting_path,
    );
    system.clear(&mut color_system);
    system.draw(&mut color_system);

    // let names = collect_dir(".").into_iter().map(|f| f.name).collect();
    // system.list_entries(&mut color_system, 1, collect_dir("."));
    system.get();
}
