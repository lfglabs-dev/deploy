mod actions;
mod config;
mod finder;
mod info;
mod logger;
use crate::logger::Logger;
use chrono::{DateTime, Local, Utc};
use clap::Parser;
use colored::*;
use git2::Repository;
use std::{fs, path::PathBuf, time::SystemTime};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    // to find config files in the folder
    #[arg(long)]
    find: Option<String>,

    #[arg(long)]
    info: bool,

    // config file
    file: Option<String>,

    // actions from the config file to skip
    #[arg(long, use_value_delimiter = true)]
    skip: Option<Vec<String>>,
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    if args.info {
        info::get_info();
    } else if let Some(start_folder) = args.find {
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
        let mut logger = Logger::new();
        log!(logger, "{} {}", "Loading:".bright_black(), &config_path);
        log!(
            logger,
            "{} {}",
            "Timestamp:".bright_black(),
            Utc::now().timestamp()
        );
        let config = config::load(&config_path);
        match Repository::open(".") {
            Ok(repo) => {
                let head = repo.head().expect("Unable to access git HEAD");
                let head = head
                    .peel_to_commit()
                    .expect("Unable to pull reference to HEAD commit");
                log!(
                    logger,
                    "{} {}",
                    "Commit hash:".bright_black(),
                    head.id().to_string()
                );
            }
            Err(_) => {}
        }

        actions::runner::execute_actions(
            &mut logger,
            config,
            args.skip.unwrap_or_else(Vec::new).into_iter().collect(),
        )
        .await;
    }
}
