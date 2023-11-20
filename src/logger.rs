use crate::actions::commands::handle_terminal_streaming;
use colored::Colorize;
use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use lazy_static::lazy_static;
use regex::Regex;
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{stdout, Write},
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::Notify;
use tokio_util::codec::{FramedRead, LinesCodec};

pub const REMOTE_TERM_SIZE: usize = 5;

lazy_static! {
    pub static ref ANSI_ESCAPE_CODE: Regex = Regex::new("\x1B\\[[0-9;]*[a-zA-Z]").unwrap();
}
pub struct Logger {
    log_file: Arc<tokio::sync::Mutex<std::fs::File>>,
    remote_buffer: Arc<Mutex<VecDeque<String>>>,
}

impl Logger {
    pub fn new() -> Logger {
        let deployments_dir = Path::new(".deployments");

        // Create the directory if it does not exist
        if !deployments_dir.exists() {
            fs::create_dir(deployments_dir).expect("failed to create .deployments directory");
        }

        // Find the smallest number not already taken
        let mut num = 1;
        loop {
            let file_name = format!("deployment_{}.txt", num);
            let file_path = deployments_dir.join(file_name);
            if !file_path.exists() {
                let log_file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(file_path)
                    .expect("cannot open file");

                return Logger {
                    log_file: Arc::new(tokio::sync::Mutex::new(log_file)),
                    remote_buffer: Arc::new(Mutex::new(VecDeque::new())),
                };
            }
            num += 1;
        }
    }

    async fn log_to_file(&mut self, message: String) {
        let mut log_file = self.log_file.lock().await;
        if let Err(e) = writeln!(log_file, "{}", message) {
            eprintln!("Failed to write to log file: {}", e);
        }
        if let Err(e) = log_file.flush() {
            eprintln!("Failed to flush log file: {}", e);
        }
    }

    pub async fn log(&mut self, message: String) {
        println!("{}", message);
        self.log_to_file(ANSI_ESCAPE_CODE.replace_all(&message, "").to_string())
            .await;
    }

    pub async fn add_uploaded_file(&mut self, file_name: String) {
        let mut buffer = self.remote_buffer.lock().unwrap();
        let prev_buffer_length: u16 = buffer.len().try_into().unwrap();

        if buffer.len() == REMOTE_TERM_SIZE.into() {
            buffer.pop_front();
        }
        buffer.push_back(format!(
            "{} '{}'",
            "✔".bright_green(),
            file_name.bright_black()
        ));
        // saving without colors and no flushing
        if let Err(e) = writeln!(
            self.log_file.lock().await,
            "{}",
            format!("✔ '{}'", file_name)
        ) {
            eprintln!("Failed to write to log file: {}", e);
        }

        let mut writer = stdout();
        writer.execute(MoveUp(prev_buffer_length + 1)).unwrap();
        for line in buffer.iter() {
            writer
                .execute(Clear(ClearType::CurrentLine))
                .unwrap()
                .execute(MoveToColumn(0))
                .unwrap();
            println!("{}", line);
        }
        // clear previous temporary updating
        writer
            .execute(Clear(ClearType::CurrentLine))
            .unwrap()
            .execute(MoveToColumn(0))
            .unwrap();
    }

    pub async fn stop_files_display(&mut self) {
        self.remote_buffer = Arc::new(Mutex::new(VecDeque::new()));
        if let Err(e) = self.log_file.lock().await.flush() {
            eprintln!("Failed to flush log file: {}", e);
        }
    }

    pub async fn start_remote_logging(&mut self, mut command: openssh::Child<&openssh::Session>) {
        let stdout_reader = FramedRead::new(
            command.stdout().take().expect("Failed to open stdout"),
            LinesCodec::new(),
        );
        let stderr_reader = FramedRead::new(
            command.stderr().take().expect("Failed to open stderr"),
            LinesCodec::new(),
        );

        println!("{} Connecting...", "Remote console:".bright_black());

        let notifier = Arc::new(Notify::new());
        let notifier_clone = notifier.clone();
        let handle_notifier = tokio::spawn(async move {
            let mut reader = BufReader::new(io::stdin()).lines();
            let mut writer = stdout();
            loop {
                tokio::select! {
                    _ = reader.next_line() => {
                        notifier_clone.notify_waiters();
                        writer.execute(MoveUp(2)).unwrap();
                        break;
                    }
                    _ = notifier_clone.notified() => {
                        writer.execute(MoveUp(1)).unwrap();
                        break;
                    }
                }
            }
            writer
                .execute(Clear(ClearType::CurrentLine))
                .unwrap()
                .execute(MoveToColumn(0))
                .unwrap();
            println!("{} Exited", "Remote console:".bright_black());
            writer.flush().unwrap();
        });

        let stdout_handle = handle_terminal_streaming(
            stdout_reader,
            Arc::clone(&self.remote_buffer),
            stdout(),
            Arc::clone(&self.log_file),
            notifier.clone(),
        );
        let stderr_handle = handle_terminal_streaming(
            stderr_reader,
            Arc::clone(&self.remote_buffer),
            stdout(),
            Arc::clone(&self.log_file),
            notifier.clone(),
        );

        // Await the tasks
        let _ = tokio::try_join!(handle_notifier, stdout_handle, stderr_handle)
            .expect("Failed to start remote streaming");

        // Ensure writing logs to file
        if let Err(e) = self.log_file.lock().await.flush() {
            eprintln!("Failed to flush log file: {}", e);
        }

        // Clear buffer
        self.remote_buffer = Arc::new(Mutex::new(VecDeque::new()));
    }
}

#[macro_export]
macro_rules! log {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log(format!($($arg)*)).await;
    };
}
