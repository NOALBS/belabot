use std::sync::{Arc, Weak};

use thiserror::Error;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{error, info};
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{self, ServerMessage},
    transport::tcp::{TCPTransport, TLS},
    ClientConfig, SecureTCPTransport, TwitchIRCClient,
};

use crate::{config, error::Error};

#[derive(Error, Debug)]
pub enum TwitchError {
    #[error("disconnected from twitch")]
    Disconnected,
    #[error("twitch error")]
    TwitchIrc(#[from] twitch_irc::Error<TCPTransport<TLS>, StaticLoginCredentials>),
}

#[derive(Debug, Clone)]
pub struct HandleMessage {
    pub channel_name: String,
    pub sender_name: String,
    pub broadcaster: bool,
    pub moderator: bool,
    pub vip: bool,
    pub message: String,
}

pub struct Twitch {
    pub read_handle: JoinHandle<()>,
    pub client: TwitchIRCClient<TCPTransport<TLS>, StaticLoginCredentials>,
    message_tx: Weak<broadcast::Sender<HandleMessage>>,
    channel: String,
}

impl Twitch {
    pub async fn run(settings: config::Twitch) -> Result<Self, Error> {
        let config::Twitch {
            bot_username,
            bot_oauth,
            channel,
            ..
        } = settings;

        let username = bot_username.to_lowercase();
        let channel = channel.to_lowercase();
        let mut oauth = bot_oauth;

        if let Some(strip_oauth) = oauth.strip_prefix("oauth:") {
            oauth = strip_oauth.to_string();
        }

        let twitch_credentials = StaticLoginCredentials::new(username, Some(oauth));
        let twitch_config = ClientConfig::new_simple(twitch_credentials);
        let (mut incoming_messages, client) =
            TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(twitch_config);

        info!("Connected");

        let (tx, _) = broadcast::channel(100);
        let message_tx = Arc::new(tx);

        let tx_read = message_tx.clone();
        let read_handle = tokio::spawn(async move {
            while let Some(message) = incoming_messages.recv().await {
                match message {
                    ServerMessage::Notice(msg) => {
                        error!("{}", msg.message_text);
                        if msg.message_text == "Login authentication failed" {
                            break;
                        }
                    }
                    ServerMessage::Privmsg(msg) => {
                        let _ = tx_read.send(HandleMessage::from(msg));
                    }
                    _ => (),
                }
            }
        });

        client.join(channel.to_owned())?;

        Ok(Self {
            client,
            read_handle,
            message_tx: Arc::downgrade(&message_tx),
            channel,
        })
    }

    pub fn message_stream(&self) -> Result<broadcast::Receiver<HandleMessage>, TwitchError> {
        let tx = self.message_tx.upgrade().ok_or(TwitchError::Disconnected)?;

        Ok(tx.subscribe())
    }

    pub async fn send(&self, message: String) -> Result<(), TwitchError> {
        self.client
            .say(self.channel.to_owned(), message)
            .await
            .map_err(TwitchError::TwitchIrc)
    }
}

impl From<message::PrivmsgMessage> for HandleMessage {
    fn from(m: message::PrivmsgMessage) -> Self {
        let broadcaster = m.badges.contains(&message::Badge {
            name: "broadcaster".to_string(),
            version: "1".to_string(),
        });

        let moderator = m.badges.contains(&message::Badge {
            name: "moderator".to_string(),
            version: "1".to_string(),
        });

        let vip = m.badges.contains(&message::Badge {
            name: "vip".to_string(),
            version: "1".to_string(),
        });

        Self {
            channel_name: m.channel_login,
            sender_name: m.sender.login,
            broadcaster,
            moderator,
            vip,
            message: m.message_text,
        }
    }
}
