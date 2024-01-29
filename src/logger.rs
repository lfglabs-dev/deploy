use colored::Colorize;
use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    event::{Event, EventStream, KeyCode},
    execute,
    style::{Color, Print, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    ExecutableCommand,
};
use futures::{future::FutureExt, StreamExt};
use lazy_static::lazy_static;
use regex::Regex;
use russh::{client, Channel, ChannelMsg};
use std::str;
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{stdout, Write},
    path::Path,
    sync::{Arc, Mutex},
};

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

    pub async fn start_remote_logging(&mut self, mut channel: Channel<client::Msg>) {
        execute!(
            stdout(),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print("Remote console: "),
            SetForegroundColor(Color::Reset),
            Print("loading"),
            Print("\n"),
            MoveToColumn(0),
        )
        .unwrap();

        enable_raw_mode().unwrap();
        let mut reader = EventStream::new();
        loop {
            tokio::select! {
                msg = reader.next().fuse() => match msg {
                    Some(Ok(event)) => {
                        if event == Event::Key(KeyCode::Esc.into()) {
                            execute!(
                                stdout(),
                                MoveUp(1),
                                Clear(ClearType::CurrentLine),
                                SetForegroundColor(Color::Green),
                                Print("Remote console: "),
                                SetForegroundColor(Color::Reset),
                                Print("finished\n"),
                                MoveToColumn(0),
                            )
                            .unwrap();
                            break;
                        }
                    },
                    None => break,
                    _ => {},
                },
                channel_msg = channel.wait() => match channel_msg {
                    Some(next_msg) => {
                        match next_msg {
                            ChannelMsg::Data { ref data } => {
                                let bytes = data.as_ref();
                                let next_line = (str::from_utf8(bytes).expect("Invalid UTF-8")).trim_end();
                                execute!(
                                    stdout(),
                                    MoveUp(1),
                                    Clear(ClearType::CurrentLine),
                                    SetForegroundColor(Color::DarkGrey),
                                    Print("$ "),
                                    SetForegroundColor(Color::Reset),
                                    Print(next_line),
                                    Print("\n"),
                                    MoveToColumn(0),
                                    Clear(ClearType::CurrentLine),
                                    SetForegroundColor(Color::DarkGrey),
                                    Print("Remote console: "),
                                    SetForegroundColor(Color::Reset),
                                    Print("Press ESC to quit"),
                                    Print("\n"),
                                    MoveToColumn(0),
                                )
                                .unwrap();
                            }
                            ChannelMsg::ExitStatus { exit_status : _ } => {
                                // println!("Exit status: {}", exit_status);
                                break;
                            }
                            _ => {}
                        }
                    },
                    None => break,
                },
            }
        }

        disable_raw_mode().unwrap();

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
