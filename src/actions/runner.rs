use crate::actions::upload::upload;
use crate::{
    actions::{
        commands::{create_ssh_session, send_command},
        logger::Logger,
    },
    config::Config,
};
use chrono::Duration;
use colored::Colorize;
use openssh_sftp_client::Sftp;
use std::collections::HashSet;
use tokio::time::Instant;

pub async fn execute_actions(config: Config, skip: HashSet<String>) {
    let mut logger = Logger::new();
    let start_time = Instant::now();
    for (action_name, action) in &config.actions {
        if skip.contains(action_name) {
            continue;
        }
        let session = create_ssh_session(&config).await;

        if let Some(commands) = &action.commands {
            send_command(&mut logger, &session, commands).await;
        }

        match (&action.source_folder, &action.target_folder) {
            (Some(source_folder), Some(target_folder)) => {
                let mut sftp = Sftp::from_session(session, Default::default())
                    .await
                    .expect("Unable to connect in SFTP");
                upload(&config, &mut logger, &mut sftp, source_folder, target_folder).await;
                sftp.close().await.expect("Failed to close sftp session");
            }
            (_, _) => {
                session.close().await.expect("Failed to close ssh session");
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
