use anyhow::{Context, Result};
use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient};
use nusb::{Device, MaybeFuture};
use std::{num::Wrapping, time::Duration};

const RW_RAM: u8 = 0xA0;
// const RW_SPI: u8 = 0xC2;
// const ERASE_SPI: u8 = 0xC4;

const FIRMWARE_HEADER_SIZE: usize = 4;
const CHUNK_SIZE: usize = 4096;
const WORD_SIZE: usize = 4;
const CONTROL_TIMEOUT: Duration = Duration::from_secs(1);

/// Validates the firmware file header
fn validate_firmware_header(firmware: &[u8]) -> Result<()> {
    anyhow::ensure!(
        firmware.len() >= FIRMWARE_HEADER_SIZE,
        "Firmware file too small"
    );

    anyhow::ensure!(
        firmware[0] == b'C' && firmware[1] == b'Y',
        "Invalid firmware header: expected 'CY'"
    );

    anyhow::ensure!(
        firmware[3] == 0xB0,
        "Unsupported image type (expected 0xB0)"
    );

    Ok(())
}

/// Reads a 32-bit little-endian value from the firmware at the given offset
fn read_u32_le(firmware: &[u8], offset: usize) -> Result<u32> {
    firmware
        .get(offset..offset + WORD_SIZE)
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_le_bytes)
        .context("Firmware data truncated")
}

/// Downloads firmware to the FX3 device
pub fn download_firmware(device: &Device, firmware: &[u8]) -> Result<()> {
    validate_firmware_header(firmware)?;

    let mut checksum = Wrapping(0u32);
    let interface = device.claim_interface(0).wait()?;

    let mut offset = FIRMWARE_HEADER_SIZE;
    let jump_address = loop {
        let length = read_u32_le(firmware, offset)?;
        offset += WORD_SIZE;
        let address = read_u32_le(firmware, offset)?;
        offset += WORD_SIZE;

        if length == 0 {
            break address;
        }

        let data_size = (length as usize) * WORD_SIZE;
        println!("Loading {} bytes to address {:#010x}", data_size, address);

        let data = firmware
            .get(offset..offset + data_size)
            .context("Firmware data truncated")?;
        offset += data_size;

        checksum += data
            .chunks_exact(WORD_SIZE)
            .map(|chunk| Wrapping(u32::from_le_bytes(chunk.try_into().unwrap())))
            .sum::<Wrapping<u32>>();

        for (chunk_idx, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
            let addr = address + (chunk_idx as u32 * CHUNK_SIZE as u32);

            write_ram(&interface, addr, chunk)?;
            let readback_data = read_ram(&interface, addr, CHUNK_SIZE as u16)?;

            if chunk != &readback_data[..chunk.len()] {
                anyhow::bail!("Data verification failed at address {:#010x}", addr);
            }
        }
    };

    println!("Jump address: {:#010x}", jump_address);

    let firmware_checksum = read_u32_le(firmware, offset)?;

    anyhow::ensure!(
        checksum.0 == firmware_checksum,
        "Checksum mismatch: calculated {:#010x}, expected {:#010x}",
        checksum.0,
        firmware_checksum
    );
    println!("Checksum verified: {:#010x}", checksum.0);

    // Execute firmware at jump address
    run_program(&interface, jump_address)?;

    Ok(())
}

/// Executes firmware by jumping to the specified address
fn run_program(interface: &nusb::Interface, address: u32) -> Result<()> {
    write_ram(interface, address, &[])
}

/// Writes data to RAM via USB control transfer
fn write_ram(interface: &nusb::Interface, address: u32, data: &[u8]) -> Result<()> {
    interface
        .control_out(
            ControlOut {
                control_type: ControlType::Vendor,
                recipient: Recipient::Device,
                request: RW_RAM,
                value: (address & 0xFFFF) as u16,
                index: (address >> 16) as u16,
                data,
            },
            CONTROL_TIMEOUT,
        )
        .wait()
        .context("USB write failed")
}

/// Reads data from RAM via USB control transfer
fn read_ram(interface: &nusb::Interface, address: u32, length: u16) -> Result<Vec<u8>> {
    interface
        .control_in(
            ControlIn {
                control_type: ControlType::Vendor,
                recipient: Recipient::Device,
                request: RW_RAM,
                value: (address & 0xFFFF) as u16,
                index: (address >> 16) as u16,
                length,
            },
            CONTROL_TIMEOUT,
        )
        .wait()
        .context("USB read failed")
}
