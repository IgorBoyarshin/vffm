extern crate pancurses;
mod coloring;
use crate::coloring::*;

mod system;
use crate::system::*;

use std::collections::HashMap;
// use pancurses::{initscr, endwin, Input, noecho};
use pancurses::*;

use std::io;
use std::path::Path;
use std::fs::{self, DirEntry};

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
    system.get();
}
