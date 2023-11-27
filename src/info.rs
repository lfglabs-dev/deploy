use std::{
    fs::File,
    io::{self, BufRead},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Local, TimeZone, Utc};
use colored::Colorize;
use git2::Repository;

pub fn get_info() {
    let folder_path = Path::new(".deployments");
    let mut i = 1;
    let repo_opt = Repository::open(".");
    loop {
        let file_name = format!("deployment_{}.txt", i);
        let file_path = folder_path.join(&file_name);

        if file_path.exists() {
            let Ok((conf_name, date, commit_hash)) = extract_info(&file_path) else {
                println!("Unable to read \"{}\"", (&file_path).display().to_string().bright_red());
                return;
            };

            let conf_path = Path::new(&conf_name);
            let conf_file_name = &conf_path.file_name().unwrap_or_default().to_string_lossy();
            let conf_parent = conf_path.parent().unwrap().to_string_lossy();
            println!(
                "{}) {}/{}, {}",
                i,
                conf_parent.bright_black(),
                conf_file_name.bright_green(),
                date.with_timezone(&Local).format("%d-%m-%Y %H:%M:%S")
            );
            match &repo_opt {
                Ok(repo) => match repo.revparse_single(&commit_hash) {
                    Ok(oid) => {
                        let commit = repo.find_commit(oid.id()).expect("Unable to read commit");
                        println!("   {}", commit.message().unwrap().trim());
                    }
                    Err(_) => {}
                },
                Err(_) => {}
            }

            i += 1;
        } else {
            break;
        }
    }
    let deployed = i - 1;
    if deployed == 0 {
        println!("{}", "No deployments found".bright_red());
    }
}

fn extract_info(path: &PathBuf) -> io::Result<(String, DateTime<Utc>, String)> {
    let file = File::open(path)?;
    let mut lines = io::BufReader::new(file).lines();

    let config_name = lines
        .next()
        .and_then(|line| line.ok())
        .and_then(|line| line.strip_prefix("Loading: ").map(|s| s.trim().to_string()))
        .ok_or(io::Error::new(
            io::ErrorKind::Other,
            "Config name not found",
        ))?;

    let timestamp = lines
        .next()
        .and_then(|line| line.ok())
        .and_then(|line| {
            line.strip_prefix("Timestamp: ")
                .and_then(|s| s.trim().parse().ok())
        })
        .ok_or(io::Error::new(io::ErrorKind::Other, "Timestamp not found"))?;
    let date = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .ok_or(io::Error::new(io::ErrorKind::Other, "Invalid timestamp"))?;

    let commit_hash = lines
        .next()
        .and_then(|line| line.ok())
        .and_then(|line| {
            line.strip_prefix("Commit hash: ")
                .map(|s| s.trim().to_string())
        })
        .ok_or(io::Error::new(
            io::ErrorKind::Other,
            "Commit hash not found",
        ))?;

    Ok((config_name, date, commit_hash))
}
