use crate::actions::upload::upload;
use crate::config::Action;
use crate::{
    actions::commands::{create_ssh_session, send_command},
    config::Config,
    logger::Logger,
};
use chrono::Duration;
use colored::Colorize;
use openssh_sftp_client::Sftp;
use std::collections::HashSet;
use tokio::time::Instant;

pub async fn execute_actions(logger: &mut Logger, config: Config, skip: HashSet<String>) {
    let start_time = Instant::now();
    for action in &config.actions {
        match action {
            Action::Commands { name, commands } => {
                if skip.contains(name) {
                    continue;
                }
                let session = create_ssh_session(&config).await;
                send_command(&mut *logger, &session, commands).await;
                session.close().await.expect("Failed to close ssh session");
            }
            Action::Upload {
                name,
                source_folder,
                target_folder,
            } => {
                if skip.contains(name) {
                    continue;
                }
                let session = create_ssh_session(&config).await;
                let mut sftp = Sftp::from_session(session, Default::default())
                    .await
                    .expect("Unable to connect in SFTP");
                upload(
                    &config,
                    &mut *logger,
                    &mut sftp,
                    source_folder,
                    target_folder,
                )
                .await;
                sftp.close().await.expect("Failed to close sftp session");
            }
        }
    }

    let chrono_duration = Duration::seconds(start_time.elapsed().as_secs() as i64);
    let hours = chrono_duration.num_hours();
    let minutes = chrono_duration.num_minutes() % 60;
    let seconds = chrono_duration.num_seconds() % 60;
    let formatted_time = match (hours, minutes) {
        (0, 0) => format!("{}s", seconds),
        (0, _) => format!("{}m {}s", minutes, seconds),
        (_, _) => format!("{}h {}m {}s", hours, minutes, seconds),
    };

    println!("{} finished in {}", "Done:".bright_black(), formatted_time);
}
