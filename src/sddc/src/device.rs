use crate::interface::RadioModel;
use anyhow::Result;

/// Type alias for the callback function used in read_async
pub type AsyncCallback = Box<dyn Fn(&[i16]) + Send + Sync + 'static>;

/// Trait that abstracts an SDR-like device used by `VirtualRadio`.
/// Implemented by both the real `Radio` and the `MockSDR` for testing.
pub trait SdrDevice: Send {
    fn set_xtal_freq(&mut self, freq: u32) -> Result<()>;
    fn get_xtal_freq(&self) -> u32;

    fn set_direct_sampling(&mut self, mode: bool) -> Result<()>;
    fn get_direct_sampling(&self) -> bool;

    fn set_center_freq(&mut self, freq: u64) -> Result<()>;
    fn get_center_freq(&self) -> u64;

    fn set_if_gain(&mut self, gain: f32) -> Result<()>;
    fn get_if_gain(&self) -> f32;

    fn set_rf_gain(&mut self, gain: f32) -> Result<()>;
    fn get_rf_gain(&self) -> f32;

    fn get_if_gain_range(&self) -> (f32, f32);
    fn get_if_gain_steps(&self) -> &'static [f32];

    fn get_rf_gain_range(&self) -> (f32, f32);
    fn get_rf_gain_steps(&self) -> &'static [f32];

    fn enable_adc_dither(&mut self, enable: bool) -> Result<()>;
    fn enable_adc_pga(&mut self, enable: bool) -> Result<()>;
    fn enable_antenna_bias(&mut self, index: i32, enable: bool) -> Result<()>;

    fn read_async(&mut self, cb: AsyncCallback) -> Result<()>;
    fn read_cancel(&mut self) -> Result<()>;

    fn get_model(&self) -> RadioModel;
    fn get_firmware_version(&self) -> u16;
}
