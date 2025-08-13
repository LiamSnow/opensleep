use jiff::{civil::Time, tz::TimeZone};
use log::{debug, error};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fs;
use strum_macros::{Display, EnumString};
use thiserror::Error;
use tokio::sync::watch;
use tokio::time::{Duration, MissedTickBehavior};

const CONFIG_FILE: &str = "config.ron";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse RON: {0}")]
    Ron(#[from] ron::error::SpannedError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum LedPattern {
    Blue,
    BlueFire,
    RedFire,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum VibrationPattern {
    Rise = 0,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedConfig {
    pub idle: LedPattern,
    pub active: LedPattern,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MqttConfig {
    pub server: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VibrationConfig {
    pub pattern: VibrationPattern,
    pub intensity: u8,
    pub duration: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeatConfig {
    pub temp: u8,
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
pub struct Profile {
    pub temp_profile: Vec<i32>,
    #[serde(deserialize_with = "time_de", serialize_with = "time_ser")]
    pub sleep: Time,
    #[serde(deserialize_with = "time_de", serialize_with = "time_ser")]
    pub wake: Time,
    pub vibration: VibrationConfig,
    pub heat: HeatConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProfileType {
    Solo(Profile),
    Couples { left: Profile, right: Profile },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(deserialize_with = "timezone_de", serialize_with = "timezone_ser")]
    pub timezone: TimeZone,
    pub away_mode: bool,
    #[serde(deserialize_with = "time_de", serialize_with = "time_ser")]
    pub prime: Time,
    pub led: LedConfig,
    pub mqtt: MqttConfig,
    pub profile: ProfileType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<PresenceConfig>,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let config = ron::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<(), ConfigError> {
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| ConfigError::Io(std::io::Error::other(e)))?;
        fs::write(path, content)?;
        Ok(())
    }
}

impl ProfileType {
    #[allow(dead_code)]
    pub fn get_profile(&self, side: Option<&str>) -> Option<&Profile> {
        match (self, side) {
            (ProfileType::Solo(profile), _) => Some(profile),
            (ProfileType::Couples { left, .. }, Some("left")) => Some(left),
            (ProfileType::Couples { right, .. }, Some("right")) => Some(right),
            (ProfileType::Couples { .. }, _) => None,
        }
    }

    pub fn get_profile_mut(&mut self, side: Option<&str>) -> Option<&mut Profile> {
        match (self, side) {
            (ProfileType::Solo(profile), _) => Some(profile),
            (ProfileType::Couples { left, .. }, Some("left")) => Some(left),
            (ProfileType::Couples { right, .. }, Some("right")) => Some(right),
            (ProfileType::Couples { .. }, _) => None,
        }
    }

    #[allow(dead_code)]
    pub fn is_solo(&self) -> bool {
        matches!(self, ProfileType::Solo(_))
    }

    #[allow(dead_code)]
    pub fn is_couples(&self) -> bool {
        matches!(self, ProfileType::Couples { .. })
    }
}

// saves config changes to file debounced
pub async fn auto_save(config_rx: watch::Receiver<Config>) {
    let mut config_rx = config_rx;
    let mut save_timer = tokio::time::interval(Duration::from_millis(500));
    save_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut pending_save = false;

    loop {
        tokio::select! {
            Ok(_) = config_rx.changed() => {
                pending_save = true;
            }
            _ = save_timer.tick() => {
                if pending_save {
                    let config = config_rx.borrow_and_update();
                    if let Err(e) = config.save(CONFIG_FILE) {
                        error!("Failed to save config: {e}");
                    } else {
                        debug!("Config saved to disk");
                    }
                    pending_save = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_solo_config() {
        let config = Config::load("example_solo.ron").unwrap();
        assert_eq!(config.timezone.iana_name().unwrap(), "America/New_York");
        assert!(!config.away_mode);
        match &config.profile {
            ProfileType::Solo(profile) => {
                assert_eq!(profile.temp_profile, vec![27, 29, 31]);
            }
            _ => panic!("Expected solo profile"),
        }
    }

    #[test]
    fn test_load_couples_config() {
        let config = Config::load("example_couples.ron").unwrap();
        assert_eq!(config.timezone.iana_name().unwrap(), "America/New_York");
        assert!(!config.away_mode);
        match &config.profile {
            ProfileType::Couples { left, right } => {
                assert_eq!(left.temp_profile, vec![27, 29, 31]);
                assert_eq!(right.temp_profile, vec![27, 29, 31]);
            }
            _ => panic!("Expected couples profile"),
        }
    }
}
