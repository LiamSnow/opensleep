use crate::common::{
    checksum,
    codec::{CommandTrait, START},
    packet::BedSide,
};
use hex_literal::hex;

#[derive(Debug, Clone)]
pub enum FrozenCommand {
    Ping,
    #[allow(dead_code)]
    GetHardwareInfo,
    #[allow(dead_code)]
    GetFirmware,
    JumpToFirmware,
    #[allow(dead_code)]
    Prime,
    #[allow(dead_code)]
    SetTemperature {
        side: BedSide,
        temp: f32,
        enabled: bool,
    },
}

impl CommandTrait for FrozenCommand {
    fn to_bytes(&self) -> Vec<u8> {
        use FrozenCommand::*;
        match self {
            Ping => hex!("7E 01 01 DC BD").to_vec(),
            GetHardwareInfo => hex!("7E 01 02 EC DE").to_vec(),
            GetFirmware => hex!("7E 01 04 8C 18").to_vec(),
            JumpToFirmware => hex!("7E 01 10 DE AD").to_vec(),
            Prime => hex!("7E 01 52 b6 2b").to_vec(),
            SetTemperature {
                side,
                temp,
                enabled,
            } => make_temp_cmd(side, temp, enabled),
        }
    }
}

fn make_temp_cmd(side: &BedSide, temp: &f32, enabled: &bool) -> Vec<u8> {
    let temp_u16 = (*temp * 100.0) as u16;
    let payload: [u8; 5] = [
        0x40,
        *side as u8,
        *enabled as u8,
        (temp_u16 >> 8) as u8,
        temp_u16 as u8,
    ];

    let checksum = checksum::compute(&payload);

    vec![
        START,
        0x05,
        payload[0],
        payload[1],
        payload[2],
        payload[3],
        payload[4],
        (checksum >> 8) as u8,
        checksum as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::codec::CommandTrait;
    use hex_literal::hex;

    #[test]
    fn test_temperature_commands() {
        let cmd = FrozenCommand::SetTemperature {
            side: BedSide::Left,
            temp: 36.0,
            enabled: true,
        };
        assert_eq!(cmd.to_bytes(), hex!("7E 05 40 00 01 0E 10 E6 A8").to_vec());
    }
}
