mod action;
mod config;
mod finder;
use chrono::{DateTime, Local};
use clap::Parser;
use colored::*;
use std::{fs, path::PathBuf, time::SystemTime};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    // to find config files in the folder
    #[arg(long)]
    find: Option<String>,

    // config file
    file: Option<String>,

    // actions from the config file to skip
    #[arg(long, use_value_delimiter = true)]
    skip: Option<Vec<String>>,
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    if let Some(start_folder) = args.find {
        let mut found: u32 = 0;
        let mut action = |path: PathBuf| {
            if let Ok(metadata) = fs::metadata(&path) {
                let modified_date = DateTime::<Local>::from(
                    metadata.modified().unwrap_or_else(|_| SystemTime::now()),
                )
                .format("%Y-%m-%d %H:%M:%S");
                println!(
                    "{:50} {:10} {}",
                    path.to_string_lossy().green(),
                    format!("{}B", metadata.len()).blue(),
                    modified_date.to_string().yellow()
                );
                found += 1;
            }
        };

        if let Err(e) = finder::find_deploy_files(&start_folder, &mut action) {
            eprintln!("Error: {}", e.to_string().red());
        }

        if found == 0 {
            println!(
                "{}",
                "No .deploy.toml files found in the specified directory".yellow()
            );
        } else {
            println!("{} files found.", found.to_string().cyan());
        }
    } else if let Some(config_path) = args.file {
        println!("{} {}", "Loading:".bright_black(), &config_path);
        let config = config::load(&config_path);
        action::execute_actions(
            config,
            args.skip.unwrap_or_else(Vec::new).into_iter().collect(),
        )
        .await;
    }
}
