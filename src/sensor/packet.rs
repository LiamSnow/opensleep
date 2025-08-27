use bytes::BytesMut;
use hex_literal::hex;

use crate::common::packet::{
    self, HardwareInfo, Packet, PacketError, invalid_structure, validate_packet_at_least,
    validate_packet_size,
};

#[derive(Debug, PartialEq)]
pub enum SensorPacket {
    /// next state, where bootloader = false, firmware = true
    Pong(bool),
    Message(String),
    HardwareInfo(HardwareInfo),
    /// unknown value
    JumpingToFirmware(u8),
    PiezoGainSet(u16, u16),
    /// unknown value, always (0,2)
    VibrationEnabled(u8, u8),
    /// unknown value, always 4
    GetFirmware(u8),
    /// unknown value, always 0
    PiezoFreqSet(u8),
    /// unknown value, always 0
    PiezoEnabled(u8),
    /// occurs in BL -> FW transition
    Init(u16),
    Capacitance(CapacitanceData),
    Piezo(PiezoData),
    Temperature(TemperatureData),
    /// unknown value, usually 172
    AlarmSet(u8),
}

#[derive(Debug, PartialEq, Clone)]
pub struct CapacitanceData {
    pub sequence: u32,
    /// ordered LTR
    pub values: [u16; 6],
}

#[derive(Debug, PartialEq, Clone)]
pub struct TemperatureData {
    /// ordered LTR
    /// centidegrees celcius
    pub bed: [u16; 8],
    /// centidegrees celcius
    pub ambient: u16,
    /// centidegrees celcius
    pub humidity: u16,
    /// centidegrees celcius
    pub microcontroller: u16,
}

#[derive(Debug, PartialEq, Clone)]
pub struct PiezoData {
    pub freq: u32,
    pub sequence: u32,
    pub gain: (u16, u16),
    pub left_samples: Vec<u16>,
    pub right_samples: Vec<u16>,
}

impl Packet for SensorPacket {
    // responses are cmd + 0x80
    fn parse(buf: BytesMut) -> Result<Self, PacketError> {
        match buf[0] {
            0x07 => packet::parse_message("Sensor/Message", buf).map(SensorPacket::Message),
            0x31 => Self::parse_init(buf),
            0x32 => Self::parse_piezo(buf),
            0x33 => Self::parse_capacitance(buf),
            0x81 => packet::parse_pong("Sensor/Pong", buf).map(SensorPacket::Pong),
            0x82 => packet::parse_hardware_info("Sensor/HardwareInfo", buf)
                .map(SensorPacket::HardwareInfo),
            0x84 => Self::parse_get_firmware(buf),
            0x90 => packet::parse_jumping_to_firmware("Sensor/JumpingToFirmware", buf)
                .map(SensorPacket::JumpingToFirmware),
            0xA1 => Self::parse_piezo_freq_set(buf),
            0xA8 => Self::parse_piezo_enabled(buf),
            0xAB => Self::parse_piezo_gain_set(buf),
            0xAC => Self::parse_alarm_set(buf),
            0xAE => Self::parse_vibration_enabled(buf),
            0xAF => Self::parse_temperature(buf),
            _ => Err(PacketError::Unexpected {
                subsystem_name: "Sensor",
                buf: buf.freeze(),
            }),
        }
    }
}

impl SensorPacket {
    fn parse_get_firmware(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Sensor/GetFirmware", &buf, 2)?;
        Ok(SensorPacket::GetFirmware(buf[1]))
    }

    fn parse_alarm_set(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Sensor/AlarmSet", &buf, 2)?;
        Ok(SensorPacket::AlarmSet(buf[0]))
    }

    fn parse_piezo_gain_set(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Sensor/PiezoGainSet", &buf, 6)?;
        Ok(SensorPacket::PiezoGainSet(
            u16::from_be_bytes([buf[2], buf[3]]),
            u16::from_be_bytes([buf[4], buf[5]]),
        ))
    }

    fn parse_piezo_freq_set(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Sensor/PiezoFreqSet", &buf, 2)?;
        Ok(SensorPacket::PiezoFreqSet(buf[1]))
    }

    fn parse_piezo_enabled(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Sensor/PiezoEnabled", &buf, 2)?;
        Ok(SensorPacket::PiezoEnabled(buf[1]))
    }

    fn parse_vibration_enabled(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Sensor/VibrationEnabled", &buf, 3)?;
        Ok(SensorPacket::VibrationEnabled(buf[1], buf[2]))
    }

    // TODO FIXME new packet 31 00 00 00 0c 00 00 1d 22 00
    /// 31 00 00 00 0b 00 00 XX XX 00
    fn parse_init(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Sensor/Init", &buf, 10)?;

        if buf[1..=6] != hex!("00 00 00 0b 00 00") || buf[9] != 0 {
            log::warn!("Unexpected init packet: {buf:02X?}");
        }

        Ok(SensorPacket::Init(u16::from_be_bytes([buf[7], buf[8]])))
    }

    /// Direct indexing is pretty nasty here, but _should_ be faster than using BytesMut as a buffer.
    /// Strict tests are used to enforce behavior.
    /// If you have a better method please reach out to me!!
    fn parse_capacitance(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Sensor/Capacitance", &buf, 27)?;

        let indices_valid = buf[9] == 0
            && buf[12] == 1
            && buf[15] == 2
            && buf[18] == 3
            && buf[21] == 4
            && buf[24] == 5;

        if !indices_valid {
            return Err(invalid_structure(
                "Sensor/Capacitance",
                "invalid indices".to_string(),
                buf,
            ));
        }

        Ok(Self::Capacitance(CapacitanceData {
            sequence: u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]),
            values: [
                u16::from_be_bytes([buf[10], buf[11]]),
                u16::from_be_bytes([buf[13], buf[14]]),
                u16::from_be_bytes([buf[16], buf[17]]),
                u16::from_be_bytes([buf[19], buf[20]]),
                u16::from_be_bytes([buf[22], buf[23]]),
                u16::from_be_bytes([buf[25], buf[26]]),
            ],
        }))
    }

    /// see parse_capacitance doc comment
    fn parse_temperature(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_size("Sensor/Temperature", &buf, 35)?;

        let indices_valid = buf[1] == 0
            && buf[2] == 0
            && buf[5] == 1
            && buf[8] == 2
            && buf[11] == 3
            && buf[14] == 4
            && buf[17] == 5
            && buf[20] == 6
            && buf[23] == 7
            && buf[26] == 8
            && buf[29] == 9
            && buf[32] == 10;

        if !indices_valid {
            return Err(invalid_structure(
                "Sensor/Temperature",
                "invalid indices or spacer".to_string(),
                buf,
            ));
        }

        Ok(SensorPacket::Temperature(TemperatureData {
            bed: [
                u16::from_be_bytes([buf[3], buf[4]]),
                u16::from_be_bytes([buf[6], buf[7]]),
                u16::from_be_bytes([buf[9], buf[10]]),
                u16::from_be_bytes([buf[12], buf[13]]),
                u16::from_be_bytes([buf[15], buf[16]]),
                u16::from_be_bytes([buf[18], buf[19]]),
                u16::from_be_bytes([buf[21], buf[22]]),
                u16::from_be_bytes([buf[24], buf[25]]),
            ],
            ambient: u16::from_be_bytes([buf[27], buf[28]]),
            humidity: u16::from_be_bytes([buf[30], buf[31]]),
            microcontroller: u16::from_be_bytes([buf[33], buf[34]]),
        }))
    }

    /// see parse_capacitance doc comment
    /// common sizes: 174, 254, 202, 142, 178
    fn parse_piezo(buf: BytesMut) -> Result<Self, PacketError> {
        validate_packet_at_least("Sensor/Piezo", &buf, 20)?;

        if buf[1] != 0x02 {
            log::warn!("Unexpected Piezo header: {:02X}", buf[1]);
        }

        let freq = u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]);
        let sequence = u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]);
        let gain = (
            u16::from_be_bytes([buf[10], buf[11]]),
            u16::from_be_bytes([buf[12], buf[13]]),
        );

        let num_samples = (buf.len() - 14) >> 2;
        let mut left_samples = Vec::with_capacity(num_samples);
        let mut right_samples = Vec::with_capacity(num_samples);

        for sample_num in 0..num_samples {
            let idx = 14 + (sample_num << 2);
            left_samples.push(u16::from_be_bytes([buf[idx], buf[idx + 1]]));
            right_samples.push(u16::from_be_bytes([buf[idx + 2], buf[idx + 3]]));
        }

        Ok(SensorPacket::Piezo(PiezoData {
            freq,
            sequence,
            gain,
            left_samples,
            right_samples,
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
            SensorPacket::parse(BytesMut::from(&hex!("81 00 42")[..])),
            Ok(SensorPacket::Pong(false))
        );

        assert_eq!(
            SensorPacket::parse(BytesMut::from(&hex!("81 00 46")[..])),
            Ok(SensorPacket::Pong(true))
        );

        assert!(SensorPacket::parse(BytesMut::from(&hex!("81 01 01")[..])).is_err());
        assert!(SensorPacket::parse(BytesMut::from(&hex!("81 00 01")[..])).is_err());
    }

    #[test]
    fn test_jumping_to_firmware() {
        assert_eq!(
            SensorPacket::parse(BytesMut::from(&[0x90, 0x01][..])),
            Ok(SensorPacket::JumpingToFirmware(1))
        );
        assert!(SensorPacket::parse(BytesMut::from(&[0x90][..])).is_err());
    }

    #[test]
    fn test_set_gain() {
        let data = hex!("AB 00 01 95 01 95");
        assert_eq!(
            SensorPacket::parse(BytesMut::from(&data[..])),
            Ok(SensorPacket::PiezoGainSet(405, 405))
        );
        assert!(SensorPacket::parse(BytesMut::from(&hex!("AB 01")[..])).is_err());
    }

    #[test]
    fn test_vibration_enabled() {
        assert_eq!(
            SensorPacket::parse(BytesMut::from(&[0xAE, 0, 2][..])),
            Ok(SensorPacket::VibrationEnabled(0, 2))
        );
        assert!(SensorPacket::parse(BytesMut::from(&[0xAE, 1][..])).is_err());
    }

    #[test]
    fn test_get_fw() {
        assert_eq!(
            SensorPacket::parse(BytesMut::from(&[0x84, 4][..])),
            Ok(SensorPacket::GetFirmware(4))
        );
        assert!(SensorPacket::parse(BytesMut::from(&[0x84][..])).is_err());
    }

    #[test]
    fn test_message() {
        let data = hex!("07 00 48 65 6C 6C 6F");
        assert_eq!(
            SensorPacket::parse(BytesMut::from(&data[..])),
            Ok(SensorPacket::Message("Hello".into()))
        );

        let invalid_utf8 = hex!("07 00 FF");
        assert!(SensorPacket::parse(BytesMut::from(&invalid_utf8[..])).is_err());
    }

    #[test]
    fn test_capacitance() {
        let mut data = hex!(
            "33 01 02 03 04 00 00 00 00"
            "00 01 02"
            "01 03 04"
            "02 05 06"
            "03 07 08"
            "04 09 0A"
            "05 0B 0C"
        );
        assert_eq!(
            SensorPacket::parse(BytesMut::from(&data[..])),
            Ok(SensorPacket::Capacitance(CapacitanceData {
                sequence: 0x01020304,
                values: [0x0102, 0x0304, 0x0506, 0x0708, 0x090A, 0x0B0C]
            }))
        );

        // test bad index
        data[9] = 99;
        assert!(SensorPacket::parse(BytesMut::from(&data[..])).is_err());
    }

    #[test]
    fn test_piezo() {
        let data = hex!(
            "32 02 00 00"
            "03 E8"
            "00 00 00 01"
            "00 01"
            "00 01"
            "00 01 00 02"
            "00 03 00 04"
        );
        let parsed = SensorPacket::parse(BytesMut::from(&data[..])).unwrap();
        match parsed {
            SensorPacket::Piezo(piezo) => {
                assert_eq!(piezo.freq, 1000);
                assert_eq!(piezo.sequence, 1);
                assert_eq!(piezo.gain, (1, 1));
                assert_eq!(piezo.left_samples, vec![1, 3]);
                assert_eq!(piezo.right_samples, vec![2, 4]);
            }
            _ => panic!("Wrong packet type"),
        }

        assert!(SensorPacket::parse(BytesMut::from(&hex!("32 02 00 00")[..])).is_err());
    }

    #[test]
    fn test_bed_temp() {
        let data = hex!(
            "AF 00"
            "00 01 02"
            "01 03 04"
            "02 05 06"
            "03 07 08"
            "04 09 0A"
            "05 0B 0C"
            "06 0D 0E"
            "07 0F 10"
            "08 11 12"
            "09 13 14"
            "0A 15 16"
        );
        let parsed = SensorPacket::parse(BytesMut::from(&data[..])).unwrap();
        match parsed {
            SensorPacket::Temperature(temp) => {
                assert_eq!(
                    temp.bed,
                    [
                        0x0102, 0x0304, 0x0506, 0x0708, 0x090A, 0x0B0C, 0x0D0E, 0x0F10
                    ]
                );
                assert_eq!(temp.ambient, 0x1112);
                assert_eq!(temp.humidity, 0x1314);
                assert_eq!(temp.microcontroller, 0x1516);
            }
            _ => panic!("Wrong packet type"),
        }

        let mut bad_index = data;
        bad_index[32] = 0x99;
        let result = SensorPacket::parse(BytesMut::from(&bad_index[..]));
        assert!(result.is_err());
    }

    #[test]
    fn test_alarm_set() {
        assert_eq!(
            SensorPacket::parse(BytesMut::from(&[0xAC, 0x01][..])),
            Ok(SensorPacket::AlarmSet(0xAC))
        );
        assert!(SensorPacket::parse(BytesMut::from(&[0xAC][..])).is_err());
    }

    #[test]
    fn test_unexpected() {
        assert_eq!(
            SensorPacket::parse(BytesMut::from(&[0x99][..])),
            Err(PacketError::Unexpected {
                subsystem_name: "Sensor",
                buf: Bytes::from(&[0x99][..])
            })
        );
    }
}
