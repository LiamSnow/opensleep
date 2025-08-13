use embedded_hal::i2c::I2c;
use linux_embedded_hal::I2cdev;
use std::time::Duration;
use tokio::time::sleep;

const DEV: &str = "/dev/i2c-1";

const ADDR: u8 = 0x20;

const REG_OUTPUT_PORT_0: u8 = 0x02;
const REG_CONFIG_PORT_0: u8 = 0x06;
const REG_CONFIG_PORT_1: u8 = 0x07;

const PORT_0_CONFIG: u8 = 0b1111_1100; // pins 0,1 as outputs
const PORT_1_CONFIG: u8 = 0b0011_0001;
const OUTPUT_RESET: u8 = 0b1111_1111;
const OUTPUT_ENABLED: u8 = 0b1111_1101;

/// Reset Controller using the PCAL6416A (16-bit I2C Expander)
///   Datasheet: <https://www.nxp.com/docs/en/data-sheet/PCAL6416A.pdf>
///
/// ## Reset/Boot State
/// 1b 0e ff 3f 00 00 fc 31 XX XX XX XX XX XX XX XX
///
/// ## Enabled State
/// 19 0e fd 3f 00 00 fc 31 XX XX XX XX XX XX XX XX
pub struct ResetController {
    dev: I2cdev,
}

impl ResetController {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            dev: I2cdev::new(DEV)?,
        })
    }

    fn write_reg(&mut self, reg: u8, value: u8) -> Result<(), Box<dyn std::error::Error>> {
        self.dev.write(ADDR, &[reg, value])?;
        Ok(())
    }

    /// resets and enables subsystems (Frozen + Sensor)
    pub async fn reset_subsystems(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting Subsystems...");

        // config ports
        self.write_reg(REG_CONFIG_PORT_0, PORT_0_CONFIG)?;
        self.write_reg(REG_CONFIG_PORT_1, PORT_1_CONFIG)?;
        sleep(Duration::from_millis(10)).await;

        // assert reset
        self.write_reg(REG_OUTPUT_PORT_0, OUTPUT_RESET)?;
        sleep(Duration::from_millis(100)).await;

        // de-assert reset (enable)
        self.write_reg(REG_OUTPUT_PORT_0, OUTPUT_ENABLED)?;
        sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    pub fn take(self) -> I2cdev {
        self.dev
    }
}
