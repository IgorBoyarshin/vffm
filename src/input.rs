use std::collections::HashMap;

#[derive(PartialEq, Eq)]
pub enum Input {
    Unknown,
    EventResize,
    Char(char),
    Tab,
    ShiftTab,
    Enter,
    Escape,
    Backspace,
}

#[derive(Debug, Copy, Clone)]
pub enum SortingType {
    Lexicographically,
    TimeModified,
    Any,
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum Combination {
    Str(String),
    Tab,
    ShiftTab,
    Enter,
}

fn regular(chars: &str) -> Combination {
    Combination::Str(chars.to_string())
}

#[derive(Copy, Clone)]
pub enum Command {
    // Terminate,
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
    ChangeCurrentName,
    EnterSearchMode,
}


pub const fn max_combination_len() -> usize { 5 }

pub type Match   = (Combination, Command);
pub type Matches = HashMap<Combination, Vec<Match>>;

pub fn generate_possible_inputs() -> Matches {
    let mut inputs: Matches = HashMap::new();
    let mut insert = |combination: Combination, command: Command| {
        let comb = combination.clone();
        if let Combination::Str(chars) = comb {
            for i in 1..=chars.len() {
                let mut partial = chars.clone();
                partial.truncate(i);
                let v = inputs.entry(Combination::Str(partial)).or_insert(Vec::new());
                (*v).push((combination.clone(), command.clone()));
            }
        } else {
            let v = inputs.entry(comb).or_insert(Vec::new());
            (*v).push((combination.clone(), command.clone()));
        }
    };
    insert(regular("h"),  Command::Left);
    // insert(regular("р"),  Command::Left);
    insert(regular("j"),  Command::Down(1));
    // insert(regular("о"),  Command::Down(1));
    insert(regular("k"),  Command::Up(1));
    // insert(regular("л"),  Command::Up(1));
    insert(regular("l"),  Command::Right);
    // insert(regular("д"),  Command::Right);
    insert(regular("K"),  Command::Up(5));
    insert(regular("J"),  Command::Down(5));
    insert(regular("sl"), Command::Sort(SortingType::Lexicographically));
    insert(regular("st"), Command::Sort(SortingType::TimeModified));
    insert(regular("sa"), Command::Sort(SortingType::Any));
    insert(regular("gh"), Command::GoTo("/home/igorek/"));
    insert(regular("gd"), Command::GoTo("/home/igorek/Downloads"));
    insert(regular("gs"), Command::GoTo("/home/igorek/Studying"));
    insert(regular("gS"), Command::GoTo("/home/igorek/Storage"));
    insert(regular("gT"), Command::GoTo("/home/igorek/Storage/torrents"));
    insert(regular("gc"), Command::GoTo("/home/igorek/screenshots"));
    insert(regular("gt"), Command::GoTo("/home/igorek/Stuff"));
    insert(regular("gm"), Command::GoTo("/home/igorek/Mutual"));
    insert(regular("ge"), Command::GoTo("/mnt/External"));
    insert(regular("gE"), Command::GoTo("/mnt/External2"));
    insert(regular("dd"), Command::Remove);
    insert(regular("dc"), Command::Cut);
    insert(regular("yy"), Command::Yank);
    insert(regular("pp"), Command::Paste);
    insert(regular("u"),  Command::Update);
    insert(regular("cs"), Command::CumulativeSize);
    insert(regular("v"),  Command::SelectUnderCursor);
    insert(regular("V"),  Command::InvertSelection);
    insert(regular("cc"), Command::ClearSelection);
    insert(Combination::Tab,      Command::NextTab);
    insert(Combination::ShiftTab, Command::PreviousTab);
    insert(regular("q"),          Command::CloseTab);
    insert(regular("t"),          Command::NewTab);
    insert(regular("/"),          Command::EnterSearchMode);
    inputs
}

pub fn description_of(command: &Command) -> String {
    match command {
        // Command::Terminate => "Close the program".to_string(),
        Command::GoTo(path) => format!("Go to {}", path),
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
        Command::ChangeCurrentName => "Change the name of the current entry".to_string(),
        Command::EnterSearchMode => "Go inside the search bar to edit the query".to_string(),
    }
}

pub fn exact_match(matches: &Vec<Match>, input: &Combination) -> bool {
    (matches.len() == 1) && (matches[0].0 == *input)
}
