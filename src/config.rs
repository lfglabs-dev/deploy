use indexmap::IndexMap;
use serde::Deserialize;
use std::fs;

macro_rules! pub_struct {
    ($($derive:path),*; $name:ident {$($field:ident: $t:ty),* $(,)?}) => {
        #[derive($($derive),*)]
        pub struct $name {
            $(pub $field: $t),*
        }
    }
}

pub_struct!(Clone, Deserialize; Server {
    host: String,
    user : String,
    ssh_key : String,
});

pub_struct!(Clone, Deserialize; Action {
    commands: Option<Vec<String>>,
    source_folder: Option<String>,
    target_folder: Option<String>,
});

pub_struct!(Clone, Deserialize; Config {
    server: Server,
    // ordered map for stdlib
    actions: IndexMap<String, Action>
});

pub fn load(config_path: &String) -> Config {
    let file_contents = fs::read_to_string(config_path);
    if file_contents.is_err() {
        panic!("error: unable to read file with path \"{}\"", config_path);
    }

    let config: Config = match toml::from_str(&file_contents.unwrap()) {
        Ok(loaded) => loaded,
        Err(err) => panic!("error: unable to deserialize config. {}", err),
    };

    // Validate source_folder and target_folder
    for (action_name, action) in &config.actions {
        match (&action.source_folder, &action.target_folder) {
            (Some(_), None) | (None, Some(_)) => {
                panic!("error in action {}: source_folder and target_folder must both be specified or neither", action_name);
            }
            _ => {}
        }
    }

    config
}
