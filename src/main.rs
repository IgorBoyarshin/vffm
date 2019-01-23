use std::time::Duration;
use std::thread;
// use std::collections::HashMap;

extern crate pancurses;
use pancurses::Input;

mod coloring;
use crate::coloring::*;

mod system;
use crate::system::*;

mod filesystem;
use crate::filesystem::*;

mod input;
use crate::input::*;

use std::path::PathBuf;
use std::fs::{self};


#[derive(PartialEq, Eq)]
enum Mode {
    Input,
    AwaitingCommand,
}

fn vec_of_refs<'a, T>(array: &'a Vec<T>) -> Vec<&'a T> {
    let mut vec = Vec::new();
    for entry in array { vec.push(entry); }
    vec
}


fn main() {
    let mut color_system = ColorSystem::new();
    let mut starting_path = PathBuf::from("/home/igorek/Stuff");
    // let mut starting_path = PathBuf::from("/home/igorek/.config/google-chrome");
    // let mut starting_path = absolute_pathbuf();
    // starting_path.pop();
    // starting_path.pop();
    // starting_path.pop();
    let mut system = System::new(
        Settings {
            columns_ratio: vec![2,3,2],
            dir_paint: Paint {fg: Color::Cyan, bg: Color::Black, bold: true, underlined: false},
            symlink_paint: Paint {fg: Color::Yellow, bg: Color::Black, bold: true, underlined: false},
            file_paint: Paint {fg: Color::White, bg: Color::Black, bold: false, underlined: false},
            unknown_paint: Paint {fg: Color::Grey, bg: Color::White, bold: true, underlined: false},
            cursor_vertical_gap: 4,
        },
        starting_path,
    );

    let current_mode = Mode::AwaitingCommand;
    let possible_inputs = generate_possible_inputs();
    let mut current_input = String::new();
    let mut found_matches = vec_of_refs(&possible_inputs);
    let exact_match = |found_matches: &Matches, current_input: &str| {
        (found_matches.len() == 1) &&
            (found_matches[0].0.len() == current_input.len())
    };

    let mut terminated = false;
    while !terminated {
        system.clear(&mut color_system);
        system.draw(&mut color_system);
        if current_input.len() > 0 && found_matches.len() >= 1 {
            // If the user is trying some input and there matches
            system.draw_available_matches(&mut color_system, &found_matches, current_input.len());
        }

        if let Some(Input::Character(c)) = system.get() {
            if current_mode == Mode::AwaitingCommand {
                current_input.push(c);
                found_matches = combinations_that_start_with(&current_input, found_matches);
                if exact_match(&found_matches, &current_input) {
                    let (_, command) = found_matches.pop().unwrap();
                    match command {
                        Command::Terminate => terminated = true,
                        Command::Up => system.up(),
                        Command::Down => system.down(),
                        Command::Left => system.left(),
                        Command::Right => system.right(),
                        Command::Sort(sorting_type) => system.change_sorting_type(*sorting_type),
                        Command::GoTo(_path) => {},
                    }
                }

                if found_matches.len() == 0 { // done with current command
                    // Reset for future commands
                    current_input.clear();
                    found_matches = vec_of_refs(&possible_inputs);
                }
            } else if current_mode == Mode::Input {}
        }

        thread::sleep_ms(10);
    };
}
