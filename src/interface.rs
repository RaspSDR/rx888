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

    // direct sampling or tuner mode,
    // | Byte 0: 0 = Tuner mode, 1 = Direct sampling |
    REG_TUNER = 0x82,

    REG_DIRECT_IF_GAIN = 0x90,
    REG_DIRECT_RF_GAIN = 0x91,
    REG_DIRECT_ANT_BIAS = 0x92,

    REG_TUNER_IF_GAIN = 0xa0,
    REG_TUNER_RF_GAIN = 0xa1,
    REG_TUNER_ANT_BIAS = 0xa2,

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
}
