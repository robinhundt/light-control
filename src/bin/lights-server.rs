use light_control::LightServer;

const SOCKET_PATH: &str = "/tmp/lights.sock";
const LIGHT_TOPIC: &str = "zigbee2mqtt/lamp_robin";

async fn run() {
    let mut server = LightServer::connect(SOCKET_PATH, "tcp://filch.lan:1883")
        .await
        .expect("Unable to connect");
    server.start(LIGHT_TOPIC).await.expect("Stopped server");
}

#[tokio::main]
async fn main() {
    run().await
}
