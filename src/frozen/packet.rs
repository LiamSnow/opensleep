use bytes::BytesMut;

use crate::common::packet::{
    self, BedSide, HardwareInfo, Packet, PacketError, validate_packet_size,
};

#[derive(Debug, PartialEq)]
pub enum FrozenPacket {
    /// next state (in_firmware)
    Pong(bool),
    HardwareInfo(HardwareInfo),
    /// unknown value
    JumpingToFirmware(u8),
    Message(String),
    /// unknown value, always (0,1)
    Heartbeat(u8, u8),
    TargetUpdate((BedSide, TargetUpdate)),
    TemperatureUpdate(TemperatureUpdate),
    PrimingStarted,
    GetFirmware,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TargetUpdate {
    pub state: bool,
    /// centidegrees celcius
    pub temp: u16,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TemperatureUpdate {
    /// centidegrees celcius
    pub left_temp: u16,
    /// centidegrees celcius
    pub right_temp: u16,
    /// centidegrees celcius
    pub heatsink_temp: u16,
    /// error in deg celcius
    pub error: u8,
    /// wrapping measurement count
    pub count: u8,
}

impl Packet for FrozenPacket {
    fn parse(buf: BytesMut) -> Result<Self, PacketError> {
        match buf[0] {
            0x07 => packet::parse_message("Frozen/Message", buf).map(FrozenPacket::Message),
            0x41 => Self::parse_state_update(buf),
            0x53 => Self::parse_heartbeat(buf),
            0x81 => packet::parse_pong("Frozen/Pong", buf).map(FrozenPacket::Pong),
            0x82 => packet::parse_hardware_info("Frozen/HardwareInfo", buf)
                .map(FrozenPacket::HardwareInfo),
            0x84 => Ok(FrozenPacket::GetFirmware),
            0x90 => packet::parse_jumping_to_firmware("Frozen/JumpingToFirmware", buf)
                .map(FrozenPacket::JumpingToFirmware),
            0xC0 => Self::parse_target_update(buf),
            0xD2 => Self::parse_priming_started(buf),
            _ => Err(PacketError::Unexpected {
                subsystem_name: "Frozen",
                buf: buf.freeze(),
            }),
        }
    }
}

impl FrozenPacket {
    fn parse_priming_started(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Frozen/PrimingStarted", &buf, 2)?;
        if buf[1] != 0 {
            log::warn!("PrimingStarted had unexpected value {}", buf[1]);
        }
        Ok(FrozenPacket::PrimingStarted)
    }

    fn parse_heartbeat(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Frozen/Heartbeat", &buf, 3)?;
        Ok(FrozenPacket::Heartbeat(buf[1], buf[2]))
    }

    /// 0xC0, 00, side, state, temp_high, temp_low
    fn parse_target_update(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Frozen/TargetUpdate", &buf, 6)?;

        let temp = u16::from_be_bytes([buf[4], buf[5]]);
        let side = BedSide::from_repr(buf[2]).ok_or(PacketError::InvalidBedSide {
            packet_name: "Frozen/TargetUpdate",
            bed_side: buf[2],
        })?;
        let state = buf[3] != 0;

        Ok(FrozenPacket::TargetUpdate((
            side,
            TargetUpdate { state, temp },
        )))
    }

    fn parse_state_update(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Frozen/StateUpdate", &buf, 9)?;

        Ok(FrozenPacket::TemperatureUpdate(TemperatureUpdate {
            left_temp: u16::from_be_bytes([buf[1], buf[2]]),
            right_temp: u16::from_be_bytes([buf[3], buf[4]]),
            heatsink_temp: u16::from_be_bytes([buf[5], buf[6]]),
            error: buf[7],
            count: buf[8],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{Bytes, BytesMut};
    use hex_literal::hex;

    #[test]
    fn test_pong() {
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&hex!("81 00 46")[..])),
            Ok(FrozenPacket::Pong(true))
        );

        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&hex!("81 00 42")[..])),
            Ok(FrozenPacket::Pong(false))
        );

        assert!(FrozenPacket::parse(BytesMut::from(&hex!("81 00 FF")[..])).is_err());
        assert!(FrozenPacket::parse(BytesMut::from(&hex!("81 00")[..])).is_err());
    }

    #[test]
    fn test_jumping_to_firmware() {
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&[0x90, 0x00][..])),
            Ok(FrozenPacket::JumpingToFirmware(0x00))
        );
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&[0x90, 0x10][..])),
            Ok(FrozenPacket::JumpingToFirmware(0x10))
        );
        assert!(FrozenPacket::parse(BytesMut::from(&[0x90][..])).is_err());
    }

    #[test]
    fn test_message() {
        let data = hex!("07 00 48 65 6C 6C 6F");
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&data[..])),
            Ok(FrozenPacket::Message("Hello".into()))
        );

        let invalid_utf8 = hex!("07 00 FF FE FD");
        assert!(FrozenPacket::parse(BytesMut::from(&invalid_utf8[..])).is_err());

        assert!(FrozenPacket::parse(BytesMut::from(&hex!("07 00")[..])).is_err());
    }

    #[test]
    fn test_heartbeat() {
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&[0x53, 0x00, 0x01][..])),
            Ok(FrozenPacket::Heartbeat(0x00, 0x01))
        );
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&[0x53, 0x01, 0x00][..])),
            Ok(FrozenPacket::Heartbeat(0x01, 0x00))
        );
        assert!(FrozenPacket::parse(BytesMut::from(&[0x53, 0x00][..])).is_err());
    }

    #[test]
    fn test_target_update() {
        let data = hex!("C0 00 00 01 0B B8");
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&data[..])),
            Ok(FrozenPacket::TargetUpdate((
                BedSide::Left,
                TargetUpdate {
                    state: true,
                    temp: 3000
                }
            )))
        );

        let data = hex!("C0 00 01 00 0A C0");
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&data[..])),
            Ok(FrozenPacket::TargetUpdate((
                BedSide::Right,
                TargetUpdate {
                    state: false,
                    temp: 2752
                }
            )))
        );

        // invalid bed side
        let data = hex!("C0 00 02 01 0B B8");
        let result = FrozenPacket::parse(BytesMut::from(&data[..]));
        assert!(matches!(result, Err(PacketError::InvalidBedSide { .. })));

        assert!(FrozenPacket::parse(BytesMut::from(&hex!("C0 00 00")[..])).is_err());
    }

    #[test]
    fn test_state_update() {
        let data = hex!("41 09 F6 0A 73 08 FC 09 00");
        let result = FrozenPacket::parse(BytesMut::from(&data[..])).unwrap();
        match result {
            FrozenPacket::TemperatureUpdate(state) => {
                assert_eq!(state.left_temp, 25.50);
                assert_eq!(state.right_temp, 26.75);
                assert_eq!(state.heatsink_temp, 23.00);
                assert_eq!(state.error, 9);
                assert_eq!(state.count, 0);
            }
            _ => panic!("Wrong packet type"),
        }

        let data = hex!("41 0B B8 0C 1C 0A 8C 0A FF");
        let result = FrozenPacket::parse(BytesMut::from(&data[..])).unwrap();
        match result {
            FrozenPacket::TemperatureUpdate(state) => {
                assert_eq!(state.left_temp, 30.00);
                assert_eq!(state.right_temp, 31.00);
                assert_eq!(state.heatsink_temp, 27.00);
                assert_eq!(state.error, 10); // cooling state
                assert_eq!(state.count, 255);
            }
            _ => panic!("Wrong packet type"),
        }

        assert!(FrozenPacket::parse(BytesMut::from(&hex!("41 00 00 00")[..])).is_err());
    }

    #[test]
    fn test_get_firmware() {
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&[0x84][..])),
            Ok(FrozenPacket::GetFirmware)
        );
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&[0x84, 0xFF, 0xFF][..])),
            Ok(FrozenPacket::GetFirmware)
        );
    }

    #[test]
    fn test_unexpected() {
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&[0x99, 0x01, 0x02][..])),
            Err(PacketError::Unexpected {
                subsystem_name: "Frozen",
                buf: Bytes::from(&[0x99, 0x01, 0x02][..])
            })
        );
        assert_eq!(
            FrozenPacket::parse(BytesMut::from(&[0xFF][..])),
            Err(PacketError::Unexpected {
                subsystem_name: "Frozen",
                buf: Bytes::from(&[0xFF][..])
            })
        );
    }
}
