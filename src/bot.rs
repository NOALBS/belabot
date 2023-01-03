use std::{collections::HashMap, sync::Arc};

use tokio::{
    sync::{broadcast::Receiver, RwLock},
    task::JoinHandle,
    time,
};

use crate::{
    belabox,
    config::{self, BotCommand},
    error::Error,
    twitch::HandleMessage,
    Belabox, CommandHandler, Monitor, Settings, Twitch,
};

pub struct Bot {
    pub bb_msg_handle: JoinHandle<()>,
    pub bb_monitor_handle: JoinHandle<()>,
    pub tw_msg_handle: JoinHandle<()>,
    pub twitch: Arc<Twitch>,
    pub belabox: Arc<Belabox>,
}

#[derive(Debug, Default)]
pub struct BelaState {
    pub online: bool,
    pub is_streaming: bool,
    pub restart: bool,
    pub notify_ups: Option<bool>,
    pub config: Option<belabox::messages::Config>,
    pub netif: Option<HashMap<String, belabox::messages::Netif>>,
    pub sensors: Option<belabox::messages::Sensors>,
    pub notification_timeout: HashMap<String, time::Instant>,
    pub pipelines: Option<HashMap<String, belabox::messages::Pipeline>>,
}

impl Bot {
    pub async fn new(config: Settings) -> Result<Self, Error> {
        let twitch = Arc::new(Twitch::run(config.twitch.clone()).await?);
        let belabox = Arc::new(Belabox::connect(config.belabox.remote_key.to_owned()).await?);

        // Create state to store BELABOX information
        let bela_state = Arc::new(RwLock::new(BelaState::default()));

        // Read BELABOX messages
        let bb_msg_handle = tokio::spawn(handle_belabox_messages(
            belabox.message_stream()?,
            belabox.clone(),
            twitch.clone(),
            bela_state.clone(),
        ));

        let bb_monitor_handle = tokio::spawn(handle_belabox_monitor(
            belabox.message_stream()?,
            belabox.clone(),
            twitch.clone(),
            config.belabox.monitor,
            bela_state.clone(),
        ));

        // Read Twitch messages
        let tw_msg_handle = tokio::spawn(handle_twitch_messages(
            twitch.message_stream()?,
            belabox.clone(),
            twitch.clone(),
            config.commands,
            config.belabox.custom_interface_name,
            config.twitch.admins,
            bela_state,
        ));

        Ok(Self {
            bb_msg_handle,
            bb_monitor_handle,
            tw_msg_handle,
            twitch,
            belabox,
        })
    }
}

async fn handle_belabox_messages(
    mut bb_msg: Receiver<belabox::Message>,
    belabox: Arc<Belabox>,
    twitch: Arc<Twitch>,
    bela_state: Arc<RwLock<BelaState>>,
) {
    use belabox::Message;

    while let Ok(message) = bb_msg.recv().await {
        match message {
            Message::Config(config) => {
                let mut lock = bela_state.write().await;
                lock.config = Some(config);
            }
            Message::RemoteEncoder(remote) => {
                let mut lock = bela_state.write().await;
                lock.online = remote.is_encoder_online
            }
            Message::Netif(netif) => {
                let mut lock = bela_state.write().await;
                lock.netif = Some(netif);
            }
            Message::Sensors(sensors) => {
                let mut lock = bela_state.write().await;
                lock.sensors = Some(sensors);
            }
            Message::Bitrate(bitrate) => {
                let mut lock = bela_state.write().await;
                if let Some(config) = &mut lock.config {
                    config.max_br = bitrate.max_br;
                }
            }
            Message::StreamingStatus(status) => {
                let mut lock = bela_state.write().await;
                lock.is_streaming = status.is_streaming;
            }
            Message::Status(status) => {
                let mut lock = bela_state.write().await;
                lock.is_streaming = status.is_streaming;

                if lock.restart {
                    lock.restart = false;

                    if let Some(config) = &lock.config {
                        let request = belabox::requests::Start::from(config.to_owned());
                        let _ = belabox.start(request).await;

                        let msg = "BB: Reboot successful, starting the stream".to_string();
                        let _ = twitch.send(msg).await;
                    }
                }
            }
            _ => {}
        }
    }
}

async fn handle_belabox_monitor(
    bb_msg: Receiver<belabox::Message>,
    belabox: Arc<Belabox>,
    twitch: Arc<Twitch>,
    monitor: config::Monitor,
    bela_state: Arc<RwLock<BelaState>>,
) {
    let handler = Monitor {
        belabox,
        bela_state,
        twitch,
    };
    handler.run(bb_msg, monitor).await;
}

async fn handle_twitch_messages(
    tw_msg: Receiver<HandleMessage>,
    belabox: Arc<Belabox>,
    twitch: Arc<Twitch>,
    commands: HashMap<BotCommand, config::CommandInformation>,
    custom_interface_name: HashMap<String, String>,
    admins: Vec<String>,
    bela_state: Arc<RwLock<BelaState>>,
) {
    let handler = CommandHandler {
        twitch,
        belabox,
        bela_state,
        commands,
        custom_interface_name,
        admins,
    };
    handler.run(tw_msg).await;
}
