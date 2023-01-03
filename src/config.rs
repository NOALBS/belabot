use std::collections::HashMap;

use read_input::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::error;

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO Error")]
    Io(#[from] std::io::Error),
    #[error("Json error: {0}")]
    Json(#[from] serde_json::error::Error),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Settings {
    pub belabox: Belabox,
    pub twitch: Twitch,
    pub commands: HashMap<BotCommand, CommandInformation>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Belabox {
    pub remote_key: String,
    pub custom_interface_name: HashMap<String, String>,
    pub monitor: Monitor,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Monitor {
    pub modems: bool,
    pub notifications: bool,
    pub ups: bool,
    pub ups_plugged_in: f64,
}

impl Default for Monitor {
    fn default() -> Self {
        Self {
            modems: true,
            notifications: true,
            ups: false,
            ups_plugged_in: 5.1,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Twitch {
    pub bot_username: String,
    pub bot_oauth: String,
    pub channel: String,
    pub admins: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandInformation {
    pub command: String,
    pub permission: Permission,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub enum BotCommand {
    AudioDelay,
    AudioSrc,
    Bitrate,
    Latency,
    Network,
    Pipeline,
    Poweroff,
    Restart,
    Sensor,
    Start,
    Stats,
    Stop,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Permission {
    Broadcaster,
    Moderator,
    Vip,
    Public,
}

impl Settings {
    /// Loads the config
    pub fn load<P>(path: P) -> Result<Self, ConfigError>
    where
        P: AsRef<std::path::Path>,
    {
        let file = std::fs::read_to_string(path)?;
        let mut config = match serde_json::from_str::<Settings>(&file) {
            Ok(c) => c,
            Err(e) => {
                error!(%e, "config error");
                return Err(ConfigError::Json(e));
            }
        };

        // Lowercase important settings such as the twitch channel name to
        // avoid issues.
        lowercase_settings(&mut config);

        // Insert chat commands in the config if they don't exist.
        default_chat_commands(&mut config.commands);

        std::fs::write(CONFIG_FILE_NAME, serde_json::to_string_pretty(&config)?)?;

        Ok(config)
    }

    pub async fn ask_for_settings() -> Result<Self, ConfigError> {
        println!("Please paste your BELABOX Cloud remote URL below");

        let remote_key: String = input()
            .msg("URL: ")
            .add_err_test(
                |u: &String| u.contains("?key="),
                "No key found, please try again",
            )
            .get()
            .split("?key=")
            .nth(1)
            .expect("No key found")
            .to_string();

        let mut custom_interface_name = HashMap::new();
        custom_interface_name.insert("eth0".to_string(), "eth0".to_string());
        custom_interface_name.insert("usb0".to_string(), "usb0".to_string());
        custom_interface_name.insert("wlan0".to_string(), "wlan0".to_string());

        println!("\nDo you want to receive automatic chat messages about:");

        let is_y_or_n = |x: &String| x.to_lowercase() == "y" || x.to_lowercase() == "n";
        let mut monitor = Monitor {
            modems: input_to_bool(
                input()
                    .msg("The status of your modems (Y/n): ")
                    .add_test(is_y_or_n)
                    .err("Please enter y or n: ")
                    .default("y".to_string())
                    .get(),
            ),
            notifications: input_to_bool(
                input()
                    .msg("The belaUI notifications (Y/n): ")
                    .add_test(is_y_or_n)
                    .err("Please enter y or n: ")
                    .default("y".to_string())
                    .get(),
            ),
            ups: input_to_bool(
                input()
                    .msg("The status of your UPS (y/N): ")
                    .add_test(is_y_or_n)
                    .err("Please enter y or n: ")
                    .default("n".to_string())
                    .get(),
            ),
            ups_plugged_in: 5.1,
        };

        if monitor.ups {
            monitor.ups_plugged_in = input()
                .msg("UPS charging threshold (default 5.1 V): ")
                .err("Please enter a number")
                .default(5.1)
                .get();
        }

        let belabox = Belabox {
            remote_key,
            custom_interface_name,
            monitor,
        };

        println!("\nPlease enter your Twitch details below");
        let mut twitch = Twitch {
            bot_username: input().msg("Bot username: ").get(),
            bot_oauth: input()
                .msg("(You can generate an Oauth here: https://twitchapps.com/tmi/)\nBot oauth: ")
                .get(),
            channel: input().msg("Channel name: ").get(),
            admins: Vec::new(),
        };

        let admins = input::<String>()
            .msg("Admin users (separate multiple names by a comma): ")
            .get();

        if !admins.is_empty() {
            for admin in admins.split(',') {
                twitch.admins.push(admin.trim().to_lowercase());
            }
        }

        let mut commands = HashMap::new();
        default_chat_commands(&mut commands);

        let mut settings = Self {
            belabox,
            twitch,
            commands,
        };

        std::fs::write(CONFIG_FILE_NAME, serde_json::to_string_pretty(&settings)?)?;

        // FIXME: Does not work on windows
        print!("\x1B[2J");

        let mut path = std::env::current_dir()?;
        path.push(CONFIG_FILE_NAME);
        println!(
            "Saved settings to {} in {}",
            CONFIG_FILE_NAME,
            path.display()
        );

        lowercase_settings(&mut settings);

        Ok(settings)
    }
}

/// Lowercase settings which should always be lowercase
fn lowercase_settings(settings: &mut Settings) {
    let Twitch {
        bot_username,
        bot_oauth,
        channel,
        admins,
        ..
    } = &mut settings.twitch;

    *channel = channel.to_lowercase();
    *bot_oauth = bot_oauth.to_lowercase();
    *bot_username = bot_username.to_lowercase();

    for user in admins {
        *user = user.to_lowercase();
    }

    for info in settings.commands.values_mut() {
        info.command = info.command.to_lowercase();
    }
}

/// Converts y or n to bool.
fn input_to_bool(confirm: String) -> bool {
    confirm.to_lowercase() == "y"
}

// Insert default commands if they don't exist
fn default_chat_commands(commands: &mut HashMap<BotCommand, CommandInformation>) {
    commands
        .entry(BotCommand::Start)
        .or_insert(CommandInformation {
            command: "!bbstart".to_string(),
            permission: Permission::Broadcaster,
        });

    commands
        .entry(BotCommand::Stop)
        .or_insert(CommandInformation {
            command: "!bbstop".to_string(),
            permission: Permission::Broadcaster,
        });

    commands
        .entry(BotCommand::Stats)
        .or_insert(CommandInformation {
            command: "!bbs".to_string(),
            permission: Permission::Public,
        });

    commands
        .entry(BotCommand::Restart)
        .or_insert(CommandInformation {
            command: "!bbrs".to_string(),
            permission: Permission::Broadcaster,
        });

    commands
        .entry(BotCommand::Poweroff)
        .or_insert(CommandInformation {
            command: "!bbpo".to_string(),
            permission: Permission::Broadcaster,
        });

    commands
        .entry(BotCommand::Bitrate)
        .or_insert(CommandInformation {
            command: "!bbb".to_string(),
            permission: Permission::Broadcaster,
        });

    commands
        .entry(BotCommand::Sensor)
        .or_insert(CommandInformation {
            command: "!bbsensor".to_string(),
            permission: Permission::Public,
        });

    commands
        .entry(BotCommand::Network)
        .or_insert(CommandInformation {
            command: "!bbt".to_string(),
            permission: Permission::Broadcaster,
        });

    commands
        .entry(BotCommand::Latency)
        .or_insert(CommandInformation {
            command: "!bbl".to_string(),
            permission: Permission::Broadcaster,
        });

    commands
        .entry(BotCommand::AudioDelay)
        .or_insert(CommandInformation {
            command: "!bbd".to_string(),
            permission: Permission::Broadcaster,
        });

    commands
        .entry(BotCommand::Pipeline)
        .or_insert(CommandInformation {
            command: "!bbp".to_string(),
            permission: Permission::Broadcaster,
        });

    commands
        .entry(BotCommand::AudioSrc)
        .or_insert(CommandInformation {
            command: "!bba".to_string(),
            permission: Permission::Broadcaster,
        });
}
