#[derive(Debug, Copy, Clone)]
pub enum SortingType {
    Lexicographically,
    TimeModified,
    Any,
}

pub type Combination = String;
pub enum Command {
    Terminate,
    GoTo(String),
    Up(u32),
    Down(u32),
    Left,
    Right,
    Sort(SortingType),
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
    inputs.push(("j".to_string(), Command::Down(1)));
    inputs.push(("k".to_string(), Command::Up(1)));
    inputs.push(("l".to_string(), Command::Right));
    inputs.push(("K".to_string(), Command::Up(5)));
    inputs.push(("J".to_string(), Command::Down(5)));
    inputs.push(("sl".to_string(), Command::Sort(SortingType::Lexicographically)));
    inputs.push(("st".to_string(), Command::Sort(SortingType::TimeModified)));
    inputs.push(("sa".to_string(), Command::Sort(SortingType::Any)));
    inputs
}

pub fn description_of(command: &Command) -> String {
    match command {
        Command::Terminate => "Close the program".to_string(),
        Command::GoTo(path) => "Go to ".to_string() + path.as_str(),
        Command::Up(n) => format!("Navigate up one entry in the list {} times", n),
        Command::Down(n) => format!("Navigate down one entry in the list {} times", n),
        Command::Left => "Navigate to the parent directory".to_string(),
        Command::Right => "Navigate into the child directory or file".to_string(),
        Command::Sort(sorting_type) => format!("Sort entries {:?}", sorting_type),
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

