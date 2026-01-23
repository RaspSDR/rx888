use anyhow::{Context, Result};
use nusb::MaybeFuture;
use nusb::transfer as ntransfer;
use std::time::Duration;

/// Simplified USB API used only by the flasher.
pub trait UsbInterface {
    /// Control OUT (host -> device)
    fn control_write(
        &self,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Duration,
    ) -> Result<()>;

    /// Control IN (device -> host)
    fn control_read(
        &self,
        request: u8,
        value: u16,
        index: u16,
        length: u16,
        timeout: Duration,
    ) -> Result<Vec<u8>>;
}

impl UsbInterface for nusb::Interface {
    fn control_write(
        &self,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Duration,
    ) -> Result<()> {
        let out = ntransfer::ControlOut {
            control_type: ntransfer::ControlType::Vendor,
            recipient: ntransfer::Recipient::Device,
            request,
            value,
            index,
            data,
        };
        self.control_out(out, timeout)
            .wait()
            .context("USB write failed")
    }

    fn control_read(
        &self,
        request: u8,
        value: u16,
        index: u16,
        length: u16,
        timeout: Duration,
    ) -> Result<Vec<u8>> {
        let inp = ntransfer::ControlIn {
            control_type: ntransfer::ControlType::Vendor,
            recipient: ntransfer::Recipient::Device,
            request,
            value,
            index,
            length,
        };
        self.control_in(inp, timeout)
            .wait()
            .context("USB read failed")
    }
}
