pub type Combination = String;
pub enum Command {
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

pub type Matches<'a> = Vec<&'a (Combination, Command)>;

pub const fn max_combination_len() -> usize { 5 }

pub fn generate_possible_inputs() -> Vec<(Combination, Command)> {
    let mut inputs = Vec::new();
    inputs.push(("q".to_string(), Command::Terminate));
    inputs.push(("h".to_string(), Command::Left));
    inputs.push(("j".to_string(), Command::Down));
    inputs.push(("k".to_string(), Command::Up));
    inputs.push(("l".to_string(), Command::Right));
    inputs
}

pub fn description_of(command: &Command) -> String {
    match command {
        Command::Terminate => "Close the program".to_string(),
        Command::GoTo(path) => "Go to ".to_string() + path.as_str(),
        Command::Up => "Navigate up one entry in the list".to_string(),
        Command::Down => "Navigate down one entry in the list".to_string(),
        Command::Left => "Navigate to the parent directory".to_string(),
        Command::Right => "Navigate into the child directory or file".to_string(),
    }
}

pub fn combinations_that_start_with<'a>(slice: &str, array: Matches<'a>)
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

