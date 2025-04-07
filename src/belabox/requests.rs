use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Request {
    Bitrate(Bitrate),
    Command(Command),
    Keepalive(Option<()>),
    Netif(Netif),
    Remote(Remote),
    Start(Start),
    Stop(u8),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Command {
    Poweroff,
    Reboot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Remote {
    #[serde(rename = "auth/key")]
    AuthKey { key: String, version: u32 },
    #[serde(rename = "auth/token")]
    AuthToken { token: String, version: u32 },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde_with::skip_serializing_none]
pub struct Start {
    pub pipeline: String,
    pub delay: i32,
    pub max_br: u32,
    pub srt_latency: u64,
    pub bitrate_overlay: bool,
    pub asrc: Option<String>,
    pub acodec: Option<String>,
    pub remote_key: String,
    pub relay_server: Option<String>,
    pub relay_account: Option<String>,
}

impl From<super::messages::Config> for Start {
    fn from(c: super::messages::Config) -> Self {
        Self {
            pipeline: c.pipeline,
            delay: c.delay,
            max_br: c.max_br,
            srt_latency: c.srt_latency,
            bitrate_overlay: c.bitrate_overlay,
            asrc: c.asrc,
            acodec: c.acodec,
            remote_key: c.remote_key,
            relay_server: c.relay_server,
            relay_account: c.relay_account,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bitrate {
    pub max_br: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Netif {
    pub name: String,
    pub ip: String,
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keepalive() {
        let message = Request::Keepalive(None);

        let json = serde_json::to_string(&message).unwrap();
        println!("{}", json);

        let expected = r#"{"keepalive":null}"#;
        assert_eq!(expected, json);
    }

    #[test]
    fn start() {
        let message = Request::Start(Start {
            pipeline: "7ca3d9dd20726a7c2dad06948e1eadc6f84c461c".to_string(),
            delay: 0,
            max_br: 500,
            srt_latency: 4000,
            bitrate_overlay: false,
            asrc: "No audio".to_string(),
            acodec: "opus".to_string(),
            remote_key: "remote_key".to_string(),
            relay_server: "1".to_string(),
            relay_account: "1".to_string(),
        });

        let json = serde_json::to_string(&message).unwrap();
        println!("{}", json);

        let expected = r#"{"start":{"pipeline":"7ca3d9dd20726a7c2dad06948e1eadc6f84c461c","delay":0,"max_br":500,"srt_latency":4000,"bitrate_overlay":false,"asrc":"No audio","acodec":"opus","remote_key":"remote_key","relay_server":"1","relay_account":"1"}}"#;
        assert_eq!(expected, json);
    }

    #[test]
    fn stop() {
        let message = Request::Stop(0);

        let json = serde_json::to_string(&message).unwrap();
        println!("{}", json);

        let expected = r#"{"stop":0}"#;
        assert_eq!(expected, json);
    }

    #[test]
    fn bitrate() {
        let message = Request::Bitrate(Bitrate { max_br: 1250 });

        let json = serde_json::to_string(&message).unwrap();
        println!("{}", json);

        let expected = r#"{"bitrate":{"max_br":1250}}"#;
        assert_eq!(expected, json);
    }

    #[test]
    fn reboot() {
        let message = Request::Command(Command::Reboot);

        let json = serde_json::to_string(&message).unwrap();
        println!("{}", json);

        let expected = r#"{"command":"reboot"}"#;
        assert_eq!(expected, json);
    }

    #[test]
    fn auth_key() {
        let message = Request::Remote(Remote::AuthKey {
            key: "testkey".to_string(),
            version: 6,
        });

        let json = serde_json::to_string(&message).unwrap();
        println!("{}", json);

        let expected = r#"{"remote":{"auth/key":{"key":"testkey","version":6}}}"#;
        assert_eq!(expected, json);
    }

    #[test]
    fn netif() {
        let message = Request::Netif(Netif {
            name: "wlan0".to_string(),
            ip: "192.168.1.10".to_string(),
            enabled: false,
        });

        let json = serde_json::to_string(&message).unwrap();
        println!("{}", json);

        let expected = r#"{"netif":{"name":"wlan0","ip":"192.168.1.10","enabled":false}}"#;
        assert_eq!(expected, json);
    }
}
