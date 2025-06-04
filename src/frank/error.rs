use hex::FromHexError;
use thiserror::Error;
use tokio::io;

use crate::settings::SettingsError;

#[derive(Error, Debug)]
pub enum FrankError {
    #[error("could not remove existing socket: `{0}`")]
    RemoveSocket(io::Error),
    #[error("could not bind unix listener: `{0}`")]
    BindUnixListener(io::Error),
    #[error("io error: `{0}`")]
    IO(#[from] io::Error),
    #[error("cbor deserialization error: `{0}`")]
    CborDe(#[from] ciborium::de::Error<io::Error>),
    #[error("cbor serialization error: `{0}`")]
    CborSer(#[from] ciborium::ser::Error<io::Error>),
    #[error("from hex error: `{0}`")]
    FromHex(#[from] FromHexError),
    #[error("json error: `{0}`")]
    JSON(#[from] serde_json::Error),
    #[error("settings error: `{0}`")]
    Settings(#[from] SettingsError),
    #[error("tried to perform operation but frank is not listening")]
    NotConnected,
    #[error("unexpected end of stream while reading response")]
    UnexpectedEndOfStream,
    #[error("timed out while waiting for response")]
    Timeout,
    #[error("frank gave bad ping response")]
    BadPing,
    #[error("variable `{0}` is missing")]
    VarMissing(String),
    #[error("failed to parse variable `{0}`")]
    VarFailedParse(String),
    #[error(r#"expected frank to say "ok" but got `{0}`"#)]
    ExpectedOk(String),
}
