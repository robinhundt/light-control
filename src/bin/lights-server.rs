use anyhow::{Context, Result};
use env_logger::{Builder, Env};
use light_control::LightServer;

const SOCKET_PATH: &str = "/tmp/lights.sock";
const LIGHT_TOPIC: &str = "zigbee2mqtt/lamp_robin";

async fn run() -> Result<()> {
    let mut server = LightServer::connect(SOCKET_PATH, ("tcp://filch.lan:1883", "robin_arch"))
        .await
        .context("Unable to connect")?;
    server.start(LIGHT_TOPIC).await.context("Server crashed")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    Builder::from_env(Env::default().default_filter_or("info")).init();
    run().await
}
