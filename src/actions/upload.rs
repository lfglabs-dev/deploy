use crate::Logger;
use crate::{config::Config, log};
use async_std::fs::File;
use async_std::io::ReadExt;
use colored::*;
use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType::CurrentLine;
use crossterm::ExecutableCommand;
use dirs_next::home_dir;
use ignore::WalkBuilder;
use openssh_sftp_client::fs::Fs;
use openssh_sftp_client::Sftp;
use std::future::Future;
use std::io::{stdout, Error, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;

fn expand_user_path(user_path: &str) -> String {
    if user_path.starts_with("~/") {
        user_path.replacen(
            "~",
            home_dir().expect("couldn't expand ~/").to_str().unwrap(),
            1,
        )
    } else {
        user_path.to_string()
    }
}

fn expand_server_path(server_path: &str, username: &str) -> String {
    if server_path.starts_with("~/") {
        server_path.replacen("~", &format!("/home/{}", username), 1)
    } else {
        server_path.to_string()
    }
}

const CHUNK_SIZE: usize = 8 * 1024;

async fn ensure_directory_exists(fs: &mut Fs, file_path: &PathBuf) -> Result<(), Error> {
    if let Some(parent_path) = file_path.parent() {
        create_dir_recursive(fs, parent_path.to_path_buf()).await
    } else {
        Err(Error::new(
            std::io::ErrorKind::NotFound,
            "No parent directory found",
        ))
    }
}

fn create_dir_recursive<'a>(
    fs: &'a mut Fs,
    dir_path: PathBuf,
) -> Pin<Box<dyn Future<Output = Result<(), Error>> + 'a>> {
    Box::pin(async move {
        if let Some(parent) = dir_path.parent() {
            create_dir_recursive(fs, parent.to_path_buf()).await?;
        }

        // Here, you may need to adjust the error handling based on the actual API of openssh_sftp_client
        match fs.metadata(&dir_path).await {
            Ok(_) => Ok(()), // Directory exists, nothing to do
            Err(_) => match fs.create_dir(&dir_path).await {
                Ok(_) => Ok(()),
                Err(_) => Err(Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to create directory",
                )),
            },
        }
    })
}

fn rewrite(message: String) {
    let mut writer = stdout();
    writer.execute(MoveUp(1)).unwrap();
    writer
        .execute(Clear(CurrentLine))
        .unwrap()
        .execute(MoveToColumn(0))
        .unwrap();
    println!("{}", message);
    writer.flush().unwrap();
}

fn progress_str(progress: f64) -> String {
    match progress {
        p if p < 0.125 => "\u{258F}".to_string(), // ▏
        p if p < 0.250 => "\u{258E}".to_string(), // ▎
        p if p < 0.375 => "\u{258D}".to_string(), // ▍
        p if p < 0.500 => "\u{258C}".to_string(), // ▌
        p if p < 0.625 => "\u{258B}".to_string(), // ▋
        p if p < 0.750 => "\u{258A}".to_string(), // ▊
        p if p < 0.875 => "\u{2589}".to_string(), // ▉
        _ => "\u{2588}".to_string(),              // █
    }
}

pub async fn upload(
    config: &Config,
    logger: &mut Logger,
    sftp: &mut Sftp,
    source_folder: &String,
    target_folder: &String,
) {
    log!(
        logger,
        "{}{}{}{}{}",
        "Uploading: '".bright_black(),
        source_folder.blue(),
        "' to '".bright_black(),
        target_folder.blue(),
        "'".bright_black()
    );
    let source_folder = expand_user_path(&source_folder);
    let mut ignore_path = Path::new(&source_folder).to_path_buf();
    ignore_path.push(".deployignore");
    let mut builder = WalkBuilder::new(&source_folder);
    builder.standard_filters(false);
    if ignore_path.exists() {
        println!(
            "{}{}{}",
            "Found: '".bright_black(),
            ignore_path.to_str().unwrap(),
            "'".bright_black()
        );
        builder.add_ignore(ignore_path);
    }

    let target_folder = expand_server_path(target_folder, &config.server.user);
    for result in builder.build() {
        match result {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() {
                    // Compute relative path
                    let relative_path = path.strip_prefix(&source_folder).unwrap();
                    let target_path = Path::new(&target_folder).join(relative_path);

                    // Log the file transfer
                    println!(
                        "{} '{}'",
                        "▏".bright_cyan(),
                        relative_path.display().to_string().bright_black()
                    );

                    if let Err(err) = ensure_directory_exists(&mut sftp.fs(), &target_path).await {
                        rewrite(format!(
                            "{} Failed to ensure directory exists: {}",
                            "Error:".bright_red(),
                            err
                        ))
                    }

                    match sftp.create(&target_path).await {
                        Ok(mut target_file) => {
                            // Open the source file
                            if let Ok(mut source_file) = File::open(&path).await {
                                let metadata = source_file
                                    .metadata()
                                    .await
                                    .expect("Unable to read file metadata");

                                let total_size = metadata.len() as usize;
                                let mut buffer = vec![0; CHUNK_SIZE];
                                let mut uploaded = 0;

                                // Read and write in chunks
                                while let Ok(bytes_read) = source_file.read(&mut buffer).await {
                                    if bytes_read == 0 {
                                        break;
                                    };
                                    target_file
                                        .write_all(&buffer[..bytes_read])
                                        .await
                                        .expect("Error writing to file");
                                    uploaded += bytes_read;

                                    let upload_ratio = uploaded as f64 / total_size as f64;
                                    rewrite(format!(
                                        "{} '{}' ({:.2}%)",
                                        progress_str(upload_ratio).bright_cyan(),
                                        relative_path.display().to_string().bright_black(),
                                        upload_ratio * 100.
                                    ));
                                }
                                target_file
                                    .sync_all()
                                    .await
                                    .expect("Unable to sync file write");

                                logger
                                    .add_uploaded_file(relative_path.display().to_string())
                                    .await;
                            } else {
                                println!(
                                    "{} Unable to open source file, {}",
                                    "Error:".bright_red(),
                                    path.display()
                                );
                            }
                        }
                        Err(err) => println!("{} {}", "Error:".bright_red(), err),
                    }
                }
            }
            Err(err) => println!("{} {}", "Error:".bright_red(), err),
        }
    }
    logger.stop_files_display().await;
}
