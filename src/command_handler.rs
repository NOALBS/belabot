use std::fmt::Write as _;
use std::{collections::HashMap, sync::Arc};

use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info};

use crate::{
    belabox::{self, BelaboxError},
    bot::BelaState,
    config::{self, BotCommand, Permission},
    error::{Error, Result},
    twitch, Belabox, Twitch,
};

pub struct CommandHandler {
    pub twitch: Arc<Twitch>,
    pub belabox: Arc<Belabox>,
    pub bela_state: Arc<RwLock<BelaState>>,
    pub commands: HashMap<config::BotCommand, config::CommandInformation>,
    pub custom_interface_name: HashMap<String, String>,
    pub admins: Vec<String>,
}

impl CommandHandler {
    pub async fn run(&self, mut messages: broadcast::Receiver<twitch::HandleMessage>) {
        while let Ok(hm) = messages.recv().await {
            debug!("Handle message: {:?}", hm);

            let mut split_message = hm.message.split_whitespace();

            // You can't send a blank message.. hopefully
            let command = split_message.next().unwrap().to_lowercase();
            let (command, info) = match self.command(command) {
                Some(c) => c,
                None => continue,
            };
            debug!(?command, "found command");

            if !self.is_allowed_to_execute(&info.permission, &hm) {
                continue;
            };

            info!("{} used command {:?}", hm.sender_name, command);

            if !{ self.bela_state.read().await.online } {
                self.send("Offline :(".to_string()).await;
                continue;
            }

            let response = match command {
                BotCommand::AudioDelay => self.audio_delay(split_message.next()).await,
                BotCommand::Bitrate => self.bitrate(split_message.next()).await,
                BotCommand::Network => self.network(split_message.next()).await,
                BotCommand::Poweroff => self.poweroff().await,
                BotCommand::Restart => self.restart().await,
                BotCommand::Sensor => self.sensor().await,
                BotCommand::Start => self.start().await,
                BotCommand::Stats => self.stats().await,
                BotCommand::Stop => self.stop().await,
                BotCommand::Latency => self.latency(split_message.next()).await,
            };

            match response {
                Ok(message) => self.send(message).await,
                Err(e) => self.send(format!("Error {}", e)).await,
            }
        }
    }

    async fn send(&self, message: String) {
        if let Err(e) = self.twitch.send(message).await {
            error!(?e, "error sending message to twitch");
        }
    }

    fn command(
        &self,
        command: String,
    ) -> Option<(&config::BotCommand, &config::CommandInformation)> {
        self.commands
            .iter()
            .find(|(_, info)| command == info.command)
    }

    fn is_allowed_to_execute(
        &self,
        permission: &config::Permission,
        handle_message: &twitch::HandleMessage,
    ) -> bool {
        let twitch::HandleMessage {
            sender_name,
            broadcaster,
            moderator,
            vip,
            ..
        } = handle_message;

        let broadcaster = *broadcaster || self.admins.contains(sender_name);
        let moderator = broadcaster || *moderator;
        let vip = moderator || *vip;

        match permission {
            Permission::Broadcaster => broadcaster,
            Permission::Moderator => moderator,
            Permission::Vip => vip,
            Permission::Public => true,
        }
    }

    pub async fn start(&self) -> Result<String> {
        let (config, is_streaming) = {
            let read = self.bela_state.read().await;
            (read.config.clone(), read.is_streaming)
        };

        let config = match config {
            Some(c) => c,
            None => {
                return Ok("Error starting BELABOX".to_string());
            }
        };

        if is_streaming {
            return Ok("Error already streaming".to_string());
        }

        let request = belabox::requests::Start::from(config);
        self.belabox.start(request).await?;

        Ok("Starting BELABOX".to_string())
    }

    pub async fn stop(&self) -> Result<String> {
        if !{ self.bela_state.read().await.is_streaming } {
            return Ok("Error not streaming".to_string());
        }

        self.belabox.stop().await?;
        Ok("Stopping BELABOX".to_string())
    }

    pub async fn stats(&self) -> Result<String> {
        let (netifs, ups) = {
            let read = self.bela_state.read().await;
            (read.netif.to_owned(), read.notify_ups)
        };

        let mut total_bitrate = 0;
        let mut interfaces = netifs
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
                if let Some(custom) = self.custom_interface_name.get(name) {
                    name = custom;
                }

                // Check if custom interface name based on IP
                if let Some(custom) = self.custom_interface_name.get(&i.ip) {
                    name = custom;
                }

                format!("{}: {}", name, value)
            })
            .collect::<Vec<String>>();

        // Sort interfaces because they like to move around
        interfaces.sort();

        let mut msg = format!("{}, Total: {} kbps", interfaces.join(", "), total_bitrate);

        if let Some(connected) = ups {
            let a = if !connected { "not" } else { "" };
            let _ = write!(msg, ", UPS: {} charging", a);
        }

        Ok(msg)
    }

    pub async fn restart(&self) -> Result<String> {
        let is_streaming = {
            let mut lock = self.bela_state.write().await;

            if lock.restart {
                return Err(Error::Belabox(BelaboxError::AlreadyRestarting));
            }

            if lock.is_streaming {
                lock.restart = true;
            }

            lock.is_streaming
        };

        if is_streaming {
            self.belabox.stop().await?;
        }

        self.belabox.restart().await?;
        Ok("Rebooting BELABOX".to_string())
    }

    pub async fn poweroff(&self) -> Result<String> {
        self.belabox.poweroff().await?;
        Ok("Powering off BELABOX".to_string())
    }

    pub async fn bitrate(&self, bitrate: Option<&str>) -> Result<String> {
        let bitrate = match bitrate {
            Some(b) => b,
            None => {
                return Ok("No bitrate given".to_string());
            }
        };

        let bitrate = match bitrate.parse::<u32>() {
            Ok(b) => b,
            Err(_) => {
                return Ok(format!("Invalid number {} given", bitrate));
            }
        };

        if !(500..=12000).contains(&bitrate) {
            let msg = format!(
                "Invalid value: {}, use a value between 500 - 12000",
                bitrate
            );
            return Ok(msg);
        }

        let bitrate = increment_by_step(bitrate as f64, 250.0) as u32;
        self.belabox.bitrate(bitrate).await?;

        {
            let mut read = self.bela_state.write().await;
            if let Some(config) = &mut read.config {
                config.max_br = bitrate;
            }
        }

        Ok(format!("Changed max bitrate to {} kbps", bitrate))
    }

    pub async fn network(&self, name: Option<&str>) -> Result<String> {
        let name = match name {
            Some(b) => b.to_lowercase(),
            None => {
                return Ok("No interface given".to_string());
            }
        };

        let netifs = {
            let read = self.bela_state.read().await;
            read.netif.to_owned()
        };

        let netifs = match netifs {
            Some(n) => n,
            None => {
                return Ok("Interfaces not available".to_string());
            }
        };

        if netifs.len() == 1 {
            return Ok("You only have one connection!".to_string());
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
            for (original, custom) in &self.custom_interface_name {
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
                return Ok("Interface not found".to_string());
            }
        };

        if netifs.len() - disabled_count == 1 && interface.enabled {
            return Ok("Can't disable all networks".to_string());
        }

        let enabled = !interface.enabled;
        let network = belabox::requests::Netif {
            name: interface_name.to_owned(),
            ip: interface.ip.to_owned(),
            enabled,
        };
        self.belabox.netif(network).await?;

        Ok(format!(
            "{} has been {}",
            name,
            if enabled { "enabled" } else { "disabled" }
        ))
    }

    pub async fn sensor(&self) -> Result<String> {
        let sensors = {
            let read = self.bela_state.read().await;
            read.sensors.to_owned()
        };

        let sensors = match sensors {
            Some(s) => s,
            None => {
                return Ok("Sensors not available".to_string());
            }
        };

        let belabox::messages::Sensors {
            soc_voltage,
            soc_current,
            soc_temperature,
        } = sensors;

        let mut response = format!("Temp: {}", soc_temperature);

        if let Some(voltage) = soc_voltage {
            let _ = write!(response, ", Voltage: {}", voltage);
        }

        if let Some(current) = soc_current {
            let _ = write!(response, ", Amps: {}", current);
        }

        Ok(response)
    }

    pub async fn latency(&self, latency: Option<&str>) -> Result<String> {
        let latency = match latency {
            Some(b) => b,
            None => {
                let current_latency = {
                    self.bela_state
                        .read()
                        .await
                        .config
                        .as_ref()
                        .map(|config| config.srt_latency)
                };

                let latency = if let Some(current) = current_latency {
                    current.to_string()
                } else {
                    "unknown".to_string()
                };

                return Ok(format!("Current SRT latency is {} ms", latency));
            }
        };

        let latency = match latency.parse::<u64>() {
            Ok(l) => l,
            Err(_) => {
                return Ok(format!("Invalid number {} given", latency));
            }
        };

        if !(100..=4000).contains(&latency) {
            let msg = format!("Invalid value: {}, use a value between 100 - 4000", latency);
            return Ok(msg);
        }

        let latency = increment_by_step(latency as f64, 100.0);
        let is_streaming = { self.bela_state.read().await.is_streaming };

        if is_streaming {
            let _ = self.stop().await?;
            self.send("Restarting the stream".to_string()).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await
        }

        {
            let mut lock = self.bela_state.write().await;

            if let Some(config) = &mut lock.config {
                config.srt_latency = latency as u64;
            }
        }

        if is_streaming {
            let _ = self.start().await?;
        }

        Ok(format!("Changed SRT latency to {} ms", latency))
    }

    pub async fn audio_delay(&self, delay: Option<&str>) -> Result<String> {
        let delay = match delay {
            Some(b) => b,
            None => {
                let current_delay = {
                    self.bela_state
                        .read()
                        .await
                        .config
                        .as_ref()
                        .map(|config| config.delay)
                };

                let delay = if let Some(current) = current_delay {
                    current.to_string()
                } else {
                    "unknown".to_string()
                };

                return Ok(format!("Current audio delay is {} ms", delay));
            }
        };

        let delay = match delay.parse::<i32>() {
            Ok(l) => l,
            Err(_) => {
                return Ok(format!("Invalid number {} given", delay));
            }
        };

        if delay.abs() > 2000 {
            let msg = format!("Invalid value: {}, use a value between -2000 - 2000", delay);
            return Ok(msg);
        }

        let delay = increment_by_step(delay, 20.0);
        let is_streaming = { self.bela_state.read().await.is_streaming };

        if is_streaming {
            let _ = self.stop().await?;
            self.send("Restarting the stream".to_string()).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await
        }

        {
            let mut lock = self.bela_state.write().await;

            if let Some(config) = &mut lock.config {
                config.delay = delay as i32;
            }
        }

        if is_streaming {
            let _ = self.start().await?;
        }

        Ok(format!("Changed audio delay to {} ms", delay))
    }
}

fn increment_by_step<V, S>(value: V, step: S) -> f64
where
    V: Into<f64>,
    S: Into<f64>,
{
    let value = value.into();
    let step = step.into();

    (value / step).round() * step
}
