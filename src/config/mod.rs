use jiff::{civil::Time, tz::TimeZone};
use ron::extensions::Extensions;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fs;
use thiserror::Error;

use crate::common::packet::BedSide;
use crate::led::{CurrentBand, LedPattern};
use crate::sensor::command::AlarmPattern;

pub mod mqtt;
#[cfg(test)]
mod tests;

const CONFIG_FILE: &str = "config.ron";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse RON: {0}")]
    Ron(#[from] ron::error::SpannedError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LEDConfig {
    pub idle: LedPattern,
    pub active: LedPattern,
    pub band: CurrentBand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MqttConfig {
    pub server: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlarmConfig {
    pub pattern: AlarmPattern,
    pub intensity: u8,
    /// duration in seconds (TODO plz verify)
    pub duration: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresenceConfig {
    pub baselines: [u16; 6],
    pub threshold: u16,
    pub debounce_count: u8,
}

fn time_de<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Time, D::Error> {
    let s = String::deserialize(deserializer)?;
    Time::strptime("%H:%M", &s).map_err(serde::de::Error::custom)
}

fn time_ser<S: Serializer>(time: &Time, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&time.strftime("%H:%M").to_string())
}

fn timezone_de<'de, D: Deserializer<'de>>(deserializer: D) -> Result<TimeZone, D::Error> {
    let tzname = String::deserialize(deserializer)?;
    TimeZone::get(&tzname).map_err(serde::de::Error::custom)
}

fn timezone_ser<S: Serializer>(tz: &TimeZone, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(tz.iana_name().unwrap())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SideConfig {
    /// degrees celcius
    pub temperatures: Vec<f32>,
    #[serde(deserialize_with = "time_de", serialize_with = "time_ser")]
    pub sleep: Time,
    #[serde(deserialize_with = "time_de", serialize_with = "time_ser")]
    pub wake: Time,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alarm: Option<AlarmConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SidesConfig {
    Solo(SideConfig),
    Couples { left: SideConfig, right: SideConfig },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(deserialize_with = "timezone_de", serialize_with = "timezone_ser")]
    pub timezone: TimeZone,
    pub away_mode: bool,
    #[serde(deserialize_with = "time_de", serialize_with = "time_ser")]
    pub prime: Time,
    pub led: LEDConfig,
    pub mqtt: MqttConfig,
    pub profile: SidesConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<PresenceConfig>,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let opts = ron::Options::default().with_default_extension(Extensions::IMPLICIT_SOME);
        let config = opts.from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<(), ConfigError> {
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| ConfigError::Io(std::io::Error::other(e)))?;
        fs::write(path, content)?;
        Ok(())
    }
}

impl SidesConfig {
    pub fn get_side(&self, side: &BedSide) -> &SideConfig {
        match self {
            SidesConfig::Solo(cfg) => cfg,
            SidesConfig::Couples { left, right } => match side {
                BedSide::Left => left,
                BedSide::Right => right,
            },
        }
    }

    pub fn is_solo(&self) -> bool {
        matches!(self, SidesConfig::Solo(_))
    }

    pub fn is_couples(&self) -> bool {
        matches!(self, SidesConfig::Couples { .. })
    }

    pub fn unwrap_solo_mut(&mut self) -> &mut SideConfig {
        match self {
            SidesConfig::Solo(c) => c,
            SidesConfig::Couples { left: _, right: _ } => panic!(),
        }
    }

    pub fn unwrap_left_mut(&mut self) -> &mut SideConfig {
        match self {
            SidesConfig::Solo(_) => panic!(),
            SidesConfig::Couples { left, right: _ } => left,
        }
    }

    pub fn unwrap_right_mut(&mut self) -> &mut SideConfig {
        match self {
            SidesConfig::Solo(_) => panic!(),
            SidesConfig::Couples { left: _, right } => right,
        }
    }
}
