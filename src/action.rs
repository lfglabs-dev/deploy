use crate::config::Config;
use colored::Colorize;
use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use futures::stream::StreamExt;
use openssh::{SessionBuilder, Stdio};
use std::{
    collections::{HashSet, VecDeque},
    io::{stdout, Write},
    sync::{Arc, Mutex},
};
use tokio::io::AsyncRead;
use tokio_util::codec::{FramedRead, LinesCodec};

const REMOTE_TERM_SIZE: usize = 10;

async fn create_ssh_session(config: &Config) -> openssh::Session {
    SessionBuilder::default()
        .known_hosts_check(openssh::KnownHosts::Accept)
        .keyfile(&config.server.ssh_key)
        .user(config.server.user.to_owned())
        .connect(&config.server.host)
        .await
        .expect("Unable to connect in SSH")
}

fn handle_terminal_streaming<R, W: Write>(
    reader: FramedRead<R, LinesCodec>,
    buffer: Arc<Mutex<VecDeque<String>>>,
    mut writer: W,
) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: Send + 'static,
{
    tokio::spawn(async move {
        reader
            .for_each(|line| {
                let line = line.unwrap();
                let mut accessible_buffer = buffer.lock().unwrap();
                let prev_buffer_length: u16 = accessible_buffer.len().try_into().unwrap();

                // todo: check if the line is an update of a previous line and update in place
                if accessible_buffer.len() == REMOTE_TERM_SIZE.into() {
                    accessible_buffer.pop_front();
                }
                accessible_buffer.push_back(line);
                // adding some space
                if prev_buffer_length > 0 {
                    writer.execute(MoveUp(prev_buffer_length)).unwrap();
                }
                for line in accessible_buffer.iter() {
                    writer
                        .execute(Clear(ClearType::CurrentLine))
                        .unwrap()
                        .execute(MoveToColumn(0))
                        .unwrap();
                    println!("{}{}{}", "$".bright_black(), " ".clear(), line);
                }
                writer.flush().unwrap();
                futures::future::ready(())
            })
            .await;
    })
}

struct Logger {
    remote_buffer: Arc<Mutex<VecDeque<String>>>,
}

impl Logger {
    fn new() -> Logger {
        Logger {
            remote_buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn log(&mut self, message: String) {
        println!("{}", message);
    }

    async fn start_remote_logging(&mut self, mut command: openssh::Child<&openssh::Session>) {
        self.remote_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let stdout_reader = FramedRead::new(
            command.stdout().take().expect("Failed to open stdout"),
            LinesCodec::new(),
        );
        let stderr_reader = FramedRead::new(
            command.stderr().take().expect("Failed to open stderr"),
            LinesCodec::new(),
        );

        let stdout_handle =
            handle_terminal_streaming(stdout_reader, Arc::clone(&self.remote_buffer), stdout());
        let stderr_handle =
            handle_terminal_streaming(stderr_reader, Arc::clone(&self.remote_buffer), stdout());

        // Await the tasks
        let _ = tokio::try_join!(stdout_handle, stderr_handle)
            .expect("Failed to start remote streaming");
    }
}

#[macro_export]
macro_rules! log {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log(format!($($arg)*));
    };
}

pub async fn execute_actions(config: Config, skip: HashSet<String>) {
    let mut logger = Logger::new();

    for (action_name, action) in &config.actions {
        if skip.contains(action_name) {
            continue;
        }
        let session = create_ssh_session(&config).await;

        if let Some(commands) = &action.commands {
            let forged_command = commands.join(" && ");
            log!(
                logger,
                "{}{}{}",
                "Dispatching: \'".bright_black(),
                forged_command.cyan(),
                "\'".bright_black()
            );

            let command = session
                .raw_command(forged_command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .await
                .expect("Unable to send command");

            logger.start_remote_logging(command).await;
        }

        match (&action.source_folder, &action.target_folder) {
            (Some(source_folder), Some(target_folder)) => {
                log!(
                    logger,
                    "{}{}{}{}{}",
                    "Uploading: '".bright_black(),
                    source_folder.blue(),
                    "' to '".bright_black(),
                    target_folder.blue(),
                    "'".bright_black()
                );
            }
            (_, _) => {}
        }
    }
    println!("{}", "Done".bright_black());
}
