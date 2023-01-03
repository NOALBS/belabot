use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", untagged)]
pub enum Message {
    Config(Config),
    RemoteAuth(RemoteAuth),
    RemoteEncoder(RemoteEncoder),
    Netif(HashMap<String, Netif>),
    Revisions(Revisions),
    Sensors(Sensors),
    Status(Status),
    Updating(Updating),
    Wifi(WifiChange),
    StreamingStatus(StreamingStatus),
    Notification(Notification),
    Bitrate(Bitrate),
    Pipelines(HashMap<String, Pipeline>),
    Acodecs(Acodecs),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteAuth {
    #[serde(rename = "auth/key")]
    pub auth_key: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteEncoder {
    pub is_encoder_online: bool,
    pub version: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub password_hash: String,
    pub remote_key: String,
    pub max_br: u32,
    pub delay: i32,
    pub pipeline: String,
    pub srt_latency: u64,
    pub srt_streamid: String,
    pub srtla_addr: String,
    pub srtla_port: String,
    pub bitrate_overlay: bool,
    pub ssh_pass: Option<String>,
    pub asrc: String,
    pub acodec: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Netif {
    pub ip: String,
    pub txb: u64,
    pub tp: u64,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pipeline {
    pub acodec: bool,
    pub asrc: bool,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamingStatus {
    pub is_streaming: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Status {
    pub is_streaming: bool,
    pub available_updates: Option<AvailableUpdates>,
    pub updating: Option<serde_json::Value>,
    pub ssh: Ssh,
    pub wifi: HashMap<String, Wifi>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Updating {
    pub updating: Update,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Update {
    pub downloading: u32,
    pub unpacking: u32,
    pub setting_up: u32,
    pub total: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WifiChange {
    pub wifi: HashMap<String, Wifi>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AvailableUpdates {
    pub package_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Ssh {
    pub user: String,
    pub user_pass: bool,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Wifi {
    pub ifname: String,
    pub conn: Option<String>,
    pub available: Vec<Available>,
    pub saved: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Available {
    pub active: bool,
    pub ssid: String,
    pub signal: i64,
    pub security: String,
    pub freq: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sensors {
    #[serde(rename = "SoC voltage")]
    pub soc_voltage: Option<String>,
    #[serde(rename = "SoC current")]
    pub soc_current: Option<String>,
    #[serde(rename = "SoC temperature")]
    pub soc_temperature: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Revisions {
    #[serde(rename = "belaUI")]
    pub bela_ui: String,
    pub belacoder: String,
    pub srtla: String,
    #[serde(rename = "BELABOX image")]
    pub belabox_image: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub show: Vec<NotificationMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NotificationMessage {
    pub duration: u32,
    pub is_dismissable: bool,
    pub is_persistent: bool,
    pub msg: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bitrate {
    pub max_br: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Acodecs {
    pub aac: String,
    pub opus: String,
}
