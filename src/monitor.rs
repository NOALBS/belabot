use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::{
    sync::{broadcast, RwLock},
    time::Instant,
};
use tracing::{error, warn};

use crate::{
    belabox::{self, messages, Message},
    bot::BelaState,
    config, Belabox, Twitch,
};

pub struct Monitor {
    pub belabox: Arc<Belabox>,
    pub bela_state: Arc<RwLock<BelaState>>,
    pub twitch: Arc<Twitch>,
}

impl Monitor {
    pub async fn run(
        &self,
        mut messages: broadcast::Receiver<belabox::Message>,
        monitor: config::Monitor,
    ) {
        while let Ok(message) = messages.recv().await {
            match message {
                Message::Netif(netif) => {
                    if monitor.modems {
                        self.modems(netif).await;
                    }
                }
                Message::Sensors(sensors) => {
                    if monitor.ups {
                        self.ups(sensors, monitor.ups_plugged_in).await;
                    }
                }
                Message::Notification(messages::Notifications::Show(notification)) => {
                    if monitor.notifications {
                        self.notifications(notification, monitor.notification_timeout)
                            .await;
                    }
                }
                _ => {}
            }
        }
    }

    async fn send(&self, message: String) {
        if let Err(e) = self.twitch.send(message).await {
            error!(?e, "error sending message to twitch");
        }
    }

    pub async fn modems(&self, netif: HashMap<String, messages::Netif>) {
        let read = self.bela_state.read().await;
        let previous = match &read.netif {
            Some(p) => p,
            None => return,
        };

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
            self.send(format!("BB: {}", message.join(", "))).await;
        }
    }

    pub async fn ups(&self, sensors: messages::Sensors, plugged_voltage: f64) {
        let voltage = match &sensors.soc_voltage {
            Some(v) => v,
            None => return,
        };

        let voltage = voltage.split_whitespace().next().unwrap();
        let voltage = voltage.parse::<f64>().unwrap();
        let plugged_in = (voltage * 100.0).floor() / 100.0 >= plugged_voltage;

        let charging = {
            let mut lock = self.bela_state.write().await;
            let notify = &mut lock.notify_ups;

            let current_notify = match notify {
                Some(n) => *n,
                None => plugged_in,
            };

            let charging = match (plugged_in, current_notify) {
                (true, false) => Some(true),
                (false, true) => Some(false),
                _ => None,
            };

            *notify = Some(plugged_in);

            charging
        };

        if let Some(c) = charging {
            let a = if !c { "not" } else { "" };
            let msg = format!("BB: UPS {} charging", a);

            self.send(msg).await;
        }
    }

    pub async fn notifications(
        &self,
        notification: messages::NotificationShow,
        notification_timeout: u64,
    ) {
        let mut lock = self.bela_state.write().await;
        let timeout = &mut lock.notification_timeout;

        let now = Instant::now();
        for notification in notification.show {
            if let Some(time) = timeout.get(&notification.name) {
                if time.elapsed() < Duration::from_secs(notification_timeout) {
                    continue;
                }
            }

            warn!(notification.msg, "notication");

            timeout
                .entry(notification.name)
                .and_modify(|n| *n = now)
                .or_insert(now);

            self.send("BB: ".to_owned() + &notification.msg).await;
        }
    }
}
