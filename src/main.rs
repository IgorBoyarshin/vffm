#![feature(drain_filter)]
#![feature(const_str_len)]

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
                preview_paint: Paint::with_fg_bg(Color::Green,  Color::Default),
                paint_settings: PaintSettings {
                    dir_paint:     Paint::with_fg_bg(Color::Cyan,   Color::Default).bold(),
                    symlink_paint: Paint::with_fg_bg(Color::Yellow, Color::Default).bold(),
                    file_paint:    Paint::with_fg_bg(Color::White,  Color::Default),
                    unknown_paint: Paint::with_fg_bg(Color::Grey,   Color::White)  .bold(),
                },
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
            } else if self.mode == Mode::Input {
                match input {
                    Input::Escape    => self.system.cancel_input(),
                    Input::Enter     => self.system.confirm_input(),
                    Input::Char(c)   => self.system.insert_input(c),
                    Input::Backspace => self.system.remove_input_before_cursor(),
                    Input::Delete    => self.system.remove_input_under_cursor(),
                    Input::Left      => self.system.move_input_cursor_left(),
                    Input::Right     => self.system.move_input_cursor_right(),
                    Input::Tab       => self.current_input =
                        self.handle_combination(Some(Combination::Tab)),
                    Input::ShiftTab  => self.current_input =
                        self.handle_combination(Some(Combination::ShiftTab)),
                    _ => {},
                };
            }
            self.mode = match self.system.inside_input_mode() {
                true  => Mode::Input,
                false => Mode::AwaitingCommand,
            };
        }
    }

    // Returns the new current_input
    fn handle_combination(&mut self, combination: Option<Combination>) -> Option<Combination> {
        if let Some(combination) = combination {
            if let Some(matches) = self.possible_inputs.get(&combination) {
                if !exact_match(matches, &combination) { return Some(combination); }
                let (_, command) = matches[0];
                self.handle_command(&command);
            }
        }
        None
    }

    fn handle_command(&mut self, command: &Command) {
        match command {
            Command::Up(n)              => for _ in 0..*n {self.system.up()},
            Command::Down(n)            => for _ in 0..*n {self.system.down()},
            Command::Left               => self.system.left(),
            Command::Right              => self.system.right(),
            Command::Sort(sorting_type) => self.system.sort_with(*sorting_type),
            Command::GoTo(path)         => self.system.goto(path),
            Command::Remove             => self.system.remove_selected(),
            Command::Update             => self.system.update_current(),
            Command::Yank               => self.system.yank_selected(),
            Command::Cut                => self.system.cut_selected(),
            Command::Paste              => self.system.paste_into_current(),
            Command::CumulativeSize     => self.system.get_cumulative_size(),
            Command::SelectUnderCursor  => self.system.select_under_cursor(),
            Command::InvertSelection    => self.system.invert_selection(),
            Command::ClearSelection     => self.system.clear_selection(),
            Command::NewTab             => self.system.new_tab(),
            Command::CloseTab           => self.terminated = self.system.close_tab(),
            Command::NextTab            => self.system.next_tab(),
            Command::PreviousTab        => self.system.previous_tab(),
            Command::ChangeCurrentName  => {
                self.mode = Mode::Input;
                self.system.start_changing_current_name();
            },
            Command::EnterSearchMode    => {
                self.mode = Mode::Input;
                self.system.start_search();
            },
        }
    }
}

fn main() {
    Overseer::init().work();
}
