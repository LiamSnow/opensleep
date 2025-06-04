use std::{collections::HashMap, str::FromStr};

use serde::{Deserialize, Serialize};

use super::error::FrankError;

#[derive(Debug, PartialEq, Eq, Serialize, Default, Clone)]
pub struct FrankState {
    /// Before Frank connects this will be false
    /// and all values will be default
    pub valid: bool,
    /// The current temperature for each side of the bed
    pub cur_temp: BedTemp,
    /// The target/setpoint temperature for each side of the bed
    pub tar_temp: BedTemp,
    /// How long the target temperture will last
    /// for in seconds for each side of the bed
    pub tar_temp_time: BedTempTime,
    /// Example "20600-0001-F00-0001089C"
    pub sensor_label: String,
    pub water_level: bool,
    /// Whether the bed is priming or not
    pub priming: bool,
    pub settings: FrankSettings,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Default, Clone)]
pub struct FrankSettings {
    #[serde(rename = "v")]
    pub version: u8,
    #[serde(rename = "gl")]
    pub gain_left: u16,
    #[serde(rename = "gr")]
    pub gain_right: u16,
    #[serde(rename = "lb")]
    pub led_brightness_perc: u8,
}

/// Temp is offset from "neutral" temperature, °C*10 (IE -40 -> -4°C)
#[derive(Debug, PartialEq, Eq, Serialize, Default, Clone)]
pub struct BedTemp {
    /// Temp is offset from "neutral" temperature, °C*10 (IE -40 -> -4°C)
    pub left: i16,
    /// Temp is offset from "neutral" temperature, °C*10 (IE -40 -> -4°C)
    pub right: i16,
}

/// How long in seconds the tempature
/// will last for each side of the bed
#[derive(Debug, PartialEq, Eq, Serialize, Default, Clone)]
pub struct BedTempTime {
    pub left: u16,
    pub right: u16,
}

impl FrankState {
    pub fn parse(s: String) -> Result<Self, FrankError> {
        let variables: HashMap<&str, &str> = s
            .lines()
            .filter_map(|line| line.split_once(" = "))
            .collect();

        Ok(FrankState {
            valid: true,
            cur_temp: BedTemp {
                left: Self::parse_var::<i16>(&variables, "heatLevelL")?,
                right: Self::parse_var::<i16>(&variables, "heatLevelR")?,
            },
            tar_temp: BedTemp {
                left: Self::parse_var::<i16>(&variables, "tgHeatLevelL")?,
                right: Self::parse_var::<i16>(&variables, "tgHeatLevelR")?,
            },
            tar_temp_time: BedTempTime {
                left: Self::parse_var::<u16>(&variables, "heatTimeL")?,
                right: Self::parse_var::<u16>(&variables, "heatTimeR")?,
            },
            sensor_label: Self::get_var_string(&variables, "sensorLabel")?.to_string(),
            water_level: Self::parse_var::<bool>(&variables, "waterLevel")?,
            priming: Self::parse_var::<bool>(&variables, "priming")?,
            settings: FrankSettings::from_cbor(Self::get_var_string(&variables, "settings")?)?,
        })
    }

    fn get_var_string<'a>(
        vars: &HashMap<&str, &'a str>,
        var_name: &str,
    ) -> Result<&'a str, FrankError> {
        let s = vars
            .get(var_name)
            .ok_or_else(|| FrankError::VarMissing(var_name.to_string()))?;

        match s.len() {
            0 => Ok(s),
            1 => Ok(&s[0..0]),
            _ => Ok(&s[1..s.len() - 1]),
        }
    }

    fn parse_var<T: FromStr>(vars: &HashMap<&str, &str>, var_name: &str) -> Result<T, FrankError> {
        vars.get(var_name)
            .ok_or_else(|| FrankError::VarMissing(var_name.to_string()))?
            .parse()
            .or_else(|_| Err(FrankError::VarFailedParse(var_name.to_string())))
    }
}

impl FrankSettings {
    pub fn from_cbor(data: &str) -> Result<Self, FrankError> {
        let bytes = hex::decode(data)?;
        Ok(ciborium::from_reader(&bytes[..])?)
    }

    pub fn to_cbor(&self) -> Result<String, FrankError> {
        let mut buffer = Vec::<u8>::new();
        ciborium::into_writer(&self, &mut buffer)?;
        Ok(hex::encode_upper(buffer))
    }
}

#[cfg(test)]
mod tests {
    use crate::frank::state::{BedTemp, BedTempTime, FrankSettings, FrankState};

    #[test]
    fn test_settings_deserialize() {
        let test = "BF61760162676C190190626772190190626C621864FF";

        assert_eq!(
            FrankSettings::from_cbor(test).unwrap(),
            FrankSettings {
                version: 1,
                gain_right: 400,
                gain_left: 400,
                led_brightness_perc: 100,
            },
        );
    }

    #[test]
    fn test_settings_serialize() {
        assert_eq!(
            FrankSettings {
                version: 1,
                gain_right: 400,
                gain_left: 400,
                led_brightness_perc: 100,
            }
            .to_cbor()
            .unwrap(),
            // NOTE: this test string looks different because
            // ciborium is encoding a defined length (A4) map
            // versus frank is defining an indefinte length
            // map BF -- FF
            // This is totally fine and frank will happily
            // parse the defined length map
            "A461760162676C190190626772190190626C621864"
        );
    }

    #[test]
    fn test_frank_variables() {
        let inp = r#"tgHeatLevelR = 100
tgHeatLevelL = 100
heatTimeL = 0
heatLevelL = -100
heatTimeR = 0
heatLevelR = -100
sensorLabel = "20600-0001-F00-0001089C"
waterLevel = true
priming = false
settings = "BF61760162676C190190626772190190626C621864FF""#;
        let expected = FrankState {
            valid: true,
            cur_temp: BedTemp {
                left: -100,
                right: -100,
            },
            tar_temp: BedTemp {
                left: 100,
                right: 100,
            },
            tar_temp_time: BedTempTime { left: 0, right: 0 },
            sensor_label: "20600-0001-F00-0001089C".to_string(),
            water_level: true,
            priming: false,
            settings: FrankSettings {
                version: 1,
                gain_right: 400,
                gain_left: 400,
                led_brightness_perc: 100,
            },
        };
        let actual = FrankState::parse(inp.to_string()).unwrap();
        println!("{actual:#?}");
        assert_eq!(actual, expected);
    }
}
