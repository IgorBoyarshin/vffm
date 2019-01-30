use std::{thread, time};

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


#[derive(PartialEq, Eq)]
enum Mode {
    Input,
    AwaitingCommand,
}

struct Overseer {
    color_system: ColorSystem,
    system: System,

    mode: Mode,
    current_input: String,

    possible_inputs: Vec<(Combination, Command)>, // const
    terminated: bool,
}

impl Overseer {
    fn init_system(starting_path: PathBuf) -> System {
        System::new(
            Settings {
                columns_ratio: vec![2,3,3],
                primary_paint: Paint::with_fg_bg(Color::White,  Color::Default),
                dir_paint:     Paint::with_fg_bg(Color::Cyan,   Color::Default).bold(),
                symlink_paint: Paint::with_fg_bg(Color::Yellow, Color::Default).bold(),
                file_paint:    Paint::with_fg_bg(Color::White,  Color::Default),
                unknown_paint: Paint::with_fg_bg(Color::Grey,   Color::White)  .bold(),
                preview_paint: Paint::with_fg_bg(Color::Green,  Color::Default),
                scrolling_gap: 4,
                copy_done_notification_delay_ms: 2000,
            },
            starting_path,
        )
    }

    fn init() -> Overseer {
        // let starting_path = PathBuf::from("/home/igorek/Stuff");
        // let starting_path = PathBuf::from("/home/igorek/.config/google-chrome");
        let mut starting_path = absolute_pathbuf();
        starting_path.pop();
        starting_path.pop();
        starting_path.pop();

        Overseer {
            color_system: ColorSystem::new(),
            system: Overseer::init_system(starting_path),
            mode: Mode::AwaitingCommand,
            possible_inputs: generate_possible_inputs(),
            current_input: String::new(),
            terminated: false,
        }
    }

    fn work(&mut self) {
        while !self.terminated {
            self.system.draw(&mut self.color_system);
            self.maybe_draw_matches();
            self.handle_input();
        };
    }

    fn maybe_draw_matches(&mut self) {
        if self.current_input.is_empty() { return; }

        let found_matches = matches_that_start_with(
            &self.current_input, &self.possible_inputs);
        if found_matches.len() > 0 {
            let completion = self.current_input.len();
            self.system.draw_available_matches(
                &mut self.color_system, &found_matches, completion);
        }
    }

    fn handle_input(&mut self) {
        let input = self.system.get();
        if let Some(Input::Character(c)) = input {
            if self.mode == Mode::AwaitingCommand {
                self.current_input.push(c);

                let mut found_matches = matches_that_start_with(
                    &self.current_input, &self.possible_inputs);
                let found_exact = exact_match(&found_matches, &self.current_input);
                if found_exact {
                    let (_, command) = found_matches.pop().unwrap();
                    let terminate = Overseer::handle_command(&mut self.system, command);
                    if terminate { self.terminated = true; }
                }

                if found_exact || found_matches.is_empty() {
                    self.current_input.clear();
                }
            } else if self.mode == Mode::Input {}
        } else if let Some(Input::KeyResize) = input { self.system.resize(); }
    }

    fn handle_command(system: &mut System, command: &Command) -> bool {
        let mut terminate = false;
        match command {
            Command::Terminate          => terminate = true,
            Command::Up(n)              => for _ in 0..*n {system.up()},
            Command::Down(n)            => for _ in 0..*n {system.down()},
            Command::Left               => system.left(),
            Command::Right              => system.right(),
            Command::Sort(sorting_type) => system.sort_with(*sorting_type),
            Command::GoTo(path)         => system.goto(path),
            Command::Remove             => system.remove_selected(),
            Command::Update             => system.update_current(),
            Command::Yank               => system.yank_selected(),
            Command::Cut                => system.cut_selected(),
            Command::Paste              => system.paste_into_current(),
            Command::CumulativeSize     => system.get_cumulative_size(),
        }
        terminate
    }
}

fn main() {
    Overseer::init().work();
}
