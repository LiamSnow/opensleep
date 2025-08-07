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
    // 0?
    // #[serde(rename = "di")]
    // pub unknown: u16,
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
    pub fn to_cbor(&self) -> Result<Vec<u8>, FrankError> {
        let mut cbor_buf = Vec::<u8>::new();
        ciborium::into_writer(&self, &mut cbor_buf)?;
        let mut buf = vec![0u8; cbor_buf.len() * 2];
        hex::encode_to_slice(&cbor_buf, &mut buf)?;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::TimestampedVibrationAlarm;

    #[test]
    fn test_timestamped_vibration_alarm_cbor() {
        let ts = TimestampedVibrationAlarm {
            intensity_percent: 50,
            duration_sec: 100,
            pattern: "rise".to_string(),
            timestamp: 1749056040,
        };

        assert_eq!(
            ts.to_cbor().unwrap(),
            b"a462706c1832626475186462706964726973656274741a68407a28".to_vec()
        );
    }
}
