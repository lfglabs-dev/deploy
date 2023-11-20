use crate::log;
use crate::Logger;
use crate::{
    config::Config,
    logger::{ANSI_ESCAPE_CODE, REMOTE_TERM_SIZE},
};
use colored::Colorize;
use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use futures::stream::StreamExt;
use openssh::{Session, SessionBuilder, Stdio};
use std::{
    collections::VecDeque,
    fs::File,
    io::Write,
    sync::{Arc, Mutex},
};
use tokio::io::AsyncRead;
use tokio::sync::Notify;
use tokio_util::codec::{FramedRead, LinesCodec};

pub async fn send_command(logger: &mut Logger, session: &Session, commands: &Vec<String>) {
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

pub async fn create_ssh_session(config: &Config) -> openssh::Session {
    SessionBuilder::default()
        .known_hosts_check(openssh::KnownHosts::Accept)
        .keyfile(&config.server.ssh_key)
        .user(config.server.user.to_owned())
        .connect(&config.server.host)
        .await
        .expect("Unable to connect in SSH")
}

pub fn handle_terminal_streaming<R, W: Write>(
    mut reader: FramedRead<R, LinesCodec>,
    buffer: Arc<Mutex<VecDeque<String>>>,
    mut writer: W,
    log_writer: Arc<tokio::sync::Mutex<File>>,
    notifier: Arc<Notify>,
) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: Send + 'static,
{
    tokio::spawn(async move {
        loop {
            tokio::select! {
                line = reader.next() => {
                    if line.is_none() {
                        break;
                    }
                    let line = line.unwrap().unwrap();
                    // not declared before to avoid locking the file if it sends both in stderr and stdout
                    let mut log_file = log_writer.lock().await;
                    if let Err(e) = writeln!(log_file, "$ {}", ANSI_ESCAPE_CODE.replace_all(&line, "")) {
                        eprintln!("Failed to write to log file: {}", e);
                    }

                    let mut accessible_buffer = buffer.lock().unwrap();
                    let prev_buffer_length: u16 = accessible_buffer.len().try_into().unwrap();

                    // todo: check if the line is an update of a previous line and update in place
                    if accessible_buffer.len() == REMOTE_TERM_SIZE.into() {
                        accessible_buffer.pop_front();
                    }
                    accessible_buffer.push_back(line);
                    // adding some space

                    writer.execute(MoveUp(prev_buffer_length + 1)).unwrap();
                    for line in accessible_buffer.iter() {
                        writer
                            .execute(Clear(ClearType::CurrentLine))
                            .unwrap()
                            .execute(MoveToColumn(0))
                            .unwrap();
                        println!("{}{}{}", "$".bright_black(), " ".clear(), line);
                    }
                    println!("{} Press enter to quit", "Remote console:".bright_green());
                    writer.flush().unwrap();
                }
                _ = notifier.notified() => {
                    // If notified, break the loop and exit
                    break;
                }
            }
        }
        notifier.notify_waiters();
    })
}
