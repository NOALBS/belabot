use std::{collections::HashMap, sync::Arc};

use tokio::{
    sync::{broadcast::Receiver, RwLock},
    task::JoinHandle,
    time::{self, Duration, Instant},
};
use tracing::{debug, info, warn};

use crate::{
    belabox,
    config::{self, BotCommand, Permission},
    error::Error,
    twitch::HandleMessage,
    Belabox, Settings, Twitch,
};

pub struct Bot {
    pub bb_msg_handle: JoinHandle<()>,
    pub tw_msg_handle: JoinHandle<()>,
    pub twitch: Arc<Twitch>,
    pub belabox: Arc<Belabox>,
    pub state: Arc<RwLock<State>>,
}

// TODO: Do I need this?
#[derive(Debug)]
pub struct State {
    pub belabox: Arc<RwLock<BelaState>>,
}

#[derive(Debug, Default)]
pub struct BelaState {
    online: bool,
    is_streaming: bool,
    config: Option<belabox::messages::Config>,
    netif: Option<HashMap<String, belabox::messages::Netif>>,
    sensors: Option<belabox::messages::Sensors>,
    notification_timeout: HashMap<String, time::Instant>,
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
            bela_state.clone(),
        ));

        let state = Arc::new(RwLock::new(State {
            belabox: bela_state,
        }));

        Ok(Self {
            bb_msg_handle,
            tw_msg_handle,
            twitch,
            belabox,
            state,
        })
    }
}

async fn handle_belabox_messages(
    mut bb_msg: Receiver<belabox::Message>,
    belabox: Arc<Belabox>,
    twitch: Arc<Twitch>,
    monitor: config::Monitor,
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
                if monitor.modems {
                    let read = bela_state.read().await;
                    if let Some(previous) = &read.netif {
                        let added = netif
                            .keys()
                            .filter(|&n| !previous.contains_key(n))
                            .map(|n| n.to_owned())
                            .collect::<Vec<String>>();

                        let removed = previous
                            .keys()
                            .filter(|&n| !netif.contains_key(n))
                            .map(|n| n.to_owned())
                            .collect::<Vec<String>>();

                        let mut message = Vec::new();

                        if !added.is_empty() {
                            let a = if added.len() > 1 { "are" } else { "is" };

                            message.push(format!("{} {} now connected", added.join(", "), a));
                        }

                        if !removed.is_empty() {
                            let a = if removed.len() > 1 { "have" } else { "has" };

                            message.push(format!("{} {} disconnected", removed.join(", "), a));
                        }

                        if !message.is_empty() {
                            twitch.send(format!("BB: {}", message.join(", "))).await;
                        }
                    }
                }

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
            }
            Message::Notification(notification) => {
                if monitor.notifications {
                    let mut lock = bela_state.write().await;
                    let timeout = &mut lock.notification_timeout;

                    let now = Instant::now();
                    for notification in notification.show {
                        if let Some(time) = timeout.get(&notification.name) {
                            if time.elapsed() < Duration::from_secs(30) {
                                continue;
                            }
                        }

                        warn!(notification.msg, "notication");

                        timeout
                            .entry(notification.name)
                            .and_modify(|n| *n = now)
                            .or_insert(now);

                        twitch.send("BB: ".to_owned() + &notification.msg).await;
                    }
                }
            }
            _ => {}
        }
    }
}

async fn handle_twitch_messages(
    mut tw_msg: Receiver<HandleMessage>,
    belabox: Arc<Belabox>,
    twitch: Arc<Twitch>,
    commands: HashMap<BotCommand, config::CommandInformation>,
    custom_interface_name: HashMap<String, String>,
    admins: Vec<String>,
    bela_state: Arc<RwLock<BelaState>>,
) {
    while let Ok(hm) = tw_msg.recv().await {
        debug!("Handle message: {:?}", hm);

        let mut split_message = hm.message.split_whitespace();

        // You can't send a blank message.. hopefully
        let command = split_message.next().unwrap().to_lowercase();
        let (command, info) = match commands.iter().find(|(_, info)| command == info.command) {
            Some((c, i)) => (c, i),
            None => {
                // No command found, continue
                continue;
            }
        };

        debug!(?command, "found command");

        let broadcaster = hm.broadcaster || admins.contains(&hm.sender_name);
        let moderator = broadcaster || hm.moderator;
        let vip = moderator || hm.vip;

        // Check user and command permission
        let is_allowed = match info.permission {
            Permission::Broadcaster => broadcaster,
            Permission::Moderator => moderator,
            Permission::Vip => vip,
            Permission::Public => true,
        };

        if !is_allowed {
            continue;
        };

        info!("{} used command {:?}", hm.sender_name, command);

        let online = {
            let read = bela_state.read().await;
            read.online
        };

        if !online {
            twitch.send("Offline :(".to_string()).await;
            continue;
        }

        let response = match command {
            BotCommand::Start => {
                let (config, is_streaming) = {
                    let read = bela_state.read().await;
                    (read.config.clone(), read.is_streaming)
                };

                let config = match config {
                    Some(c) => c,
                    None => {
                        twitch.send("Error starting BELABOX".to_string()).await;
                        continue;
                    }
                };

                if is_streaming {
                    twitch.send("Error already streaming".to_string()).await;
                    continue;
                }

                let request = belabox::requests::Start::from(config);
                belabox.start(request).await;

                "Starting BELABOX".to_string()
            }
            BotCommand::Stop => {
                let is_streaming = {
                    let read = bela_state.read().await;
                    read.is_streaming
                };

                if !is_streaming {
                    twitch.send("Error not streaming".to_string()).await;
                    continue;
                }

                belabox.stop().await;
                "Stopping BELABOX".to_string()
            }
            BotCommand::Stats => {
                let netifs = {
                    let read = bela_state.read().await;
                    read.netif.to_owned()
                };

                let mut total_bitrate = 0;
                let interfaces = netifs
                    .iter()
                    .flatten()
                    .map(|(mut name, i)| {
                        let value = if i.enabled {
                            let bitrate = (i.tp * 8) / 1024;
                            total_bitrate += bitrate;
                            format!("{} kbps", bitrate)
                        } else {
                            "disabled".to_string()
                        };

                        // Check if custom interface name based on interface
                        if let Some(custom) = custom_interface_name.get(name) {
                            name = custom;
                        }

                        // Check if custom interface name based on IP
                        if let Some(custom) = custom_interface_name.get(&i.ip) {
                            name = custom;
                        }

                        format!("{}: {}", name, value)
                    })
                    .collect::<Vec<String>>()
                    .join(", ");

                format!("{}, Total: {} kbps", interfaces, total_bitrate)
            }
            BotCommand::Restart => {
                belabox.restart().await;
                "Rebooting BELABOX".to_string()
            }
            BotCommand::Poweroff => {
                belabox.poweroff().await;
                "Powering off BELABOX".to_string()
            }
            BotCommand::Bitrate => {
                let bitrate = split_message.next();

                let bitrate = match bitrate {
                    Some(b) => b,
                    None => {
                        twitch.send("No bitrate given".to_string()).await;
                        continue;
                    }
                };

                let bitrate = match bitrate.parse::<u32>() {
                    Ok(b) => b,
                    Err(_) => {
                        twitch.send("Invalid number given".to_string()).await;
                        continue;
                    }
                };

                if !(500..=12000).contains(&bitrate) {
                    let msg = format!(
                        "Invalid value: {}, use a value between 500 - 12000",
                        bitrate
                    );
                    twitch.send(msg).await;
                    continue;
                }

                let increment = 250.0;
                let bitrate = (bitrate as f64 / increment as f64).round() * increment;

                let max_br = bitrate as u32;
                belabox.bitrate(max_br).await;

                {
                    let mut read = bela_state.write().await;
                    if let Some(config) = &mut read.config {
                        config.max_br = max_br;
                    }
                }

                format!("Changed max bitrate to {} kbps", bitrate)
            }
            BotCommand::Network => {
                let name = split_message.next();

                let name = match name {
                    Some(b) => b.to_lowercase(),
                    None => {
                        twitch.send("No interface given".to_string()).await;
                        continue;
                    }
                };

                let netifs = {
                    let read = bela_state.read().await;
                    read.netif.to_owned()
                };

                let netifs = match netifs {
                    Some(n) => n,
                    None => {
                        twitch.send("Interfaces not available".to_string()).await;
                        continue;
                    }
                };

                if netifs.len() == 1 {
                    twitch
                        .send("You only have one connection!".to_string())
                        .await;
                    continue;
                }

                let disabled_count = {
                    let mut total = 0;

                    for v in netifs.values() {
                        if !v.enabled {
                            total += 1;
                        }
                    }

                    total
                };

                let mut interface = netifs.get_key_value(&name);

                if interface.is_none() {
                    // get iterface name based on custom name
                    let mut possible_ip = None;

                    // Custom name based on interface
                    for (original, custom) in &custom_interface_name {
                        if name == custom.to_lowercase() {
                            interface = netifs.get_key_value(original);
                            possible_ip = Some(original);
                            break;
                        }
                    }

                    // Custom name based on ip
                    if interface.is_none() && possible_ip.is_some() {
                        let possible_ip = possible_ip.unwrap();

                        for (k, v) in &netifs {
                            if &v.ip == possible_ip {
                                interface = netifs.get_key_value(k);
                                break;
                            }
                        }
                    }
                }

                let (interface_name, interface) = match interface {
                    Some(i) => i,
                    None => {
                        twitch.send("Interface not found".to_string()).await;
                        continue;
                    }
                };

                if netifs.len() - disabled_count == 1 && interface.enabled {
                    twitch.send("Can't disable all networks".to_string()).await;
                    continue;
                }

                let enabled = !interface.enabled;
                let network = belabox::requests::Netif {
                    name: interface_name.to_owned(),
                    ip: interface.ip.to_owned(),
                    enabled,
                };
                belabox.netif(network).await;

                format!(
                    "{} has been {}",
                    name,
                    if enabled { "enabled" } else { "disabled" }
                )
            }
            BotCommand::Sensor => {
                let sensors = {
                    let read = bela_state.read().await;
                    read.sensors.to_owned()
                };

                let sensors = match sensors {
                    Some(s) => s,
                    None => {
                        twitch.send("Sensors not available".to_string()).await;
                        continue;
                    }
                };

                let belabox::messages::Sensors {
                    soc_voltage,
                    soc_current,
                    soc_temperature,
                } = sensors;

                let mut response = format!("Temp: {}", soc_temperature);

                if let Some(voltage) = soc_voltage {
                    response += &format!(", Voltage: {}", voltage);
                }

                if let Some(current) = soc_current {
                    response += &format!(", Amps: {}", current);
                }

                response
            }
        };

        twitch.send(response).await;
    }
}
