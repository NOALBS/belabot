use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
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
    Acodecs(HashMap<String, String>),
    Asrcs(Asrcs),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RemoteAuth {
    #[serde(rename = "auth/key")]
    pub auth_key: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RemoteEncoder {
    pub is_encoder_online: bool,
    pub version: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Config {
    pub password_hash: String,
    pub remote_key: String,
    pub max_br: u32,
    pub delay: i32,
    pub pipeline: String,
    pub srt_latency: u64,
    pub srt_streamid: String,
    pub srtla_addr: String,
    pub srtla_port: u16,
    pub bitrate_overlay: bool,
    pub ssh_pass: Option<String>,
    pub asrc: String,
    pub acodec: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Netif {
    pub ip: String,
    pub txb: u64,
    pub tp: u64,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub acodec: bool,
    pub asrc: bool,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct StreamingStatus {
    pub is_streaming: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Status {
    pub is_streaming: bool,
    pub available_updates: Option<AvailableUpdates>,
    pub updating: Option<serde_json::Value>,
    pub ssh: Ssh,
    pub wifi: HashMap<String, Wifi>,
    pub asrcs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Updating {
    pub updating: Update,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Update {
    pub downloading: u32,
    pub unpacking: u32,
    pub setting_up: u32,
    pub total: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct WifiChange {
    pub wifi: HashMap<String, Wifi>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AvailableUpdates {
    pub package_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Ssh {
    pub user: String,
    pub user_pass: bool,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Wifi {
    pub ifname: String,
    pub conn: Option<String>,
    pub available: Vec<Available>,
    pub saved: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Available {
    pub active: bool,
    pub ssid: String,
    pub signal: i64,
    pub security: String,
    pub freq: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Sensors {
    #[serde(rename = "SoC voltage")]
    pub soc_voltage: Option<String>,
    #[serde(rename = "SoC current")]
    pub soc_current: Option<String>,
    #[serde(rename = "SoC temperature")]
    pub soc_temperature: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Revisions {
    #[serde(rename = "belaUI")]
    pub bela_ui: String,
    pub belacoder: String,
    pub srtla: String,
    #[serde(rename = "BELABOX image")]
    pub belabox_image: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Notification {
    pub show: Vec<NotificationMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct NotificationMessage {
    pub duration: u32,
    pub is_dismissable: bool,
    pub is_persistent: bool,
    pub msg: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Bitrate {
    pub max_br: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Asrcs {
    pub asrcs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asrcs() {
        let message = r#"{"status":{"asrcs":["Cam Link 4k","USB audio","No audio"]}}"#;

        let parsed = deserialize(message);
        println!("{:#?}", parsed);

        let expected = Message::Asrcs(Asrcs {
            asrcs: vec![
                "Cam Link 4k".to_string(),
                "USB audio".to_string(),
                "No audio".to_string(),
            ],
        });

        assert_eq!(parsed, expected);
    }

    #[test]
    fn is_streaming() {
        let message = r#"{"status":{"is_streaming":true}}"#;

        let parsed = deserialize(message);
        println!("{:#?}", parsed);

        let expected = Message::StreamingStatus(StreamingStatus { is_streaming: true });

        assert_eq!(parsed, expected);
    }

    fn deserialize(json: &str) -> Message {
        let text: serde_json::Value = serde_json::from_str(json).unwrap();
        let text = text.as_object().unwrap();
        let value = text.values().next().unwrap();
        let m: Message = serde_json::from_value(value.to_owned()).unwrap();

        m
    }
}
