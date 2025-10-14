//! Bazecore config format.

/// Config schema.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// Neuron data.
    pub neuron: Neuron,
    /// Backup data.
    pub backup: Vec<Command>,
}

#[derive(Clone, Debug, Deserialize)]
/// Neuron schema.
pub struct Neuron {
    /// Layer details.
    pub layers: Vec<Layer>,
    /// Layout information about this device.
    pub device: Device,
}

/// Layer schema.
#[derive(Clone, Debug, Deserialize)]
pub struct Layer {
    /// Layer name.
    pub name: String,
}

/// Device schema.
#[derive(Clone, Debug, Deserialize)]
pub struct Device {
    /// Layout information about the keyboard.
    pub keyboard: Keyboard,
}

/// Keyboard schema.
#[derive(Clone, Debug, Deserialize)]
pub struct Keyboard {
    /// Layout information for the left half of the keyboard.
    pub left: Vec<Vec<u8>>,
    /// Layout information for the right half of the keyboard.
    pub right: Vec<Vec<u8>>,
}

/// Command schema.
#[derive(Clone, Debug, Deserialize)]
pub struct Command {
    /// Command type.
    pub command: CommandKind,
    /// Command data.
    pub data: String,
}

/// Possible commands.
#[derive(Clone, Debug, Deserialize, IsVariant)]
pub enum CommandKind {
    /// Custom keymap.
    #[serde(rename = "keymap.custom")]
    KeymapCustom,
    /// A command we don't know or care about.
    #[serde(untagged)]
    Unknown(String),
}
