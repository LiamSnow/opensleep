use crate::common::{
    checksum,
    codec::{CommandTrait, START},
    packet::BedSide,
};
use hex_literal::hex;

#[derive(Debug, Clone)]
pub enum SensorCommand {
    Ping,
    #[allow(dead_code)]
    GetHardwareInfo,
    #[allow(dead_code)]
    GetFirmwareHash,
    JumpToFirmware,
    SetPiezoGain400400,
    SetPiezoFreq1KHz,
    EnablePiezo,
    EnableVibration,
    ProbeTemperature,
    Alarm(AlarmCommand),
}

impl CommandTrait for SensorCommand {
    fn to_bytes(&self) -> Vec<u8> {
        use SensorCommand::*;
        match self {
            Ping => hex!("7E 01 01 DC BD").to_vec(),
            GetHardwareInfo => hex!("7E 01 02 EC DE").to_vec(),
            GetFirmwareHash => hex!("7E 01 04 8C 18").to_vec(),
            JumpToFirmware => hex!("7E 01 10 DE AD").to_vec(),
            SetPiezoGain400400 => hex!("7E 05 2B 01 90 01 90 AB 80").to_vec(),
            SetPiezoFreq1KHz => hex!("7E 05 21 00 00 03 E8 7A 5E").to_vec(),
            EnablePiezo => hex!("7E 01 28 69 F6").to_vec(),
            EnableVibration => hex!("7E 01 2E 09 30").to_vec(),
            ProbeTemperature => hex!("7E 02 2F FF 8C E8").to_vec(),
            Alarm(cmd) => {
                make_alarm_command(cmd.side, cmd.intensity, cmd.duration, cmd.pattern).to_vec()
            }
        }
    }
}

const fn make_alarm_command(
    side: BedSide,
    intensity: u8,
    duration: u32,
    pattern: AlarmPattern,
) -> [u8; 12] {
    let payload = [
        0x2c,
        side as u8,
        intensity,
        pattern.to_byte(),
        (duration >> 24) as u8,
        (duration >> 16) as u8,
        (duration >> 8) as u8,
        duration as u8,
    ];

    let checksum = checksum::compute(&payload);

    [
        START,
        0x08,
        payload[0],
        payload[1],
        payload[2],
        payload[3],
        payload[4],
        payload[5],
        payload[6],
        payload[7],
        (checksum >> 8) as u8,
        checksum as u8,
    ]
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[allow(dead_code)]
pub enum AlarmPattern {
    /// Normal pattern
    Normal,
    /// Rise pattern
    Rise,
}

impl AlarmPattern {
    pub const fn to_byte(&self) -> u8 {
        match self {
            Self::Normal => 0x00,
            Self::Rise => 0x01,
        }
    }

    #[allow(dead_code)]
    pub const fn from_byte(val: u8) -> Option<Self> {
        match val {
            0x00 => Some(Self::Normal),
            0x01 => Some(Self::Rise),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
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
    fn test_alarm_command() {
        assert_eq!(
            make_alarm_command(BedSide::Right, 100, 20, AlarmPattern::Normal),
            hex!("7e 08 2c 01 64 00 00 00 00 14 38 8b")
        );

        assert_eq!(
            make_alarm_command(BedSide::Left, 50, 50, AlarmPattern::Normal),
            hex!("7e 08 2c 00 32 00 00 00 00 32 39 3b")
        );

        assert_eq!(
            make_alarm_command(BedSide::Left, 50, 0, AlarmPattern::Rise),
            hex!("7e 08 2c 00 32 01 00 00 00 00 85 7b")
        );
    }
}
