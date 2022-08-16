use anyhow::Result;

use not_yet_named_bot::{Bot, Settings};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let config = match Settings::load("config.json") {
        Ok(c) => c,
        Err(_) => Settings::ask_for_settings().await?,
    };

    let bot = Bot::new(config).await?;

    // There is no way to recover when any of these stop, so stop the program
    tokio::select! {
        _ = bot.bb_msg_handle => {}
        _ = bot.tw_msg_handle => {}
    };

    Ok(())
}
