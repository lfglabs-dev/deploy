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
    user: String,
    ssh_key: String,
});

#[derive(Clone, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    #[serde(rename = "commands")]
    Commands { name: String, commands: Vec<String> },
    #[serde(rename = "upload")]
    Upload {
        name: String,
        source_folder: String,
        target_folder: String,
    },
}

pub_struct!(Clone, Deserialize; Config {
    server: Server,
    actions: Vec<Action>,
});

pub fn load(config_path: &str) -> Config {
    let file_contents = fs::read_to_string(config_path).expect("error: unable to read file");

    let config: Config =
        toml::from_str(&file_contents).expect("error: unable to deserialize config");

    config
}
