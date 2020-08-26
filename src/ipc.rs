use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    On,
    Off,
    Dim(usize),
    Brighten(usize),
    SetBrightness(usize),
}
