use super::codec::PacketCodec;
use super::packet::Packet;
use std::time::Duration;
use strum_macros::Display;
use thiserror::Error;
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, SerialStream, StopBits};
use tokio_util::codec::Framed;

#[derive(Error, Debug)]
pub enum SerialError {
    #[error("Serial port error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serial port error: {0}")]
    Serial(#[from] tokio_serial::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Display)]
pub enum DeviceMode {
    #[default]
    Unknown,
    Bootloader,
    Firmware,
}

impl DeviceMode {
    pub fn from_pong(in_firmware: bool) -> Self {
        if in_firmware {
            Self::Firmware
        } else {
            Self::Bootloader
        }
    }
}

pub fn create_port(port_path: &str, baud_rate: u32) -> Result<SerialStream, SerialError> {
    let port = tokio_serial::new(port_path, baud_rate)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_millis(1000))
        .open_native_async()?;

    Ok(port)
}

pub fn create_framed_port<P: Packet>(
    port_path: &str,
    baud_rate: u32,
) -> Result<Framed<SerialStream, PacketCodec<P>>, SerialError> {
    let port = create_port(port_path, baud_rate)?;
    Ok(Framed::new(port, PacketCodec::new()))
}
