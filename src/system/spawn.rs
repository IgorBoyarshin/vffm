use std::process::Child;
use std::process::{Command, Stdio};
use std::collections::HashMap;
use std::path::PathBuf;
use std::ffi::OsStr;

#[derive(Clone)]
pub struct SpawnRule {
    rule: String,
    is_external: bool,
}

impl SpawnRule {
    fn generate(&self, file_name: &str) -> (String, Vec<String>, bool) {
        let placeholder = "@";
        let mut args = Vec::new();
        let mut parts = self.rule.split_whitespace();
        let app = parts.next().unwrap();
        for arg in parts { // the rest
            if arg == placeholder { args.push(file_name.to_string()); }
            else                  { args.push(arg.to_string()); }
        }
        (app.to_string(), args, self.is_external)
    }
}

pub enum SpawnFile {
    Extension(String),
    ExactName(String),
}

pub struct SpawnPattern {
    file: SpawnFile,
    rule: SpawnRule,
}

impl SpawnPattern {
    fn new_ext(ext: &str, rule: &str, is_external: bool) -> SpawnPattern {
        SpawnPattern {
            file: SpawnFile::Extension(ext.to_string()),
            rule: SpawnRule{ rule: rule.to_string(), is_external },
        }
    }

    fn new_exact(name: &str, rule: &str, is_external: bool) -> SpawnPattern {
        SpawnPattern {
            file: SpawnFile::ExactName(name.to_string()),
            rule: SpawnRule{ rule: rule.to_string(), is_external },
        }
    }
}

pub fn text_extensions() -> Vec<&'static str> {
    vec!["txt", "cpp", "h", "rs", "lock", "toml", "zsh", "java", "py",
        "sh", "md", "log", "yml", "tex", "nb", "js", "ts", "html", "css", "json"]
}

pub fn text_exact_names() -> Vec<&'static str> {
    vec!["Makefile", ".gitignore"]
}

pub fn generate_spawn_patterns() -> Vec<SpawnPattern> {
    let add_to_apps = |apps: &mut HashMap<String, (Vec<String>, bool)>,
            app: &str, spawn_files: Vec<&'static str>, is_external: bool| {
        let mut vec = Vec::new();
        for spawn_file in spawn_files {
            vec.push(spawn_file.to_string());
        }
        apps.insert(app.to_string(), (vec, is_external));
    };

    let mut apps_extensions  = HashMap::new();
    let mut apps_exact_names = HashMap::new();
    let external = true; // for convenience and readability
    let not_external = !external;

    add_to_apps(&mut apps_extensions, "vim @", text_extensions(), not_external);
    add_to_apps(&mut apps_exact_names, "vim @", text_exact_names(), not_external);
    add_to_apps(&mut apps_extensions, "vlc @", vec!["mkv", "avi", "mp4", "mp3", "m4b"], external);
    add_to_apps(&mut apps_extensions, "zathura @", vec!["pdf", "djvu"], external);
    add_to_apps(&mut apps_extensions, "rifle_sxiv @", vec!["jpg", "jpeg", "png"], external);

    let mut patterns: Vec<SpawnPattern> = Vec::new();
    for (app, (extensions, is_external)) in apps_extensions.into_iter() {
        for ext in extensions {
            patterns.push(SpawnPattern::new_ext(&ext, &app, is_external));
        }
    }
    for (app, (names, is_external)) in apps_exact_names.into_iter() {
        for name in names {
            patterns.push(SpawnPattern::new_exact(&name, &app, is_external));
        }
    }

    patterns
}

pub fn spawn_rule_for(full_path: &PathBuf, spawn_patterns: &Vec<SpawnPattern>)
        -> Option<(String, Vec<String>, bool)> {
    let file_name = full_path.file_name().unwrap().to_str().unwrap();
    let full_path = full_path.to_str().unwrap();
    for SpawnPattern { file, rule } in spawn_patterns.iter() {
        match file {
            SpawnFile::Extension(ext) => if file_name.to_ascii_lowercase()
                                                .ends_with(ext.as_str()) {
                return Some(rule.generate(full_path));
            },
            SpawnFile::ExactName(name) => if file_name == name {
                return Some(rule.generate(full_path));
            }
        }
    }
    None
}

fn split_into_app_and_args(text: &str) -> (&str, Vec<String>) {
    let mut parts = text.split_whitespace();
    let app = parts.next().unwrap();
    let mut args = Vec::new();
    for part in parts {
        if part.starts_with("-") && !part.starts_with("--") {
            // Assume valid format like -fLaG
            for c in part.chars().skip(1) {
                let mut arg = "-".to_string();
                arg.push(c);
                args.push(arg);
            }
        } else {
            args.push(part.to_string());
        }
    }
    (app, args)
}

pub fn execute_command_from(path: &PathBuf, command: &str) {
    let (app, args) = split_into_app_and_args(command);
    Command::new(app).args(args)
        .stderr(Stdio::null()).stdout(Stdio::piped())
        .current_dir(path)
        .spawn().expect("failed to execute process");
}

pub fn spawn_process_async<S: AsRef<OsStr>>(app: &str, args: Vec<S>) -> Child {
    Command::new(app).args(args)
        .stderr(Stdio::null()).stdout(Stdio::piped())
        .spawn().expect("failed to execute process")
}

pub fn spawn_process_wait<S: AsRef<OsStr>>(app: &str, args: Vec<S>) {
    Command::new(app).args(args)
        .stderr(Stdio::null()).stdout(Stdio::null())
        .status().expect("failed to execute process");
}

pub fn spawn_program<S: AsRef<OsStr>>(app: &str, args: Vec<S>, is_external: bool) {
    if is_external {
        Command::new(app).args(args)
            .stderr(Stdio::null()).stdout(Stdio::null())
            .spawn().expect("failed to execute process");
    } else {
        Command::new(app).args(args)
            .status().expect("failed to execute process");
    }
}
