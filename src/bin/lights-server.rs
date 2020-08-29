use anyhow::{Context, Result};
use light_control::LightServer;

const SOCKET_PATH: &str = "/tmp/lights.sock";
const LIGHT_TOPIC: &str = "zigbee2mqtt/lamp_robin";

async fn run() -> Result<()> {
    let mut server = LightServer::connect(SOCKET_PATH, "tcp://filch.lan:1883")
        .await
        .context("Unable to connect")?;
    server.start(LIGHT_TOPIC).await.context("Server crashed")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}
