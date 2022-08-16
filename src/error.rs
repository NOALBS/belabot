use crate::{belabox, config, twitch};

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// All Errors in this lib.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("BELABOX error")]
    Belabox(#[from] belabox::BelaboxError),
    #[error("Config error")]
    Config(#[from] config::ConfigError),
    #[error("Twitch validate error")]
    TwitchValide(#[from] twitch_irc::validate::Error),
    #[error("Twitch error")]
    Twitch(#[from] twitch::TwitchError),
}
