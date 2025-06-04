use actix_web::{body::BoxBody, HttpRequest, HttpResponse, Responder, ResponseError};
use jiff::{civil::Time, tz::TimeZone};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json;
use std::{fs, io, num::ParseIntError, str::FromStr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SettingsError {
    #[error("file io: `{0}`")]
    File(#[from] io::Error),
    #[error("json: `{0}`")]
    Json(#[from] serde_json::Error),
    #[error("parse int: `{0}`")]
    ParseInt(#[from] ParseIntError),
    #[error(r#"invalid vibration pattern: `{0}`, espected "double" or "rise""#)]
    InvalidVibrationPattern(String),
    #[error(
        "the settings are currently in Couples mode, use `/left` or `/right` prefixes not `/both`"
    )]
    NotCouples,
    #[error(
        "the settings are currently in Solo mode, use `/both` prefix not `/left` or `/right`"
    )]
    NotSolo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    #[serde(deserialize_with = "timezone_de", serialize_with = "timezone_ser")]
    pub timezone: TimeZone,
    #[serde(default)]
    pub away_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prime: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub led_brightness: Option<u8>,
    #[serde(flatten)]
    pub by_side: BySideSettings,
    // TODO nap mode
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum BySideSettings {
    Couples {
        left: SideSettings,
        right: SideSettings,
    },
    Solo {
        both: SideSettings,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SideSettings {
    ///offset from "neutral" temperature, °C*10 (IE -40 -> -4°C)
    pub temp_profile: Vec<i16>,
    pub sleep: Time,
    pub wake: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vibration: Option<VibrationAlarm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heat: Option<HeatAlarm>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VibrationAlarm {
    pub pattern: VibrationPattern,
    ///0-100
    pub intensity: u8,
    ///seconds
    pub duration: u16,
    ///seconds before sleep
    pub offset: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VibrationPattern {
    ///heavy
    Double,
    ///gentle
    Rise,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct HeatAlarm {
    pub temp: i16,
    ///seconds before sleep
    pub offset: u16,
}

impl Settings {
    pub fn from_file(path: &str) -> Result<Self, SettingsError> {
        let file_contents = fs::read_to_string(path)?;
        Self::from_str(&file_contents)
    }

    pub fn from_str(json: &str) -> Result<Self, SettingsError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn serialize(&self) -> Result<String, SettingsError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn save(&self, path: &str) -> Result<(), SettingsError> {
        let json = self.serialize()?;
        Ok(fs::write(path, json)?)
    }

    pub fn as_couples_mut(&mut self) -> Result<(&mut SideSettings, &mut SideSettings), SettingsError> {
        match &mut self.by_side {
            BySideSettings::Couples { left, right } => Ok((left, right)),
            _ => Err(SettingsError::NotSolo),
        }
    }

    pub fn as_solo_mut(&mut self) -> Result<&mut SideSettings, SettingsError> {
        match &mut self.by_side {
            BySideSettings::Solo { both } => Ok(both),
            _ => Err(SettingsError::NotCouples),
        }
    }

    pub fn as_couples(&self) -> Result<(&SideSettings, &SideSettings), SettingsError> {
        match &self.by_side {
            BySideSettings::Couples { left, right } => Ok((left, right)),
            _ => Err(SettingsError::NotSolo),
        }
    }

    pub fn as_solo(&self) -> Result<&SideSettings, SettingsError> {
        match &self.by_side {
            BySideSettings::Solo { both } => Ok(both),
            _ => Err(SettingsError::NotCouples),
        }
    }
}

impl VibrationPattern {
    pub fn to_string(&self) -> String {
        match self {
            VibrationPattern::Double => "double",
            VibrationPattern::Rise => "rise",
        }
        .to_string()
    }
}

impl FromStr for VibrationPattern {
    type Err = SettingsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "double" => Ok(Self::Double),
            "rise" => Ok(Self::Rise),
            _ => Err(SettingsError::InvalidVibrationPattern(s.to_string())),
        }
    }
}

fn timezone_de<'de, D: Deserializer<'de>>(deserializer: D) -> Result<TimeZone, D::Error> {
    let tzname = String::deserialize(deserializer)?;
    TimeZone::get(&tzname)
        .map_err(serde::de::Error::custom)
}

fn timezone_ser<S: Serializer>(tz: &TimeZone, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(tz.iana_name().unwrap())
}

impl Responder for SettingsError {
    type Body = BoxBody;

    fn respond_to(self, _: &HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::InternalServerError().body(self.to_string())
    }
}

impl ResponseError for SettingsError {}
