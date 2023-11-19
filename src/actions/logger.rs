use crate::actions::commands::handle_terminal_streaming;
use colored::Colorize;
use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use std::{
    collections::VecDeque,
    io::{stdout, Write},
    sync::{Arc, Mutex},
};
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::Notify;
use tokio_util::codec::{FramedRead, LinesCodec};

pub struct Logger {
    remote_buffer: Arc<Mutex<VecDeque<String>>>,
}

impl Logger {
    pub fn new() -> Logger {
        Logger {
            remote_buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn log(&mut self, message: String) {
        println!("{}", message);
    }

    pub async fn start_remote_logging(&mut self, mut command: openssh::Child<&openssh::Session>) {
        self.remote_buffer = Arc::new(Mutex::new(VecDeque::new()));
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
            notifier.clone(),
        );
        let stderr_handle = handle_terminal_streaming(
            stderr_reader,
            Arc::clone(&self.remote_buffer),
            stdout(),
            notifier.clone(),
        );

        // Await the tasks
        let _ = tokio::try_join!(handle_notifier, stdout_handle, stderr_handle)
            .expect("Failed to start remote streaming");
    }
}

#[macro_export]
macro_rules! log {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log(format!($($arg)*));
    };
}
