use crate::ipc::Message;
use anyhow::{Context, Result};
use futures::{FutureExt, StreamExt};
use paho_mqtt as mqtt;
use paho_mqtt::{ConnectOptionsBuilder, CreateOptions};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;

pub mod ipc;

pub struct LightServer {
    async_client: mqtt::AsyncClient,
    socket_listener: UnixListener,
}

impl LightServer {
    pub async fn connect<P, T>(unix_socket: P, mqtt_broker: T) -> Result<Self>
    where
        P: AsRef<Path>,
        T: Into<CreateOptions> + Debug + Clone,
    {
        match fs::remove_file(&unix_socket).await {
            Ok(_) => (),
            Err(err) if err.kind() == ErrorKind::NotFound => (),
            result @ Err(_) => result.with_context(|| {
                format!(
                    "Deleting old socket at {}",
                    unix_socket.as_ref().to_string_lossy()
                )
            })?,
        };

        let socket_listener = UnixListener::bind(&unix_socket).with_context(|| {
            format!(
                "Failed to bind to socket at: {}",
                unix_socket.as_ref().to_string_lossy()
            )
        })?;

        let async_client = mqtt::AsyncClient::new(mqtt_broker.clone())
            .with_context(|| format!("Failed to create client for: {:?}", mqtt_broker))?;
        let connect_options = ConnectOptionsBuilder::new()
            .clean_session(false)
            .automatic_reconnect(Duration::from_secs(1), Duration::from_secs(8))
            .finalize();
        async_client
            .connect(connect_options)
            .await
            .with_context(|| format!("Failed to connect to mqqt broker: {:?}", mqtt_broker))?;
        Ok(LightServer {
            async_client,
            socket_listener,
        })
    }

    pub async fn start(&mut self, light_topic: &str) -> Result<()> {
        // TODO i think this function should be rewritten utilizing
        // tokio::spawn and selecting the join handles to get parallelism

        let curr_light_state: Mutex<Option<LightState>> = Mutex::new(None);

        self.async_client
            .subscribe(light_topic, mqtt::QOS_1)
            .await
            .with_context(|| format!("Unable to subscribe to topic: {}", light_topic))?;
        let mut stream = self.async_client.get_stream(1024);
        let handle_subscriptions = async {
            while let Some(item) = stream.next().await {
                if let Some(msg) = item {
                    let decoded = serde_json::from_slice(msg.payload())?;
                    log::info!("Received light state subscription: {:?}", &decoded);
                    let mut curr_light_state = curr_light_state
                        .lock()
                        .expect("Failed to lock curr_light_state");
                    *curr_light_state = Some(decoded);
                }
            }
            Err(anyhow::anyhow!("Subscription stream returned None"))
        };

        let handle_ipc = async {
            let light_topic_set = format!("{}/set", light_topic);
            let mut buf = Vec::new();

            while let Some(stream) = self.socket_listener.next().await {
                let mut stream = stream.context("Failed getting Unix stream")?;
                stream.read_to_end(&mut buf).await?;
                let msg: Message = bincode::deserialize(&buf)?;
                buf.clear();
                let light_change = {
                    let mut curr_light_state = curr_light_state
                        .lock()
                        .expect("Failed to lock curr_light_state");
                    curr_light_state
                        .as_mut()
                        .context("Curr lights not set")?
                        .compute_and_aplly_change(&msg)
                };
                let serialized = serde_json::to_string(&light_change)?;
                log::info!("Sending mqtt msg: {}", &serialized);
                let mqtt_msg = mqtt::Message::new(&light_topic_set, serialized, mqtt::QOS_0);
                self.async_client
                    .publish(mqtt_msg)
                    .await
                    .context("Failed publishing msg")?;
            }
            Err(anyhow::anyhow!("Socket listener stream returned None"))
        };

        futures::select! {
            res = handle_ipc.fuse() => res.context("IPC handling terminated"),
            res = handle_subscriptions.fuse() => res.context("Subscription handling terminated")
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LightState {
    state: String,
    brightness: usize,
    color_temp: usize,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct LightStateChange {
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    brightness: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color_temp: Option<usize>,
}

impl LightState {
    pub fn compute_and_aplly_change(&mut self, cmd: &ipc::Message) -> LightStateChange {
        match cmd {
            Message::On => {
                let on = "ON".to_string();
                self.state = on.clone();
                LightStateChange {
                    state: Some(on),
                    ..Default::default()
                }
            }
            Message::Off => {
                let off = "OFF".to_string();
                self.state = off.clone();
                LightStateChange {
                    state: Some(off),
                    ..Default::default()
                }
            }
            Message::Dim(val) => {
                let brightness = self.brightness.checked_sub(*val).unwrap_or(0);
                self.brightness = brightness;
                LightStateChange {
                    brightness: Some(brightness),
                    ..Default::default()
                }
            }
            Message::Brighten(val) => {
                let brightness = self.brightness.checked_add(*val).unwrap_or(1000);
                self.brightness = brightness;
                LightStateChange {
                    brightness: Some(brightness),
                    ..Default::default()
                }
            }
            Message::SetBrightness(val) => {
                self.brightness = *val;
                LightStateChange {
                    brightness: Some(*val),
                    ..Default::default()
                }
            }
        }
    }
}
