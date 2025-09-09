use std::fmt;

use bytes::{Buf, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, FromRepr};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum PacketError {
    #[error("{packet_name} wrong size: expected {expected}, got {actual}. Buffer: {buf:02X?}")]
    InvalidSize {
        packet_name: &'static str,
        expected: usize,
        actual: usize,
        buf: Bytes,
    },
    #[error(
        "{packet_name} too small: expected at least {min_size}, got {actual}. Buffer: {buf:02X?}"
    )]
    TooSmall {
        packet_name: &'static str,
        min_size: usize,
        actual: usize,
        buf: Bytes,
    },
    #[error("{packet_name} had invalid structure: {error}. Buffer: {buf:02X?}")]
    InvalidStructure {
        packet_name: &'static str,
        error: String,
        buf: Bytes,
    },
    #[error("UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error("Unexpected pong code: 0x{0:0X}")]
    UnexpectedPongCode(u8),
    #[error("{packet_name} had invalid bed side value: {bed_side}")]
    InvalidBedSide {
        packet_name: &'static str,
        bed_side: u8,
    },
    #[error("{subsystem_name} got unexpected packet: {buf:02X?}")]
    Unexpected {
        subsystem_name: &'static str,
        buf: Bytes,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Display, FromRepr, PartialEq, Eq)]
#[repr(u8)]
pub enum BedSide {
    Left = 0x00,
    Right = 0x01,
}

pub trait Packet: Sized {
    fn parse(buf: BytesMut) -> Result<Self, PacketError>;
}

pub fn validate_packet_size(
    packet_name: &'static str,
    buf: &BytesMut,
    expected: usize,
) -> Result<(), PacketError> {
    if buf.len() != expected {
        return Err(PacketError::InvalidSize {
            packet_name,
            expected,
            actual: buf.len(),
            buf: buf.clone().freeze(),
        });
    }
    Ok(())
}

pub fn validate_packet_at_least(
    packet_name: &'static str,
    buf: &BytesMut,
    min_size: usize,
) -> Result<(), PacketError> {
    if buf.len() < min_size {
        return Err(PacketError::TooSmall {
            packet_name,
            min_size,
            actual: buf.len(),
            buf: buf.clone().freeze(),
        });
    }
    Ok(())
}

pub fn invalid_structure(packet_name: &'static str, error: String, buf: BytesMut) -> PacketError {
    PacketError::InvalidStructure {
        packet_name,
        error,
        buf: buf.freeze(),
    }
}

/// returns true next state is firmware mode
pub fn parse_pong(packet_name: &'static str, buf: BytesMut) -> Result<bool, PacketError> {
    validate_packet_size(packet_name, &buf, 3)?;

    if buf[0] == 0x81 && buf[1] != 0 {
        return Err(invalid_structure(
            "Pong",
            "missing reserved bytes".to_string(),
            buf,
        ));
    }

    match buf[2] {
        0b0100_0110 => Ok(true),
        0b0100_0010 => Ok(false),
        _ => Err(PacketError::UnexpectedPongCode(buf[2])),
    }
}

pub fn parse_message(packet_name: &'static str, mut buf: BytesMut) -> Result<String, PacketError> {
    validate_packet_at_least(packet_name, &buf, 3)?;
    buf.advance(2);
    Ok(String::from_utf8(buf.into())?)
}

pub fn parse_jumping_to_firmware(
    packet_name: &'static str,
    buf: BytesMut,
) -> Result<u8, PacketError> {
    validate_packet_size(packet_name, &buf, 2)?;
    Ok(buf[1])
}

pub fn parse_hardware_info(
    packet_name: &'static str,
    buf: BytesMut,
) -> Result<HardwareInfo, PacketError> {
    let (status_code, hardware_info): (u8, HardwareInfo) = cbor4ii::serde::from_slice(&buf)
        .map_err(|e| {
            invalid_structure(
                packet_name,
                format!("Failed to parse CBOR hardware info: {e}"),
                buf,
            )
        })?;

    if status_code != 0 {
        log::warn!("Unexpected {packet_name} status code: {status_code}");
    }

    Ok(hardware_info)
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    #[serde(rename = "devicesn")]
    pub serial_number: u32,
    #[serde(rename = "pn")]
    pub part_number: u32,
    pub sku: u32,
    pub hwrev: u32,
    pub factoryline: u32,
    pub datecode: u32,
}

impl fmt::Display for HardwareInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SN {:08x} PN {} SKU {} HWREV {:04x} FACTORYFLAG {} DATECODE {:06x}",
            self.serial_number,
            self.part_number,
            self.sku,
            self.hwrev,
            self.factoryline,
            self.datecode
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use hex_literal::hex;

    #[test]
    fn test_parse_pong_firmware() {
        let buf = BytesMut::from(&[0x81, 0x00, 0b0100_0110][..]);
        assert!(parse_pong("Test/Pong", buf).unwrap());
    }

    #[test]
    fn test_parse_pong_bootloader() {
        let buf = BytesMut::from(&[0x81, 0x00, 0b0100_0010][..]);
        assert!(!parse_pong("Test/Pong", buf).unwrap());
    }

    #[test]
    fn test_parse_pong_frozen_format() {
        let buf = BytesMut::from(&[0x81, 0x00, 0b0100_0110][..]);
        assert!(parse_pong("Test/Pong", buf).unwrap());
    }

    #[test]
    fn test_parse_pong_invalid_size() {
        let buf = BytesMut::from(&[0x81, 0x00][..]);
        match parse_pong("Test/Pong", buf) {
            Err(PacketError::InvalidSize {
                packet_name: _,
                expected,
                actual,
                buf: _,
            }) => {
                assert_eq!(expected, 3);
                assert_eq!(actual, 2);
            }
            _ => panic!("Expected InvalidSize error"),
        }
    }

    #[test]
    fn test_parse_pong_invalid_spacer() {
        let buf = BytesMut::from(&[0x81, 0xFF, 0b0100_0110][..]);
        match parse_pong("Test/Pong", buf) {
            Err(PacketError::InvalidStructure { .. }) => {}
            _ => panic!("Expected InvalidStructure error"),
        }
    }

    #[test]
    fn test_parse_pong_unexpected_code() {
        let buf = BytesMut::from(&[0x81, 0x00, 0xFF][..]);
        match parse_pong("Test/Pong", buf) {
            Err(PacketError::UnexpectedPongCode(code)) => {
                assert_eq!(code, 0xFF);
            }
            _ => panic!("Expected UnexpectedPongCode error"),
        }
    }

    #[test]
    fn test_parse_message_valid() {
        let msg = "Hello World";
        let mut buf = BytesMut::from(&[0x07, 0x00][..]);
        buf.extend_from_slice(msg.as_bytes());

        assert_eq!(parse_message("Test/Message", buf).unwrap(), "Hello World");
    }

    #[test]
    fn test_parse_message_empty() {
        let buf = BytesMut::from(&[0x07, 0x00][..]);
        match parse_message("Test/Message", buf) {
            Err(PacketError::TooSmall {
                packet_name: _,
                min_size,
                actual,
                buf: _,
            }) => {
                assert_eq!(min_size, 3);
                assert_eq!(actual, 2);
            }
            _ => panic!("Expected InvalidSize error"),
        }
    }

    #[test]
    fn test_parse_message_utf8_valid() {
        let msg = "Hello 世界";
        let mut buf = BytesMut::from(&[0x07, 0x00][..]);
        buf.extend_from_slice(msg.as_bytes());

        assert_eq!(parse_message("Test/Message", buf).unwrap(), "Hello 世界");
    }

    #[test]
    fn test_parse_message_invalid_utf8() {
        let buf = BytesMut::from(&[0x07, 0x00, 0xFF, 0xFE, 0xFD, 0xFC, 0xFB][..]);
        match parse_message("Test/Message", buf) {
            Err(PacketError::Utf8Error(_)) => {}
            _ => panic!("Expected Utf8Error"),
        }
    }

    #[test]
    fn test_parse_jumping_to_firmware_valid() {
        let buf = BytesMut::from(&[0x90, 0x00][..]);
        assert_eq!(
            parse_jumping_to_firmware("Test/JumpingToFirmware", buf).unwrap(),
            0x00
        );

        let buf = BytesMut::from(&[0x90, 0x10][..]);
        assert_eq!(
            parse_jumping_to_firmware("Test/JumpingToFirmware", buf).unwrap(),
            0x10
        );

        let buf = BytesMut::from(&[0x90, 0xFF][..]);
        assert_eq!(
            parse_jumping_to_firmware("Test/JumpingToFirmware", buf).unwrap(),
            0xFF
        );
    }

    #[test]
    fn test_parse_jumping_to_firmware_invalid_size() {
        let buf = BytesMut::from(&[0x90][..]);
        match parse_jumping_to_firmware("Test/JumpingToFirmware", buf) {
            Err(PacketError::InvalidSize {
                packet_name: _,
                expected,
                actual,
                buf: _,
            }) => {
                assert_eq!(expected, 2);
                assert_eq!(actual, 1);
            }
            _ => panic!("Expected InvalidSize error"),
        }

        let buf = BytesMut::from(&[0x90, 0x00, 0x00][..]);
        match parse_jumping_to_firmware("Test/JumpingToFirmware", buf) {
            Err(PacketError::InvalidSize {
                packet_name: _,
                expected,
                actual,
                buf: _,
            }) => {
                assert_eq!(expected, 2);
                assert_eq!(actual, 3);
            }
            _ => panic!("Expected InvalidSize error"),
        }
    }

    #[test]
    fn test_hardware_info() {
        let data = hex!(
            "
            82 00 A6 63 73 6B 75 01 68 64 61 74 65
            63 6F 64 65 1A 00 16 01 0D 6B 66 61 63
            74 6F 72 79 6C 69 6E 65 01 65 68 77 72
            65 76 19 05 00 62 70 6E 19 50 78 68 64
            65 76 69 63 65 73 6E 1A 00 01 08 9C FF
            FF FF FF FF FF FF FF FF FF FF FF FF FF
            FF FF FF FF FF FF FF FF FF FF FF FF FF
            FF FF FF FF FF FF FF FF FF FF FF FF FF
            FF FF FF FF FF FF FF FF FF FF FF FF FF
            FF FF FF FF FF FF FF FF FF FF FF FF FF 
            "
        );

        let result = parse_hardware_info("Test/HardwareInfo", BytesMut::from(&data[..])).unwrap();
        assert_eq!(result.serial_number, 0x0001089C);
        assert_eq!(result.part_number, 20600);
        assert_eq!(result.sku, 1);
        assert_eq!(result.hwrev, 0x0500);
        assert_eq!(result.factoryline, 1);
        assert_eq!(result.datecode, 0x16010D);
    }
}
