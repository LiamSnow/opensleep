use strum_macros::{AsRefStr, Display, IntoStaticStr};

use crate::{
    common::{
        codec::{CommandTrait, command},
        packet::BedSide,
    },
    frozen::packet::FrozenTarget,
};

#[derive(Debug, Clone, Display, AsRefStr, IntoStaticStr)]
pub enum FrozenCommand {
    Ping,
    GetHardwareInfo,
    #[allow(dead_code)]
    GetFirmware,
    JumpToFirmware,
    #[allow(dead_code)]
    Prime,
    #[allow(dead_code)]
    /// call every 10 seconds
    SetTargetTemperature {
        side: BedSide,
        tar: FrozenTarget,
    },
    GetTemperatures,
    Random(u8),
}

impl CommandTrait for FrozenCommand {
    fn to_bytes(&self) -> Vec<u8> {
        use FrozenCommand::*;
        match self {
            // 0x05 is sometimes the first command at boot unclear purpose
            Ping => command(vec![0x01]),
            GetHardwareInfo => command(vec![0x02]),
            GetFirmware => command(vec![0x04]),
            JumpToFirmware => command(vec![0x10]),
            GetTemperatures => command(vec![0x41]),

            /*

            After sending 0x50 command, we get back:

            Response In Test #1:
            D0 00
            28 FF C2 E9 A5 21 56 F3 07 FB
            28 FF 3A CF 23 22 31 12 09 34
            28 FF CE 0B 2C E2 23 56 0A 0F
            28 FF 07 E5 2C E2 20 39 0A 0F

            Response In Test #2:
            D0 00
            28 FF C2 E9 A5 21 56 F3 08 08
            28 FF 3A CF 23 22 31 12 09 3A
            28 FF CE 0B 2C E2 23 56 0A 15
            28 FF 07 E5 2C E2 20 39 0A 15


            <- GOT 0x50 RESPONSE SHOWN ABOVE #2 ->
            Temperature update - Left: 2581, Right: 2581, Heatsink: 2362, Error: 8
            Message: FW: pid[heatsink] 3.062500 0.693750 0.693750 0.000000 0.000000
            Message: FW: pump[left] slow @ 6.030475V 0.169202A
            Message: FW: pump[right] slow @ 6.044009V 0.161510A
            Message: FW: pid[left] 25.812500 0.090498 -0.003750 0.094248 0.000000
            Message: FW: pid[right] 25.812500 0.094561 -0.003750 0.098311 0.000000

            */

            /*
            0x51 -> D1 00 (Flash/calibration status?)

            UTF-8 decode error: invalid utf-8 sequence of 1 bytes from index 16
            Message: FW: flash locked
            Message: FW: cal_info valid

            */
            Prime => command(vec![0x52]),
            Random(cmd) => command(vec![*cmd]),
            SetTargetTemperature { side, tar } => command(vec![
                0x40,
                *side as u8,
                tar.enabled as u8,
                (tar.temp >> 8) as u8,
                tar.temp as u8,
            ]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::codec::CommandTrait;
    use hex_literal::hex;

    #[test]
    fn test_ping() {
        assert_eq!(
            FrozenCommand::Ping.to_bytes(),
            hex!("7E 01 01 DC BD").to_vec()
        );
    }

    #[test]
    fn test_gethardwareinfo() {
        assert_eq!(
            FrozenCommand::GetHardwareInfo.to_bytes(),
            hex!("7E 01 02 EC DE").to_vec()
        );
    }

    #[test]
    fn test_getfirmware() {
        assert_eq!(
            FrozenCommand::GetFirmware.to_bytes(),
            hex!("7E 01 04 8C 18").to_vec()
        );
    }

    #[test]
    fn test_jumptofirmware() {
        assert_eq!(
            FrozenCommand::JumpToFirmware.to_bytes(),
            hex!("7E 01 10 DE AD").to_vec()
        );
    }

    #[test]
    fn test_prime() {
        assert_eq!(
            FrozenCommand::Prime.to_bytes(),
            hex!("7E 01 52 b6 2b").to_vec()
        );
    }

    #[test]
    fn test_temp() {
        let cmd = FrozenCommand::SetTargetTemperature {
            side: BedSide::Left,
            tar: FrozenTarget {
                enabled: true,
                temp: 3600,
            },
        };
        assert_eq!(cmd.to_bytes(), hex!("7E 05 40 00 01 0E 10 E6 A8").to_vec());
    }
}
