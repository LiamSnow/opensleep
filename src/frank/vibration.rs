use jiff::{civil::Time, tz::TimeZone, Timestamp};
use serde::{Deserialize, Serialize};

use crate::settings::VibrationAlarm;

use super::error::FrankError;

#[derive(Debug, Deserialize, Serialize)]
pub struct TimestampedVibrationAlarm {
    #[serde(rename = "pl")]
    pub intensity_percent: u8,
    #[serde(rename = "du")]
    pub duration_sec: u16,
    #[serde(rename = "pi")]
    pub pattern: String,
    #[serde(rename = "tt")]
    pub timestamp: u64,
}

impl VibrationAlarm {
    pub fn stamp(&self, time: Time, tz: TimeZone) -> TimestampedVibrationAlarm {
        TimestampedVibrationAlarm {
            intensity_percent: self.intensity,
            duration_sec: self.duration,
            pattern: self.pattern.to_string(),
            timestamp: Timestamp::now()
                .to_zoned(tz)
                .with()
                .time(time)
                .build()
                .unwrap()
                .timestamp()
                .as_second() as u64,
        }
    }
}

impl TimestampedVibrationAlarm {
    pub fn to_cbor(&self) -> Result<String, FrankError> {
        let mut buffer = Vec::<u8>::new();
        ciborium::into_writer(&self, &mut buffer)?;
        Ok(hex::encode(buffer))
    }
}

// TODO tests
