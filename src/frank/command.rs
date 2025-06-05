use jiff::{civil::Time, tz::TimeZone};
use log::{error, info};
use tokio::net::UnixStream;

use crate::{
    frank::socket::{cbor_transaction, i16_transaction, u16_transaction},
    settings::VibrationAlarm,
};

use super::{
    error::FrankError,
    socket::{cmd_transaction, read_response, write_cmd_for_no_payload},
    state::{FrankSettings, FrankState},
};

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

        match &self {
            Prime => {
                info!("[Frank] Requesting to Prime");
                cmd_transaction(stream, PRIME).await?;
            },
            ClearAlarm => {
                info!("[Frank] Requesting to Clear Alarm");
                cmd_transaction(stream, ALARM_CLEAR).await?;
            }
            SetAlarm(side, bx) => {
                let (alarm, time, tz) = *bx.clone();
                info!("[Frank] Requesting Alarm at {time}");
                let cbor = alarm.stamp(time, tz).to_cbor()?;

                if side.cont_left() {
                    cbor_transaction(stream, ALARM_LEFT, &cbor).await?;
                }

                if side.cont_right() {
                    cbor_transaction(stream, ALARM_RIGHT, &cbor).await?;
                }
            }
            SetTemp(side, temp, duration) => {
                if side.cont_left() {
                    info!("[Frank] Left Temp {temp} for {duration} seconds");
                    u16_transaction(stream, TEMP_DUR_LEFT, *duration).await?;
                    i16_transaction(stream, TEMP_LEFT, *temp).await?;
                }

                if side.cont_right() {
                    info!("[Frank] Right Temp {temp} for {duration} seconds");
                    u16_transaction(stream, TEMP_DUR_RIGHT, *duration).await?;
                    i16_transaction(stream, TEMP_RIGHT, *temp).await?;
                }
            }
            SetSettings(settings) => {
                info!("[Frank] Setting new settings to {settings:#?}");
                cbor_transaction(stream, SET_SETTINGS, &settings.to_cbor()?).await?
            }
        }

        Ok(())
    }
}

/// Says hi a new Frank. If they are unfriendly it returns None
pub async fn greet(mut stream: UnixStream) -> Option<UnixStream> {
    match cmd_transaction(&mut stream, HELLO).await {
        Ok(_) => {
            info!("[Frank] New Frank accepted");
            Some(stream)
        },
        Err(e) => {
            error!("[Frank] Unexpected HELLO response: {e}");
            None
        }
    }
}

/// Requests a status update from Frank,
/// returning the parsed result if successful
pub async fn request_new_state(stream: &mut UnixStream) -> Option<FrankState> {
    if let Err(e) = write_cmd_for_no_payload(stream, STATUS).await {
        error!("[Frank] Failed to write STATUS command: {e}");
        return None
    }

    // FrankState is usually 230-245 bytes, biggest line
    // is setting ~57 bytes
    let res = match read_response(stream, 260, 60).await {
        Ok(s) => s,
        Err(e) => {
            error!("[Frank] Get status update command failed: {e}");
            return None;
        }
    };

    let new_state = match FrankState::parse(res) {
        Ok(state) => state,
        Err(e) => {
            error!("[Frank] FrankState failed to parse: {e}");
            return None;
        }
    };

    Some(new_state)
}

impl SideTarget {
    fn cont_left(&self) -> bool {
        use SideTarget::*;
        match self {
            Left | Both => true,
            Right => false,
        }
    }

    fn cont_right(&self) -> bool {
        use SideTarget::*;
        match self {
            Right | Both => true,
            Left => false,
        }
    }
}
