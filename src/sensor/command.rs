use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString, FromRepr};

use crate::common::{
    codec::{CommandTrait, command},
    packet::BedSide,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SensorCommand {
    Ping,
    GetHardwareInfo,
    #[allow(dead_code)]
    GetFirmwareHash,
    JumpToFirmware,
    SetPiezoGain(u16, u16),
    #[allow(dead_code)]
    GetPiezoFreq,
    SetPiezoFreq(u32),
    EnablePiezo,
    // TODO add resp packet + 0x80
    #[allow(dead_code)]
    DisablePiezo,
    EnableVibration,
    #[allow(dead_code)]
    ProbeTemperature,
    SetAlarm(AlarmCommand),
    // TODO add resp packet + 0x80
    /// UNVERIFIED probably doesn't actually exist or requires some payload, seems to be crashing the mcu, or maybe its just a constant vibration mode idk
    #[allow(dead_code)]
    ClearAlarm,
    // TODO add resp packet + 0x80
    #[allow(dead_code)]
    GetHeaterOffset,
    #[allow(dead_code)]
    Random(Vec<u8>),
}

impl CommandTrait for SensorCommand {
    fn to_bytes(&self) -> Vec<u8> {
        use SensorCommand::*;
        match self {
            Ping => command(vec![0x01]),
            GetHardwareInfo => command(vec![0x02]),
            GetFirmwareHash => command(vec![0x04]),
            JumpToFirmware => command(vec![0x10]),
            GetPiezoFreq => command(vec![0x20]),
            SetPiezoFreq(freq) => command(vec![
                0x21,
                (*freq >> 24) as u8,
                (*freq >> 16) as u8,
                (*freq >> 8) as u8,
                *freq as u8,
            ]),
            EnablePiezo => command(vec![0x28]),
            DisablePiezo => command(vec![0x29]),
            SetPiezoGain(gain1, gain2) => command(vec![
                0x2B,
                (*gain1 >> 8) as u8,
                *gain1 as u8,
                (*gain2 >> 8) as u8,
                *gain2 as u8,
            ]),
            EnableVibration => command(vec![0x2E]),
            ProbeTemperature => command(vec![0x2F, 0xFF]),
            GetHeaterOffset => command(vec![0x2A]),
            Random(cmd) => command(cmd.clone()),
            SetAlarm(cmd) => {
                let payload = vec![
                    0x2C,
                    cmd.side as u8,
                    cmd.intensity,
                    cmd.pattern.clone() as u8,
                    (cmd.duration >> 24) as u8,
                    (cmd.duration >> 16) as u8,
                    (cmd.duration >> 8) as u8,
                    cmd.duration as u8,
                ];
                command(payload)
            }
            ClearAlarm => command(vec![0x2D]),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Display, EnumString, FromRepr, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
#[repr(u8)]
pub enum AlarmPattern {
    Single = 0b00,
    Double = 0b01,
    Unkown1 = 0b10,
    Unkown2 = 0b11,
    // 0b100 breaks it
    // 0b101+ seems to work tho?
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlarmCommand {
    pub side: BedSide,
    pub intensity: u8, // percentage 0-100
    pub duration: u32, // seconds
    pub pattern: AlarmPattern,
}

impl AlarmCommand {
    #[allow(dead_code)]
    pub fn new(side: BedSide, intensity: u8, duration: u32, pattern: AlarmPattern) -> Self {
        Self {
            side,
            intensity,
            duration,
            pattern,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn test_sensor_commands() {
        assert_eq!(
            SensorCommand::Ping.to_bytes(),
            hex!("7E 01 01 DC BD").to_vec()
        );
        assert_eq!(
            SensorCommand::GetHardwareInfo.to_bytes(),
            hex!("7E 01 02 EC DE").to_vec()
        );
        assert_eq!(
            SensorCommand::GetFirmwareHash.to_bytes(),
            hex!("7E 01 04 8C 18").to_vec()
        );
        assert_eq!(
            SensorCommand::JumpToFirmware.to_bytes(),
            hex!("7E 01 10 DE AD").to_vec()
        );
        assert_eq!(
            SensorCommand::SetPiezoFreq(1000).to_bytes(),
            hex!("7E 05 21 00 00 03 E8 7A 5E").to_vec()
        );
        assert_eq!(
            SensorCommand::EnablePiezo.to_bytes(),
            hex!("7E 01 28 69 F6").to_vec()
        );
        assert_eq!(
            SensorCommand::SetPiezoGain(400, 400).to_bytes(),
            hex!("7E 05 2B 01 90 01 90 AB 80").to_vec()
        );
        assert_eq!(
            SensorCommand::EnableVibration.to_bytes(),
            hex!("7E 01 2E 09 30").to_vec()
        );
        assert_eq!(
            SensorCommand::ProbeTemperature.to_bytes(),
            hex!("7E 02 2F FF 8C E8").to_vec()
        );
    }

    #[test]
    fn test_alarm_command() {
        // side, intensity, pattern, duration x4
        // 01   64   00   00 00 00 00
        // 01   64   00   00 00 00 14
        // 00   64   01   00 00 00 0c
        // 00   64   01   00 00 00 00
        // 01   32   00   00 00 00 00
        // 01   32   00   00 00 00 14
        // 01   64   01   00 00 00 00
        let alarm1 = AlarmCommand::new(BedSide::Right, 100, 20, AlarmPattern::Single);
        assert_eq!(
            SensorCommand::SetAlarm(alarm1).to_bytes(),
            hex!("7e 08 2c 01 64 00 00 00 00 14 38 8b").to_vec()
        );

        let alarm2 = AlarmCommand::new(BedSide::Left, 50, 50, AlarmPattern::Single);
        assert_eq!(
            SensorCommand::SetAlarm(alarm2).to_bytes(),
            hex!("7e 08 2c 00 32 00 00 00 00 32 39 3b").to_vec()
        );

        let alarm3 = AlarmCommand::new(BedSide::Left, 50, 0, AlarmPattern::Double);
        assert_eq!(
            SensorCommand::SetAlarm(alarm3).to_bytes(),
            hex!("7e 08 2c 00 32 01 00 00 00 00 85 7b").to_vec()
        );
    }
}
