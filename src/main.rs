// use std::{thread, time};

extern crate pancurses;

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
    current_input: Option<Combination>,

    possible_inputs: Matches, // const
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
            current_input: None,
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
        if let Some(combination) = self.current_input.as_ref() {
            if let Some(matches) = self.possible_inputs.get(&combination) {
                if matches.len() > 0 {
                    let completion_count = if let Combination::Str(string) = combination {
                        string.len()
                    } else { 0 }; // not supposed to happen
                    self.system.draw_available_matches(
                        &mut self.color_system, &matches, completion_count);
                }
            }
        }
    }

    fn handle_input(&mut self) {
        let input = self.system.get();
        if let Some(Input::EventResize) = input { self.system.resize(); }
        else if let Some(input) = input {
            if self.mode == Mode::AwaitingCommand {
                let combination = match input {
                    Input::Tab      => Some(Combination::Tab),
                    Input::ShiftTab => Some(Combination::ShiftTab),
                    Input::Char(c)  => {
                        if let Some(Combination::Str(mut string)) = self.current_input.take() {
                            string.push(c);
                            Some(Combination::Str(string))
                        } else { Some(Combination::Str(c.to_string())) }
                    },
                    _ => None,
                };

                self.current_input = self.handle_combination(combination);
            } else if self.mode == Mode::Input {}
        }
    }

    // Returns the new current_input
    fn handle_combination(&mut self, combination: Option<Combination>) -> Option<Combination> {
        if let Some(combination) = combination {
            if let Some(matches) = self.possible_inputs.get(&combination) {
                if !exact_match(matches, &combination) { return Some(combination); }
                let (_, command) = matches[0];
                let terminate = Overseer::handle_command(&mut self.system, &command);
                if terminate { self.terminated = true; }
            }
        }
        None
    }

    fn handle_command(system: &mut System, command: &Command) -> bool {
        let mut terminate = false;
        match command {
            // Command::Terminate          => terminate = true,
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
            Command::SelectUnderCursor  => system.select_under_cursor(),
            Command::InvertSelection    => system.invert_selection(),
            Command::ClearSelection     => system.clear_selection(),
            Command::NewTab             => system.new_tab(),
            Command::CloseTab           => terminate = system.close_tab(),
            Command::NextTab            => system.next_tab(),
            Command::PreviousTab        => system.previous_tab(),
        }
        terminate
    }
}

fn main() {
    Overseer::init().work();
}
