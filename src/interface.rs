pub(crate) const FIRMWARE_VID: u16 = 0x04B4;
pub(crate) const FIRMWARE_PID: u16 = 0x3DDC;
pub(crate) const BOOTLOADER_PID: u16 = 0x00f3;

pub(crate) const FIRMWARE_VER_MAJOR: u32 = 3;
pub(crate) const FIRMWARE_VER_MINOR: u32 = 0;

#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
#[repr(u8)]
pub(crate) enum FX3Command {
    // Write Register
    // INDEX: register address
    // READ/Write: UINT32
    // Register operation (direction determines read vs write)
    // INDEX: register address
    // DATA:
    //  - control_out (host-to-device): UINT32 payload to write
    //  - control_in  (device-to-host): UINT32 payload returned
    REGOP = 0x01,

    // (WRITE_REG merged into REGOP; use control_out)

    // Read/Write Non-Volatile I2C memory
    // INDEX: memory address
    // VALUE: length
    // READ/Write: array of uint8
    // Note: limit the write length to page size to avoid wrap
    NVMOP = 0x03,
}

// All registers are read and writable, all register is 32bit integer.
#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register {
    // reset the device if write, read will return 0
    REG_INFO_RESET = 0x00,

    // ADC sampling frequency, will change IF filter selection
    // | UINT32 frequency in Hz |
    REG_ADCFREQ = 0x01,

    // ADC settings like enable/disable, dither, rand, PGA.
    // Disable ADC will also stop data streaming.
    // | Byte 0: bit0 = ADC enable/disable,
    //           bit1 = dither enable/disable,
    //           bit2 = randomizer enable/disable,
    //           bit3 = PGA enable/disable |
    REG_ADC = 0x02,

    /// External GPIO register for controlling GPIO output state.
    ///
    /// Allows read/write of GPIO pin states (7 pins) through the I/O expander.
    ///
    /// When writing:
    /// - Bits 0-6 (0x7F): New GPIO output state to apply
    REG_EXT_GPIO = 0x03,

    // direct sampling or tuner mode,
    // | Bit 0 (RW): Tuner Enable: 0 = Direct sampling, 1 = Tuner mode, (RW)
    // | Bit 1 (R_): PLL Lock : 0 = Unlock, 1 = Locked
    // | Bit 2 (R_): Harmonic mode enable: 0 = fundamental, 1 = harmonic
    REG_TUNER = 0x82,

    REG_DIRECT_IF_GAIN = 0x90,
    REG_DIRECT_RF_GAIN = 0x91,
    REG_DIRECT_ANT_BIAS = 0x92,
    // PRO ONLY: 0 - 64MHz (default), 1 - 32MHz, 2 - FM undersampling, 3 - Bypass mode
    REG_DIRECT_ADC_FILTER = 0x93,
    REG_DIRECT_PREAMP = 0x98,

    REG_TUNER_IF_GAIN = 0xa0,
    REG_TUNER_RF_GAIN = 0xa1,
    REG_TUNER_ANT_BIAS = 0xa2,
    REG_TUNER_PREAMP = 0xa8,

    // low 32bit of freq, up to 4.29GHz, this will impact IF frequency selection as well as preselector
    // if tuner is not active, this is no-op. Read value is undetermined
    REG_TUNER_CENTER_FREQ_LOW = 0xa3,
    // this will not trigger freq change, you have to write to previous reg to apply
    REG_TUNER_CENTER_FREQ_HIGH = 0xa4,
}

// ADC register bit flags
pub(crate) const REG_ADC_ENABLE: u8 = 1 << 0;
pub(crate) const REG_ADC_DITHER: u8 = 1 << 1;
pub(crate) const REG_ADC_RANDO: u8 = 1 << 2;
pub(crate) const REG_ADC_PGA: u8 = 1 << 3;
pub(crate) const REG_HF_HIGHZ: u8 = 1 << 4;
pub(crate) const REG_EXT_CLOCK: u8 = 1 << 5;

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioModel {
    NORADIO = 0x00,
    // BBRF103 = 0x01,
    // HF103 = 0x02,
    RX888 = 0x03,
    RX888r2 = 0x04,
    RX888plus = 0x05,
    // RXLUCY = 0x06,
    RX888pro = 0x07,
}
