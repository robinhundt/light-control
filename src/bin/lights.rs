use clap::Clap;
use light_control::ipc::Message;
use std::convert::{TryFrom, TryInto};
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

#[derive(Clap)]
struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clap)]
enum Command {
    /// Turn of the light
    On,
    /// Turn on the light
    Off,
    /// Dim the light by a value
    Dim(Val),
    /// Brighten the light by a vlue
    Brighten(Val),
    /// Set the brightness to the value
    SetBrightness(Val),
}

#[derive(Clap)]
struct Val {
    value: usize,
}

async fn run() {
    let opts: Opts = Opts::parse();
    let mut stream = UnixStream::connect("/tmp/lights.sock").await.unwrap();
    let msg: Message = opts
        .command
        .try_into()
        .expect("Unable to convert command into message");
    let encoded = bincode::serialize(&msg).expect("Unable to encode message");
    stream.write_all(&encoded).await.unwrap();
}

#[tokio::main]
async fn main() {
    run().await
}

impl TryFrom<Command> for Message {
    type Error = ();

    fn try_from(value: Command) -> Result<Self, Self::Error> {
        let msg = match value {
            Command::On => Message::On,
            Command::Off => Message::Off,
            Command::Dim(Val { value }) => Message::Dim(value),
            Command::Brighten(Val { value }) => Message::Brighten(value),
            Command::SetBrightness(Val { value }) => Message::SetBrightness(value),
        };
        Ok(msg)
    }
}
