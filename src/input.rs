#[derive(PartialEq, Eq)]
pub enum Input {
    Unknown,
    EventResize,
    Char(char),
    Tab,
    ShiftTab,
    // Backspace,
    // Enter,
}

#[derive(Debug, Copy, Clone)]
pub enum SortingType {
    Lexicographically,
    TimeModified,
    Any,
}

#[derive(PartialEq, Eq)]
pub enum Combination {
    Str(String),
    Tab,
    ShiftTab,
}

fn regular(chars: &str) -> Combination {
    Combination::Str(chars.to_string())
}

pub enum Command {
    Terminate,
    GoTo(&'static str),
    Up(u32),
    Down(u32),
    Left,
    Right,
    Sort(SortingType),
    Remove,
    Cut,
    Update,
    Yank,
    Paste,
    CumulativeSize,
    SelectUnderCursor,
    InvertSelection,
    ClearSelection,
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
}

pub const fn max_combination_len() -> usize { 5 }

pub fn generate_possible_inputs() -> Vec<(Combination, Command)> {
    let mut inputs = Vec::new();
    inputs.push((regular("h"),  Command::Left));
    inputs.push((regular("j"),  Command::Down(1)));
    inputs.push((regular("k"),  Command::Up(1)));
    inputs.push((regular("l"),  Command::Right));
    inputs.push((regular("K"),  Command::Up(5)));
    inputs.push((regular("J"),  Command::Down(5)));
    inputs.push((regular("sl"), Command::Sort(SortingType::Lexicographically)));
    inputs.push((regular("st"), Command::Sort(SortingType::TimeModified)));
    inputs.push((regular("sa"), Command::Sort(SortingType::Any)));
    inputs.push((regular("gh"), Command::GoTo("/home/igorek/")));
    inputs.push((regular("gd"), Command::GoTo("/home/igorek/Downloads")));
    inputs.push((regular("gs"), Command::GoTo("/home/igorek/Studying")));
    inputs.push((regular("gS"), Command::GoTo("/home/igorek/Storage")));
    inputs.push((regular("gT"), Command::GoTo("/home/igorek/Storage/torrents")));
    inputs.push((regular("gc"), Command::GoTo("/home/igorek/screenshots")));
    inputs.push((regular("gt"), Command::GoTo("/home/igorek/Stuff")));
    inputs.push((regular("gm"), Command::GoTo("/home/igorek/Mutual")));
    inputs.push((regular("ge"), Command::GoTo("/mnt/External")));
    inputs.push((regular("gE"), Command::GoTo("/mnt/External2")));
    inputs.push((regular("dd"), Command::Remove));
    inputs.push((regular("dc"), Command::Cut));
    inputs.push((regular("yy"), Command::Yank));
    inputs.push((regular("pp"), Command::Paste));
    inputs.push((regular("u"),  Command::Update));
    inputs.push((regular("cs"), Command::CumulativeSize));
    inputs.push((regular("v"),  Command::SelectUnderCursor));
    inputs.push((regular("V"),  Command::InvertSelection));
    inputs.push((regular("cc"), Command::ClearSelection));
    inputs.push((Combination::Tab,      Command::NextTab));
    inputs.push((Combination::ShiftTab, Command::PreviousTab));
    inputs.push((regular("q"),          Command::CloseTab));
    inputs.push((regular("t"),          Command::NewTab));
    inputs
}

pub fn description_of(command: &Command) -> String {
    match command {
        Command::Terminate => "Close the program".to_string(),
        Command::GoTo(path) => "Go to ".to_string() + path,
        Command::Up(n) => format!("Navigate up one entry in the list {} times", n),
        Command::Down(n) => format!("Navigate down one entry in the list {} times", n),
        Command::Left => "Navigate to the parent directory".to_string(),
        Command::Right => "Navigate into the child directory or file".to_string(),
        Command::Sort(sorting_type) => format!("Sort entries {:?}", sorting_type),
        Command::Remove => "Remove selected entry(ies) from the filesystem".to_string(),
        Command::Update => "Update the current directory".to_string(),
        Command::Yank => "Yank selected entries into buffer".to_string(),
        Command::Cut => "Cut selected entries into buffer".to_string(),
        Command::Paste => "Paste the yanked entry into the current directory".to_string(),
        Command::CumulativeSize => "Calculate the cumulative size of current entry".to_string(),
        Command::SelectUnderCursor => "Flips the selection for the entry under cursor".to_string(),
        Command::InvertSelection => "Inverts the selection in the current directory".to_string(),
        Command::ClearSelection => "Clears the list of selected items and the buffer of yanked or cut items".to_string(),
        Command::NewTab => "Creates a new tab that is a clone of the current one".to_string(),
        Command::CloseTab => "Closes current Tab. If it is the last tab then closes the program".to_string(),
        Command::NextTab => "Selects the next Tab (if any) as the new current tab".to_string(),
        Command::PreviousTab => "Selects the previous Tab (if any) as the new current tab".to_string(),
    }
}

pub type Match   = (Combination, Command);
pub type Matches = Vec<Match>;

pub fn matches_that_start_with<'a>(slice: &Combination, array: &'a Matches) -> Vec<&'a Match> {
    let mut matches = Vec::new();
    for entry in array {
        let combination = &entry.0;
        let ok = match slice {
            Combination::Tab =>
                if let Combination::Tab      = combination { true }
                else                                       { false },
            Combination::ShiftTab =>
                if let Combination::ShiftTab = combination { true }
                else                                       { false },
            Combination::Str(substring) =>
                if let Combination::Str(string) = combination {
                    string.starts_with(substring)
                } else { false },
        };
        if ok { matches.push(entry); }
    }
    matches
}

pub fn exact_match(matches: &Vec<&Match>, input: &Combination) -> bool {
    (matches.len() == 1) && (matches[0].0 == *input)
}
