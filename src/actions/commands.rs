use crate::config::Config;
use crate::log;
use crate::Logger;
use colored::Colorize;
use openssh::{Session, SessionBuilder, Stdio};

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
