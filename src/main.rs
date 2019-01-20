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

// use std::path::Path;
// use std::fs::{self};


#[derive(PartialEq, Eq)]
enum Mode {
    Input,
    AwaitingCommand,
}

type Combination = String;
enum Command {
    Terminate,
    GoTo(String),
    Up,
    Down,
    Left,
    Right,
    // NewTab,
    // CloseTab,
    // NextTab,
    // PreviousTab,
}

fn generate_possible_inputs() -> Vec<(Combination, Command)> {
    let mut inputs = Vec::new();
    inputs.push(("qwq".to_string(), Command::Terminate));
    inputs.push(("hh".to_string(), Command::Left));
    inputs.push(("j".to_string(), Command::Down));
    inputs.push(("k".to_string(), Command::Up));
    inputs.push(("l".to_string(), Command::Right));
    inputs
}

fn vec_of_refs<'a, T>(array: &'a Vec<T>) -> Vec<&'a T> {
    let mut vec = Vec::new();
    for entry in array { vec.push(entry); }
    vec
}

fn combinations_that_start_with<'a>(slice: &str, array: Vec<&'a(Combination, Command)>)
        -> Vec<&'a (Combination, Command)> {
    let mut combinations = Vec::new();
    let str1 = slice.as_bytes();
    let size = slice.len();
    'entries: for entry in array {
        let str2 = (&entry.0).as_bytes();
        if size > str2.len() { continue 'entries; }
        for i in 0..size {
            if str1[i] != str2[i] { continue 'entries; }
        }
        // Complete match up to the size => take it
        combinations.push(entry);
    }
    combinations
}


fn main() {
    let mut color_system = ColorSystem::new();
    let mut starting_path = absolute_pathbuf();
    starting_path.pop();
    starting_path.pop();
    starting_path.pop();
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
    let exact_match = |found_matches: &Vec<&(Combination, Command)>, current_input: &str| {
        (found_matches.len() == 1) &&
            (found_matches[0].0.len() == current_input.len())
    };

    let mut terminated = false;
    while !terminated {
        system.clear(&mut color_system);
        system.draw(&mut color_system);
        system.draw_command(&mut color_system, &current_input);

        if let Some(Input::Character(c)) = system.get() {
            if current_mode == Mode::AwaitingCommand {
                current_input.push(c);
                found_matches = combinations_that_start_with(&current_input, found_matches);

                // Display matches

                if exact_match(&found_matches, &current_input) {
                    let (_, command) = found_matches.pop().unwrap();
                    match command {
                        Command::Terminate => terminated = true,
                        Command::Up => system.up(),
                        Command::Down => system.down(),
                        Command::Left => system.left(),
                        Command::Right => system.right(),
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
