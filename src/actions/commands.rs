use std::sync::Arc;

use crate::config::Config;
use crate::log;
use crate::Logger;
use async_trait::async_trait;
use colored::Colorize;
use russh::client;
use russh::Channel;
use russh_keys::key::PublicKey;
use russh_keys::load_secret_key;

use super::upload::expand_user_path;

pub struct SimpleHandler;

#[async_trait]
impl client::Handler for SimpleHandler {
    type Error = russh::Error;

    async fn check_server_key(
        self,
        _server_public_key: &PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        Ok((self, true))
    }
}

pub async fn send_command(
    logger: &mut Logger,
    session: &client::Handle<SimpleHandler>,
    commands: &Vec<String>,
) {
    let forged_command = commands.join(" && ");
    log!(
        logger,
        "{}{}{}",
        "Dispatching: \'".bright_black(),
        forged_command.cyan(),
        "\'".bright_black()
    );

    let channel: Channel<client::Msg> = session.channel_open_session().await.unwrap();
    channel
        .exec(true, forged_command)
        .await
        .expect("Unable to send command");

    logger.start_remote_logging(channel).await;
}

pub async fn create_ssh_session(conf: &Config) -> client::Handle<SimpleHandler> {
    let key = load_secret_key(expand_user_path(&conf.server.ssh_key), None).unwrap();
    let config: Arc<_> = Arc::new(client::Config::default());
    let sh = SimpleHandler;

    let mut session = client::connect(config, (conf.server.host.to_owned(), conf.server.port), sh)
        .await
        .unwrap();
    session
        .authenticate_publickey(conf.server.user.to_owned(), Arc::new(key))
        .await
        .expect("Unable to connect via SSH");
    session
}
