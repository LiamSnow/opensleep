use std::{fmt::Display, time::Duration};

use jiff::{civil::Time, tz::TimeZone};
use log::{debug, error};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    time::timeout,
};

use crate::settings::VibrationAlarm;

use super::{
    error::FrankError,
    state::{FrankSettings, FrankState},
};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);

const HELLO: u8 = 0;
const ALARM_LEFT: u8 = 5;
const ALARM_RIGHT: u8 = 6;
const SET_SETTINGS: u8 = 8;
const TEMP_DUR_LEFT: u8 = 9;
const TEMP_DUR_RIGHT: u8 = 10;
const TEMP_LEFT: u8 = 11;
const TEMP_RIGHT: u8 = 12;
const PRIME: u8 = 13;
const STATUS: u8 = 14;
const ALARM_CLEAR: u8 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
 #[allow(dead_code)]
pub enum FrankCommand {
    Prime,
    ClearAlarm,
    SetAlarm(SideTarget, Box<(VibrationAlarm, Time, TimeZone)>),
    /// side, temp, duration (seconds)
    SetTemp(SideTarget, i16, u16),
    SetSettings(Box<FrankSettings>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideTarget {
    Left,
    Right,
    Both,
}

impl FrankCommand {
    pub async fn exec(self, stream: &mut UnixStream) -> Result<(), FrankError> {
        use FrankCommand::*;
        use SideTarget::*;

        let res = match &self {
            Prime => trans(stream, PRIME).await?,
            ClearAlarm => trans(stream, ALARM_CLEAR).await?,
            SetAlarm(side, bx) => {
                let (alarm, time, tz) = *bx.clone();
                let cbor = alarm.stamp(time, tz).to_cbor()?;
                match side {
                    Left => transpay(stream, ALARM_LEFT, &cbor).await?,
                    Right => transpay(stream, ALARM_RIGHT, &cbor).await?,
                    Both => {
                        transpay(stream, ALARM_LEFT, &cbor).await?;
                        transpay(stream, ALARM_RIGHT, &cbor).await?
                    },
                }
            }
            SetTemp(side, temp, duration) => {
                match side {
                    Left => {
                        transpay(stream, TEMP_DUR_LEFT, &duration.to_string()).await?;
                        transpay(stream, TEMP_LEFT, &temp.to_string()).await?
                    },
                    Right => {
                        transpay(stream, TEMP_DUR_RIGHT, &duration.to_string()).await?;
                        transpay(stream, TEMP_RIGHT, &temp.to_string()).await?
                    },
                    Both => {
                        transpay(stream, TEMP_DUR_LEFT, &duration.to_string()).await?;
                        transpay(stream, TEMP_LEFT, &temp.to_string()).await?;
                        transpay(stream, TEMP_DUR_RIGHT, &duration.to_string()).await?;
                        transpay(stream, TEMP_RIGHT, &temp.to_string()).await?
                    }
                }
            }
            SetSettings(settings) => {
                let hex = settings.to_cbor()?;
                transpay(stream, SET_SETTINGS, &hex).await?
            }
        };

        debug!("sent {self}, got {res}");

        Ok(())
    }
}

/// Writes a command (no payload) to Frank and
/// returns his response if successful
async fn trans(stream: &mut UnixStream, command: u8) -> Result<String, FrankError> {
    transaction_bytes(stream, format!("{}\n\n", command).as_bytes()).await
}

/// Writes a command and payload to Frank and
/// returns his response if successful
async fn transpay(
    stream: &mut UnixStream,
    command: u8,
    data: &str,
) -> Result<String, FrankError> {
    transaction_bytes(stream, format!("{}\n{}\n\n", command, data).as_bytes()).await
}

/// Writes a message to Frank, waits RESPONSE_TIMEOUT
/// for him to respond and gives back his response
async fn transaction_bytes(stream: &mut UnixStream, bytes: &[u8]) -> Result<String, FrankError> {
    stream.writable().await?;
    stream.write(bytes).await?;

    stream.readable().await?;
    let mut reader = BufReader::new(stream);
    let read_result = timeout(RESPONSE_TIMEOUT, async {
        //read until a double newline
        let mut result = String::new();
        let mut prev_ended = false;
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).await?;

            if bytes_read == 0 {
                return Err(FrankError::UnexpectedEndOfStream);
            }
            result.push_str(&line);

            if line == "\n" && prev_ended {
                break;
            }
            prev_ended = line.ends_with('\n');
        }
        Ok(result)
    })
    .await;

    match read_result {
        Ok(result) => result,
        Err(_) => Err(FrankError::Timeout),
    }
}

/// Requests a status update from Frank,
/// returning the parsed result if successful
pub async fn request_new_state(stream: &mut UnixStream) -> Option<FrankState> {
    let res = match trans(stream, STATUS).await {
        Ok(res) => res,
        Err(e) => {
            error!("get state command failed: {e}");
            return None;
        }
    };

    let new_state = match FrankState::parse(res) {
        Ok(state) => state,
        Err(e) => {
            error!("frank state failed to parse: {e}");
            return None;
        }
    };

    Some(new_state)
}

/// Says hi a new Frank. If they are unfriendly it returns None
pub async fn greet(mut stream: UnixStream) -> Option<UnixStream> {
    match trans(&mut stream, HELLO).await {
        Ok(s) if s.as_str() == "ok" => Some(stream),
        Ok(s) => {
            error!("new Frank is unfriendly, stating: {s}");
            None
        }
        Err(e) => {
            error!("new Frank ignored us: {e}");
            None
        }
    }
}

impl Display for FrankCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use FrankCommand::*;
        match self {
            Prime => write!(f, "Prime"),
            ClearAlarm => write!(f, "ClearAlarm"),
            SetAlarm(..) => write!(f, "SetAlarm"),
            SetTemp(..) => write!(f, "SetTemp"),
            SetSettings(..) => write!(f, "SetSettings"),
        }
    }
}
