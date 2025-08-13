use embedded_hal::i2c::I2c;
use linux_embedded_hal::I2cdev;

// might be useful to reference https://github.com/kriswiner/IS31FL3194/blob/master/IS31FL3194.basic.ino for something more complex

const ADDR: u8 = 0x53;
#[allow(dead_code)]
const REG_PRODUCT_ID: u8 = 0x00;
const REG_OP_CONFIG: u8 = 0x01;
const REG_OUT_CONFIG: u8 = 0x02;
const REG_CURRENT_BAND: u8 = 0x03;
#[allow(dead_code)]
const REG_HOLD_FUNCTION: u8 = 0x04;

#[allow(dead_code)]
const REG_OUT1: u8 = 0x10;
#[allow(dead_code)]
const REG_OUT2: u8 = 0x21;
#[allow(dead_code)]
const REG_OUT3: u8 = 0x32;

// eight sleep expects BRG 😢
const REG_P1_COLOR_B: u8 = 0x10;
const REG_P1_COLOR_R: u8 = 0x11;
const REG_P1_COLOR_G: u8 = 0x12;

const REG_P1_TS_T1: u8 = 0x19;
const REG_P1_T2_T3: u8 = 0x1A;
const REG_P1_TP_T4: u8 = 0x1B;
const REG_P1_COLOR_EN: u8 = 0x1C;
const REG_P1_NXT: u8 = 0x1E;

const REG_COLOR_UPDATE: u8 = 0x40;
const REG_P1_UPDATE: u8 = 0x41;
const REG_RESET: u8 = 0x4F;

const RESET_VALUE: u8 = 0xC5;
const UPDATE_VALUE: u8 = 0xC5;

/// IS31FL3194 LED controller
pub struct LEDController {
    dev: I2cdev,
}

impl LEDController {
    pub fn new(dev: I2cdev) -> Self {
        Self { dev }
    }

    fn write_reg(&mut self, reg: u8, value: u8) -> Result<(), Box<dyn std::error::Error>> {
        self.dev.write(ADDR, &[reg, value])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.write_reg(REG_RESET, RESET_VALUE)?;
        Ok(())
    }

    pub fn set_rgb(&mut self, r: u8, g: u8, b: u8) -> Result<(), Box<dyn std::error::Error>> {
        // config for current mode
        self.write_reg(REG_OP_CONFIG, 0x01)?; // normal operation, current mode
        self.write_reg(REG_OUT_CONFIG, 0x07)?; // enable all outputs
        self.write_reg(REG_CURRENT_BAND, 0x00)?; // 10mA max current

        self.write_reg(REG_OUT1, b)?;
        self.write_reg(REG_OUT2, r)?;
        self.write_reg(REG_OUT3, g)?;

        self.write_reg(REG_COLOR_UPDATE, UPDATE_VALUE)?;
        Ok(())
    }

    pub fn start_breathing(
        &mut self,
        color: (u8, u8, u8),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (r, g, b) = color;

        self.write_reg(REG_OP_CONFIG, 0x75)?; // normal operation, pattern mode, RGB mode
        self.write_reg(REG_OUT_CONFIG, 0x07)?; // enable all outputs
        self.write_reg(REG_CURRENT_BAND, 0x2A)?; // Imax = 30mA for all

        self.write_reg(REG_P1_COLOR_B, b)?;
        self.write_reg(REG_P1_COLOR_R, r)?;
        self.write_reg(REG_P1_COLOR_G, g)?;

        self.write_reg(REG_P1_TS_T1, 0x60)?; // [4:0 start time], [7:3 rise time]
        self.write_reg(REG_P1_T2_T3, 0x66)?; // [4:0 hold time], [7:3 fall time]
        self.write_reg(REG_P1_TP_T4, 0x60)?; // [4:0 time btw pulses], [7:3 off time]

        self.write_reg(REG_P1_COLOR_EN, 0x01)?; // [1] enable color
        self.write_reg(REG_P1_NXT, 0x05)?; // gamma = 3.5, goto next pattern

        // update pattern 1 and start
        self.write_reg(REG_P1_UPDATE, UPDATE_VALUE)?;

        // update color reg
        self.write_reg(REG_COLOR_UPDATE, UPDATE_VALUE)?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn off(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // FIXME
        self.set_rgb(0, 0, 0)
    }
}
