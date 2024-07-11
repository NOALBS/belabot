use std::{collections::HashMap, sync::Arc};

use tokio::{
    sync::{broadcast::Receiver, Mutex, RwLock},
    task::JoinHandle,
    time::{self, Instant},
};

use crate::{
    belabox::{
        self,
        messages::{Remote, StatusKind},
    },
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

#[derive(Debug)]
pub struct BelaState {
    pub online: bool,
    pub is_streaming: bool,
    pub restart: bool,
    pub notify_ups: Option<bool>,
    pub config: Option<belabox::messages::Config>,
    pub netif: Option<HashMap<String, belabox::messages::Netif>>,
    pub sensors: Option<belabox::messages::Sensors>,
    pub notification_timeout: HashMap<String, time::Instant>,
    pub network_timeout: time::Instant,
    pub pipelines: Option<HashMap<String, belabox::messages::Pipeline>>,
    pub asrcs: Option<Vec<String>>,
}

impl Default for BelaState {
    fn default() -> Self {
        Self {
            network_timeout: Instant::now(),
            online: Default::default(),
            is_streaming: Default::default(),
            restart: Default::default(),
            notify_ups: Default::default(),
            config: Default::default(),
            netif: Default::default(),
            sensors: Default::default(),
            notification_timeout: Default::default(),
            pipelines: Default::default(),
            asrcs: Default::default(),
        }
    }
}

impl Bot {
    pub async fn new(config: Settings) -> Result<Self, Error> {
        let twitch = Arc::new(Twitch::run(config.twitch.clone()).await?);
        let belabox = Arc::new(Belabox::connect(config.belabox.remote_key.to_owned()).await?);

        // Create state to store BELABOX information
        let bela_state = Arc::new(RwLock::new(BelaState::default()));

        // Access to the command handler
        let command_handler = Arc::new(Mutex::new(None));

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
            command_handler.clone(),
            config.belabox.custom_interface_name.clone(),
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
            command_handler,
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
            Message::Remote(Remote::RemoteEncoder(remote)) => {
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
            Message::Status(status) => {
                let mut lock = bela_state.write().await;

                match status {
                    StatusKind::Status(s) => {
                        lock.is_streaming = s.is_streaming;
                        lock.asrcs = Some(s.asrcs);
                    }
                    StatusKind::Asrcs(a) => {
                        lock.asrcs = Some(a.asrcs);
                    }
                    StatusKind::StreamingStatus(ss) => {
                        lock.is_streaming = ss.is_streaming;
                    }
                    StatusKind::Wifi(_) => {}
                    StatusKind::AvailableUpdates(_) => {}
                };

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
            Message::Pipelines(pipelines) => {
                let mut lock = bela_state.write().await;
                lock.pipelines = Some(pipelines);
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
    command_handler: Arc<Mutex<Option<CommandHandler>>>,
    custom_interface_name: HashMap<String, String>,
) {
    let handler = Monitor {
        belabox,
        bela_state,
        twitch,
        command_handler,
        custom_interface_name,
    };
    handler.run(bb_msg, monitor).await;
}

#[allow(clippy::too_many_arguments)]
async fn handle_twitch_messages(
    tw_msg: Receiver<HandleMessage>,
    belabox: Arc<Belabox>,
    twitch: Arc<Twitch>,
    commands: HashMap<BotCommand, config::CommandInformation>,
    custom_interface_name: HashMap<String, String>,
    admins: Vec<String>,
    bela_state: Arc<RwLock<BelaState>>,
    command_handler: Arc<Mutex<Option<CommandHandler>>>,
) {
    let handler = CommandHandler {
        twitch,
        belabox,
        bela_state,
        commands,
        custom_interface_name,
        admins,
    };
    *command_handler.lock().await = Some(handler.clone());
    handler.run(tw_msg).await;
}
