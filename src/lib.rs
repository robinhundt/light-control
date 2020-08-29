use crate::ipc::Message;
use anyhow::{Context, Result};
use futures::lock::Mutex;
use futures::{FutureExt, StreamExt};
use paho_mqtt as mqtt;
use paho_mqtt::CreateOptions;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::io::ErrorKind;
use std::path::Path;
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
        async_client
            .connect(mqtt::ConnectOptions::new())
            .await
            .with_context(|| format!("Failed to connect to mqqt broker: {:?}", mqtt_broker))?;
        Ok(LightServer {
            async_client,
            socket_listener,
        })
    }

    pub async fn start(&mut self, light_topic: &str) -> Result<()> {
        let curr_light_state: Mutex<Option<LightState>> = Mutex::new(None);

        self.async_client
            .subscribe(light_topic, mqtt::QOS_0)
            .await
            .with_context(|| format!("Unable to subscribe to topic: {}", light_topic))?;
        let mut stream = self.async_client.get_stream(1024);
        let handle_subscriptions = async {
            while let Some(item) = stream.next().await {
                if let Some(msg) = item {
                    let decoded = serde_json::from_slice(msg.payload())?;
                    let mut curr_light_state = curr_light_state.lock().await;
                    *curr_light_state = Some(decoded);
                }
            }
            Ok::<(), anyhow::Error>(())
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
                    let curr_light_state = curr_light_state.lock().await;
                    curr_light_state
                        .as_ref()
                        .context("Curr lights not set")?
                        .compute_change(&msg)
                };
                let serialized = serde_json::to_string(&light_change)?;
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
    pub fn compute_change(&self, cmd: &ipc::Message) -> LightStateChange {
        match cmd {
            Message::On => LightStateChange {
                state: Some("ON".into()),
                ..Default::default()
            },
            Message::Off => LightStateChange {
                state: Some("OFF".into()),
                ..Default::default()
            },
            Message::Dim(val) => LightStateChange {
                brightness: self.brightness.checked_sub(*val).or(Some(0)),
                ..Default::default()
            },
            Message::Brighten(val) => LightStateChange {
                brightness: self.brightness.checked_add(*val),
                ..Default::default()
            },
            Message::SetBrightness(val) => LightStateChange {
                brightness: Some(*val),
                ..Default::default()
            },
        }
    }
}
