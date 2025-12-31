use core::ptr;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

use crate::sdr::Radio;

// Opaque struct for FFI - cbindgen will generate this as an opaque pointer
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct sddc_dev_t {
    _private: core::marker::PhantomData<()>,
}
impl sddc_dev_t {
    // Helper to cast from *mut sddc_dev_t to our internal type
    unsafe fn as_device_mut<'a>(ptr: *mut sddc_dev_t) -> &'a mut Box<Radio> {
        unsafe { &mut *(ptr as *mut c_void as *mut Box<Radio>) }
    }

    unsafe fn as_device_ref<'a>(ptr: *const sddc_dev_t) -> &'a Radio {
        unsafe { &*(ptr as *const c_void as *const Box<Radio>) }
    }
}

// Helper macro to safely cast and dereference device handle
macro_rules! with_device {
    ($dev:expr, $body:expr) => {{
        if $dev.is_null() {
            return -1;
        }
        unsafe {
            let device = sddc_dev_t::as_device_mut($dev);
            let device = device.as_mut();
            $body(device)
        }
    }};
}

macro_rules! with_device_ref {
    ($dev:expr, $body:expr) => {{
        if $dev.is_null() {
            return -1;
        }
        unsafe {
            let device = sddc_dev_t::as_device_ref($dev);
            $body(device)
        }
    }};
}

#[allow(non_camel_case_types)]
pub type sddc_read_async_cb_t =
    Option<extern "C" fn(buf: *const i16, count: u32, ctx: *mut c_void)>;

unsafe fn write_empty_cstr(buf: *mut c_char) {
    if !buf.is_null() {
        unsafe {
            *buf = 0;
        }
    }
}

unsafe fn write_cstr_limited(buf: *mut c_char, s: &str, maxlen: usize) {
    if buf.is_null() || maxlen == 0 {
        return;
    }
    let mut bytes = s.as_bytes();
    if bytes.len() + 1 > maxlen {
        bytes = &bytes[..maxlen.saturating_sub(1)];
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buf, bytes.len());
        *buf.add(bytes.len()) = 0;
    }
}

/// Get the number of available SDR devices.
///
/// Returns: number of devices detected.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_device_count() -> u32 {
    let mut count = 0;
    loop {
        let device = Radio::find_device(count);
        if device.is_none() {
            break;
        }
        count += 1;
    }

    count + 1
}

/// Get device name by index.
///
/// - `index`: device index
///
/// Returns: pointer to a null-terminated C string or NULL on error.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_device_name(index: u32) -> *const c_char {
    let name = match Radio::find_device(index) {
        Some(device) => device
            .product_string()
            .map(|s| s.to_owned())
            .unwrap_or_else(|| "".to_owned()),
        None => return std::ptr::null(),
    };

    // Use a static buffer to hold the device name as a C string.
    static DEVICE_NAME: OnceLock<CString> = OnceLock::new();

    let cstr = DEVICE_NAME
        .get_or_init(|| CString::new(name).unwrap_or_else(|_| CString::new("").unwrap()));
    cstr.as_ptr()
}

/// Get USB device strings.
///
/// NOTE: Each string buffer must provide space for up to 256 bytes.
///
/// - `index`: device index
/// - `manufact`: manufacturer name buffer, may be NULL
/// - `product`: product name buffer, may be NULL
/// - `serial`: serial number buffer, may be NULL
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_device_usb_strings(
    index: u32,
    manufact: *mut c_char,
    product: *mut c_char,
    serial: *mut c_char,
) -> c_int {
    let device = match Radio::find_device(index) {
        Some(dev) => dev,
        None => {
            unsafe {
                write_empty_cstr(manufact);
                write_empty_cstr(product);
                write_empty_cstr(serial);
            }
            return -1;
        }
    };

    unsafe {
        if let Some(m) = device.manufacturer_string() {
            write_cstr_limited(manufact, m, 256);
        } else {
            write_empty_cstr(manufact);
        }
        if let Some(p) = device.product_string() {
            write_cstr_limited(product, p, 256);
        } else {
            write_empty_cstr(product);
        }
        if let Some(s) = device.serial_number() {
            write_cstr_limited(serial, s, 256);
        } else {
            write_empty_cstr(serial);
        }
    }
    0
}

/// Get device index by USB serial string descriptor.
///
/// - `serial`: serial string of the device
///
/// Returns:
/// - device index of first matching device
/// - -1 if `serial` is NULL
/// - -2 if no devices were found
/// - -3 if devices were found, but none matched
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_index_by_serial(serial: *const c_char) -> c_int {
    if serial.is_null() {
        return -1;
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(serial) };
    let serial_str = match cstr.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let mut idx = 0;
    let mut found_any = false;
    loop {
        let device = Radio::find_device(idx);
        if device.is_none() {
            break;
        }
        found_any = true;
        let dev = device.unwrap();
        if let Some(dev_serial) = dev.serial_number()
            && dev_serial == serial_str
        {
            return idx as c_int;
        }

        idx += 1;
    }

    if !found_any { -2 } else { -3 }
}

/// Open the device.
///
/// - `dev`: output device handle pointer
/// - `index`: device index
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_open(dev: *mut *mut sddc_dev_t, index: u32) -> c_int {
    if dev.is_null() {
        return -1;
    }

    if let Some(device) = Radio::find_device(index) {
        match Radio::new(device) {
            Ok(radio) => {
                let boxed: Box<Radio> = Box::new(radio);
                unsafe { *dev = Box::into_raw(Box::new(boxed)) as *mut sddc_dev_t };
                0
            }
            Err(_) => -1,
        }
    } else {
        -1
    }
}

/// Close the device opened by `sddc_open()`.
///
/// - `dev`: device handle
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_close(dev: *mut sddc_dev_t) -> c_int {
    if dev.is_null() {
        return -1;
    }
    unsafe {
        let boxed = Box::from_raw(dev as *mut c_void as *mut Box<Radio>);
        drop(boxed);
    }
    0
}

/// Get USB device strings for an open device.
///
/// NOTE: Each string buffer must provide space for up to 256 bytes.
///
/// - `dev`: device handle
/// - `manufact`: manufacturer name buffer, may be NULL
/// - `product`: product name buffer, may be NULL
/// - `serial`: serial number buffer, may be NULL
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_usb_strings(
    dev: *mut sddc_dev_t,
    manufact: *mut c_char,
    product: *mut c_char,
    serial: *mut c_char,
) -> c_int {
    if dev.is_null() {
        return -1;
    }
    // Note: SdrDevice trait doesn't expose USB strings for an open device
    // This would require downcasting to Radio, which is not safe with trait objects
    // Return empty strings for now
    unsafe {
        write_empty_cstr(manufact);
        write_empty_cstr(product);
        write_empty_cstr(serial);
    }
    0
}

/// Set ADC crystal oscillator frequency.
/// At the SDR device level, xtal_freq equals the sample rate.
///
/// Default is 62 MHz for most devices. Changing this affects
/// bandwidth and usable frequency in direct sampling mode.
/// Call only if you fully understand the implications.
///
/// - `dev`: device handle
/// - `rtl_freq`: ADC clock in Hz (sample rate)
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_xtal_freq(dev: *mut sddc_dev_t, rtl_freq: u32) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if device.set_xtal_freq(rtl_freq).is_ok() {
            0
        } else {
            -1
        }
    })
}

/// Get ADC crystal oscillator frequency.
///
/// - `dev`: device handle
/// - `rtl_freq`: out pointer for ADC clock in Hz
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_xtal_freq(dev: *mut sddc_dev_t, rtl_freq: *mut u32) -> c_int {
    if rtl_freq.is_null() {
        return -1;
    }
    with_device_ref!(dev, |device: &Radio| {
        *rtl_freq = device.get_xtal_freq();
        0
    })
}

/// Set the IF gain value.
///
/// - `dev`: device handle
/// - `value`: gain value
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_if_gain(dev: *mut sddc_dev_t, value: f32) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if device.set_if_gain(value).is_ok() {
            0
        } else {
            -1
        }
    })
}

/// Set the RF gain value.
///
/// - `dev`: device handle
/// - `value`: gain value
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_rf_gain(dev: *mut sddc_dev_t, value: f32) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if device.set_rf_gain(value).is_ok() {
            0
        } else {
            -1
        }
    })
}

/// Get the IF gain value.
///
/// - `dev`: device handle
/// - `value`: out pointer for gain value
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_if_gain(dev: *mut sddc_dev_t, value: *mut f32) -> c_int {
    if value.is_null() {
        return -1;
    }
    with_device_ref!(dev, |device: &Radio| {
        *value = device.get_if_gain();
        0
    })
}

/// Get the IF gain range.
///
/// - `dev`: device handle
/// - `min`: out pointer for minimum value
/// - `max`: out pointer for maximum value
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_if_gain_range(
    _dev: *mut sddc_dev_t,
    min: *mut f32,
    max: *mut f32,
) -> c_int {
    if min.is_null() || max.is_null() {
        return -1;
    }
    with_device_ref!(_dev, |device: &Radio| {
        let (mn, mx) = device.get_if_gain_range();
        *min = mn;
        *max = mx;
        0
    })
}

/// Get the IF gain steps.
///
/// - `dev`: device handle
/// - `steps`: out pointer to a steps array
///
/// Returns: number of steps on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_if_gain_steps(_dev: *mut sddc_dev_t, steps: *mut *const f32) -> c_int {
    if steps.is_null() {
        return -1;
    }
    with_device_ref!(_dev, |device: &Radio| {
        let slice = device.get_if_gain_steps();
        *steps = slice.as_ptr();
        slice.len() as c_int
    })
}

/// Get the RF gain value.
///
/// - `dev`: device handle
/// - `value`: out pointer for gain value
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_rf_gain(dev: *mut sddc_dev_t, value: *mut f32) -> c_int {
    if value.is_null() {
        return -1;
    }
    with_device_ref!(dev, |device: &Radio| {
        *value = device.get_rf_gain();
        0
    })
}

/// Get the RF gain range.
///
/// - `dev`: device handle
/// - `min`: out pointer for minimum value
/// - `max`: out pointer for maximum value
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_rf_gain_range(
    _dev: *mut sddc_dev_t,
    min: *mut f32,
    max: *mut f32,
) -> c_int {
    if min.is_null() || max.is_null() {
        return -1;
    }
    with_device_ref!(_dev, |device: &Radio| {
        let (mn, mx) = device.get_rf_gain_range();
        *min = mn;
        *max = mx;
        0
    })
}

/// Get the RF gain steps.
///
/// - `dev`: device handle
/// - `steps`: out pointer to a steps array
///
/// Returns: number of steps on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_rf_gain_steps(_dev: *mut sddc_dev_t, steps: *mut *const f32) -> c_int {
    if steps.is_null() {
        return -1;
    }
    with_device_ref!(_dev, |device: &Radio| {
        let slice = device.get_rf_gain_steps();
        *steps = slice.as_ptr();
        slice.len() as c_int
    })
}

/// Get the current center frequency in Hz.
///
/// - `dev`: device handle
///
/// Returns: frequency in Hz, or 0 on error.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_center_freq(dev: *mut sddc_dev_t) -> u32 {
    sddc_get_center_freq64(dev) as u32
}

/// Get the current center frequency in Hz (64-bit).
///
/// - `dev`: device handle
///
/// Returns: frequency in Hz, or 0 on error.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_center_freq64(dev: *mut sddc_dev_t) -> u64 {
    if dev.is_null() {
        return 0;
    }
    unsafe {
        let device = sddc_dev_t::as_device_ref(dev);
        device.get_center_freq()
    }
}

/// Set the center frequency for the device.
///
/// - `dev`: device handle
/// - `freq`: frequency in Hz
///
/// Returns:
/// - 0 on success
/// - -1 if frequency is out of range
/// - -2 if setting requires stopping read first
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_center_freq(dev: *mut sddc_dev_t, freq: u32) -> c_int {
    sddc_set_center_freq64(dev, freq as u64)
}

/// Set the center frequency for the device (64-bit).
///
/// - `dev`: device handle
/// - `freq`: frequency in Hz
///
/// Returns:
/// - 0 on success
/// - -1 if frequency is out of range
/// - -2 if setting requires stopping read first
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_center_freq64(dev: *mut sddc_dev_t, freq: u64) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if device.set_center_freq(freq).is_err() {
            return -1;
        }
        0
    })
}

/// Get current sample rate in Hz.
///
/// Note: In raw mode, sample rate equals ADC XTAL frequency / 2.
///
/// - `dev`: device handle
///
/// Returns: sample rate in Hz, or 0 on error.
/// At SDR device level, sample rate equals xtal_freq.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_sample_rate(dev: *mut sddc_dev_t) -> u32 {
    if dev.is_null() {
        return 0;
    }
    unsafe {
        let device = sddc_dev_t::as_device_ref(dev);
        // At SDR device level: sample_rate = xtal_freq
        device.get_xtal_freq()
    }
}

/// Enable or disable direct sampling mode.
///
/// When enabled, ADC input is sent to the application without mixing
/// or filtering. Useful for wideband reception.
///
/// - `dev`: device handle
/// - `on`: 0 = disabled, 1 = enabled
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_direct_sampling(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if device.set_direct_sampling(on != 0).is_err() {
            return -1;
        }
        0
    })
}

/// Get state of direct sampling mode.
///
/// - `dev`: device handle
///
/// Returns: -1 on error, 0 = disabled, 1 = enabled.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_direct_sampling(dev: *mut sddc_dev_t) -> c_int {
    if dev.is_null() {
        return -1;
    }
    unsafe {
        let device = sddc_dev_t::as_device_ref(dev);
        if device.get_direct_sampling() { 1 } else { 0 }
    }
}

/// Read samples asynchronously; blocks until canceled via `sddc_cancel_async()`.
///
/// - `dev`: device handle
/// - `cb`: callback invoked with received samples
/// - `ctx`: user context passed to callback
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_read_async(
    dev: *mut sddc_dev_t,
    cb: sddc_read_async_cb_t,
    ctx: *mut c_void,
) -> c_int {
    if dev.is_null() || cb.is_none() {
        return -1;
    }

    // To satisfy Send/Sync, cast ctx to usize before moving into the closure.
    let ctx_val = ctx as usize;
    unsafe {
        let device = sddc_dev_t::as_device_mut(dev);
        device
            .as_mut()
            .read_async(Box::new(move |data: &[i16]| {
                let cb = cb.unwrap();
                let ctx_ptr = ctx_val as *mut c_void;
                // SAFETY: reinterpret i16 slice as u8 for C callback
                let ptr = data.as_ptr();
                let count = data.len() as u32;
                cb(ptr, count, ctx_ptr);
            }))
            .unwrap();
    }

    0
}

/// Cancel all pending asynchronous operations on the device.
///
/// - `dev`: device handle
///
/// Returns: 0 on success.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_cancel_async(dev: *mut sddc_dev_t) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if device.read_cancel().is_err() {
            return -1;
        }
        0
    })
}

/// Enable or disable the bias tee on GPIO PIN 0.
///
/// - `dev`: device handle
/// - `on`: 0 = off, 1 = HF on, 2 = VHF on, 3 = both on
///
/// Returns: -1 if device is not initialized, 0 otherwise.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_bias_tee(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        let result = device.enable_antenna_bias(0, on & 0x01 != 0).is_ok()
            && device.enable_antenna_bias(1, on & 0x02 != 0).is_ok();

        if result { 0 } else { -1 }
    })
}

/// Enable or disable ADC dither.
///
/// - `dev`: device handle
/// - `on`: 0 = off, 1 = on
///
/// Returns: -1 if device is not initialized, 0 otherwise.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_adc_dither(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if device.enable_adc_dither(on != 0).is_err() {
            return -1;
        }
        0
    })
}

/// Enable or disable ADC PGA
///
/// - `dev`: device handle
/// - `on`: 0 = off, 1 = on
///
/// Returns: -1 if device is not initialized, 0 otherwise.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_adc_pga(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if device.enable_adc_pga(on != 0).is_err() {
            return -1;
        }
        0
    })
}

/// Enable or disable ADC RANDO, only enable this before start reading
///
/// - `dev`: device handle
/// - `on`: 0 = off, 1 = on
///
/// Returns: -1 if device is not initialized or the device is busy, 0 otherwise.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_adc_rando(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if device.enable_adc_rando(on != 0).is_err() {
            return -1;
        }
        0
    })
}

/// Get firmware version in format 0xMMmm (MM=major, mm=minor).
///
/// - `dev`: device handle
///
/// Returns: version as 16-bit value.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_firmware_version(dev: *mut sddc_dev_t) -> u16 {
    if dev.is_null() {
        return 0;
    }
    unsafe {
        let device = sddc_dev_t::as_device_ref(dev);
        device.get_firmware_version()
    }
}

#[cfg(test)]
mod sddc_tests {
    use serial_test::serial;
    use std::ffi::CStr;

    use super::*;

    // Helper to create a test device
    fn create_test_device() -> *mut sddc_dev_t {
        let mut dev: *mut sddc_dev_t = std::ptr::null_mut();
        let ret = sddc_open(&mut dev, 0);
        assert_eq!(ret, 0, "Failed to open device");
        assert!(!dev.is_null(), "Device pointer is null");
        dev
    }

    // Helper to close a test device
    fn close_test_device(dev: *mut sddc_dev_t) {
        let ret = sddc_close(dev);
        assert_eq!(ret, 0, "Failed to close device");
    }

    #[test]
    fn test_get_device_count() {
        let count = sddc_get_device_count();
        assert!(count > 0, "Should detect at least one device");
    }

    #[test]
    fn test_get_device_name() {
        let name_ptr = sddc_get_device_name(0);
        assert!(!name_ptr.is_null(), "Device name should not be null");

        unsafe {
            let name = CStr::from_ptr(name_ptr).to_str().unwrap();
            assert!(!name.is_empty(), "Device name should not be empty");
        }
    }

    #[test]
    fn test_get_device_name_invalid_index() {
        let name_ptr = sddc_get_device_name(999);
        assert!(name_ptr.is_null(), "Invalid index should return null");
    }

    #[test]
    fn test_get_device_usb_strings() {
        let mut manufact = [0i8; 256];
        let mut product = [0i8; 256];
        let mut serial = [0i8; 256];

        let ret = sddc_get_device_usb_strings(
            0,
            manufact.as_mut_ptr(),
            product.as_mut_ptr(),
            serial.as_mut_ptr(),
        );
        assert_eq!(ret, 0, "Should succeed getting USB strings");
    }

    #[test]
    fn test_get_device_usb_strings_null_buffers() {
        let ret = sddc_get_device_usb_strings(
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert_eq!(ret, 0, "Should handle null buffers");
    }

    #[test]
    fn test_get_device_usb_strings_invalid_index() {
        let mut manufact = [0i8; 256];
        let mut product = [0i8; 256];
        let mut serial = [0i8; 256];

        let ret = sddc_get_device_usb_strings(
            999,
            manufact.as_mut_ptr(),
            product.as_mut_ptr(),
            serial.as_mut_ptr(),
        );
        assert_eq!(ret, -1, "Invalid index should return error");
    }

    #[test]
    fn test_get_index_by_serial_null() {
        let ret = sddc_get_index_by_serial(std::ptr::null());
        assert_eq!(ret, -1, "Null serial should return -1");
    }

    #[test]
    #[serial]
    fn test_open_close_device() {
        let mut dev: *mut sddc_dev_t = std::ptr::null_mut();
        let ret = sddc_open(&mut dev, 0);
        assert_eq!(ret, 0, "Should open device successfully");
        assert!(!dev.is_null(), "Device pointer should not be null");

        let ret = sddc_close(dev);
        assert_eq!(ret, 0, "Should close device successfully");
    }

    #[test]
    fn test_open_null_pointer() {
        let ret = sddc_open(std::ptr::null_mut(), 0);
        assert_eq!(ret, -1, "Opening with null pointer should fail");
    }

    #[test]
    fn test_close_null_device() {
        let ret = sddc_close(std::ptr::null_mut());
        assert_eq!(ret, -1, "Closing null device should fail");
    }

    #[test]
    #[serial]
    fn test_get_usb_strings_open_device() {
        let dev = create_test_device();

        let mut manufact = [0i8; 256];
        let mut product = [0i8; 256];
        let mut serial = [0i8; 256];

        let ret = sddc_get_usb_strings(
            dev,
            manufact.as_mut_ptr(),
            product.as_mut_ptr(),
            serial.as_mut_ptr(),
        );
        // Note: This returns empty strings as device_info is not accessible via trait
        assert_eq!(ret, 0);

        close_test_device(dev);
    }

    #[test]
    #[serial]
    fn test_xtal_freq() {
        let dev = create_test_device();

        // Set crystal frequency
        let ret = sddc_set_xtal_freq(dev, 64_000_000);
        assert_eq!(ret, 0, "Should set xtal freq successfully");

        // Get crystal frequency
        let mut freq: u32 = 0;
        let ret = sddc_get_xtal_freq(dev, &mut freq);
        assert_eq!(ret, 0, "Should get xtal freq successfully");
        assert_eq!(freq, 64_000_000, "Frequency should match");

        close_test_device(dev);
    }

    #[test]
    fn test_xtal_freq_null_device() {
        let ret = sddc_set_xtal_freq(std::ptr::null_mut(), 64_000_000);
        assert_eq!(ret, -1);

        let mut freq: u32 = 0;
        let ret = sddc_get_xtal_freq(std::ptr::null_mut(), &mut freq);
        assert_eq!(ret, -1);
    }

    #[test]
    #[serial]
    fn test_if_gain() {
        let dev = create_test_device();

        // Set IF gain
        let ret = sddc_set_if_gain(dev, 10.0);
        assert_eq!(ret, 0, "Should set IF gain successfully");

        // Get IF gain
        let mut gain: f32 = 0.0;
        let ret = sddc_get_if_gain(dev, &mut gain);
        assert_eq!(ret, 0, "Should get IF gain successfully");
        assert_eq!(gain, 10.0, "Gain should match");

        close_test_device(dev);
    }

    #[test]
    fn test_if_gain_null_device() {
        let ret = sddc_set_if_gain(std::ptr::null_mut(), 10.0);
        assert_eq!(ret, -1);

        let mut gain: f32 = 0.0;
        let ret = sddc_get_if_gain(std::ptr::null_mut(), &mut gain);
        assert_eq!(ret, -1);
    }

    #[test]
    #[serial]
    fn test_rf_gain() {
        let dev = create_test_device();

        // Set RF gain
        let ret = sddc_set_rf_gain(dev, 5.0);
        assert_eq!(ret, 0, "Should set RF gain successfully");

        // Get RF gain
        let mut gain: f32 = 0.0;
        let ret = sddc_get_rf_gain(dev, &mut gain);
        assert_eq!(ret, 0, "Should get RF gain successfully");
        assert_eq!(gain, 5.0, "Gain should match");

        close_test_device(dev);
    }

    #[test]
    fn test_rf_gain_null_device() {
        let ret = sddc_set_rf_gain(std::ptr::null_mut(), 5.0);
        assert_eq!(ret, -1);

        let mut gain: f32 = 0.0;
        let ret = sddc_get_rf_gain(std::ptr::null_mut(), &mut gain);
        assert_eq!(ret, -1);
    }

    #[test]
    #[serial]
    fn test_if_gain_range() {
        let dev = create_test_device();

        let mut min: f32 = 0.0;
        let mut max: f32 = 0.0;
        let ret = sddc_get_if_gain_range(dev, &mut min, &mut max);
        assert_eq!(ret, 0, "Should get IF gain range successfully");
        assert!(min <= max, "Min should be <= max");

        close_test_device(dev);
    }

    #[test]
    fn test_if_gain_range_null_pointers() {
        let mut min: f32 = 0.0;
        let mut max: f32 = 0.0;

        let ret = sddc_get_if_gain_range(std::ptr::null_mut(), &mut min, &mut max);
        assert_eq!(ret, -1);

        let dev = std::ptr::null_mut();
        let ret = sddc_get_if_gain_range(dev, std::ptr::null_mut(), &mut max);
        assert_eq!(ret, -1);

        let ret = sddc_get_if_gain_range(dev, &mut min, std::ptr::null_mut());
        assert_eq!(ret, -1);
    }

    #[test]
    #[serial]
    fn test_if_gain_steps() {
        let dev = create_test_device();

        let mut steps: *const f32 = std::ptr::null();
        let ret = sddc_get_if_gain_steps(dev, &mut steps);
        assert!(ret >= 0, "Should get IF gain steps successfully");

        if ret > 0 {
            assert!(!steps.is_null(), "Steps pointer should not be null");
        }

        close_test_device(dev);
    }

    #[test]
    fn test_if_gain_steps_null_device() {
        let mut steps: *const f32 = std::ptr::null();
        let ret = sddc_get_if_gain_steps(std::ptr::null_mut(), &mut steps);
        assert_eq!(ret, -1);
    }

    #[test]
    #[serial]
    fn test_rf_gain_range() {
        let dev = create_test_device();

        let mut min: f32 = 0.0;
        let mut max: f32 = 0.0;
        let ret = sddc_get_rf_gain_range(dev, &mut min, &mut max);
        assert_eq!(ret, 0, "Should get RF gain range successfully");
        assert!(min <= max, "Min should be <= max");

        close_test_device(dev);
    }

    #[test]
    #[serial]
    fn test_rf_gain_steps() {
        let dev = create_test_device();

        let mut steps: *const f32 = std::ptr::null();
        let ret = sddc_get_rf_gain_steps(dev, &mut steps);
        assert!(ret >= 0, "Should get RF gain steps successfully");

        if ret > 0 {
            assert!(!steps.is_null(), "Steps pointer should not be null");
        }

        close_test_device(dev);
    }

    #[test]
    #[serial]
    fn test_center_freq() {
        let dev = create_test_device();

        // Set center frequency
        let ret = sddc_set_center_freq(dev, 14_070_000);
        assert_eq!(ret, 0, "Should set center freq successfully");

        // Get center frequency
        let freq = sddc_get_center_freq(dev);
        assert_eq!(freq, 14_070_000, "Frequency should match");

        close_test_device(dev);
    }

    #[test]
    #[serial]
    fn test_center_freq64() {
        let dev = create_test_device();

        // Set center frequency (64-bit)
        let ret = sddc_set_center_freq64(dev, 14_070_000_u64);
        assert_eq!(ret, 0, "Should set center freq64 successfully");

        // Get center frequency (64-bit)
        let freq = sddc_get_center_freq64(dev);
        assert_eq!(freq, 14_070_000_u64, "Frequency should match");

        close_test_device(dev);
    }

    #[test]
    fn test_center_freq_null_device() {
        let ret = sddc_set_center_freq(std::ptr::null_mut(), 14_070_000);
        assert_eq!(ret, -1);

        let freq = sddc_get_center_freq(std::ptr::null_mut());
        assert_eq!(freq, 0);

        let ret = sddc_set_center_freq64(std::ptr::null_mut(), 14_070_000);
        assert_eq!(ret, -1);

        let freq = sddc_get_center_freq64(std::ptr::null_mut());
        assert_eq!(freq, 0);
    }

    #[test]
    #[serial]
    fn test_sample_rate() {
        let dev = create_test_device();

        // Set xtal freq to known value
        sddc_set_xtal_freq(dev, 64_000_000);

        let rate = sddc_get_sample_rate(dev);
        assert_eq!(
            rate, 64_000_000,
            "Sample rate should equal xtal_freq at SDR device level"
        );

        close_test_device(dev);
    }

    #[test]
    fn test_sample_rate_null_device() {
        let rate = sddc_get_sample_rate(std::ptr::null_mut());
        assert_eq!(rate, 0);
    }

    #[test]
    #[serial]
    fn test_direct_sampling() {
        let dev = create_test_device();

        // Enable direct sampling
        let ret = sddc_set_direct_sampling(dev, 1);
        assert_eq!(ret, 0, "Should set direct sampling successfully");

        // Get direct sampling state
        let state = sddc_get_direct_sampling(dev);
        assert_eq!(state, 1, "Direct sampling should be enabled");

        // Disable direct sampling
        let ret = sddc_set_direct_sampling(dev, 0);
        assert_eq!(ret, 0, "Should disable direct sampling successfully");

        let state = sddc_get_direct_sampling(dev);
        assert_eq!(state, 0, "Direct sampling should be disabled");

        close_test_device(dev);
    }

    #[test]
    fn test_direct_sampling_null_device() {
        let ret = sddc_set_direct_sampling(std::ptr::null_mut(), 1);
        assert_eq!(ret, -1);

        let state = sddc_get_direct_sampling(std::ptr::null_mut());
        assert_eq!(state, -1);
    }

    #[test]
    #[serial]
    fn test_cancel_async() {
        let dev = create_test_device();

        // Cancel should succeed even if not reading
        let ret = sddc_cancel_async(dev);
        assert_eq!(ret, 0, "Should cancel async successfully");

        close_test_device(dev);
    }

    #[test]
    fn test_cancel_async_null_device() {
        let ret = sddc_cancel_async(std::ptr::null_mut());
        assert_eq!(ret, -1);
    }

    #[test]
    #[serial]
    fn test_bias_tee() {
        let dev = create_test_device();

        // Test different bias tee modes
        let ret = sddc_enable_bias_tee(dev, 0);
        assert_eq!(ret, 0, "Should disable bias tee");

        let ret = sddc_enable_bias_tee(dev, 1);
        assert_eq!(ret, 0, "Should enable HF bias tee");

        let ret = sddc_enable_bias_tee(dev, 2);
        assert_eq!(ret, 0, "Should enable VHF bias tee");

        let ret = sddc_enable_bias_tee(dev, 3);
        assert_eq!(ret, 0, "Should enable both bias tees");

        close_test_device(dev);
    }

    #[test]
    fn test_bias_tee_null_device() {
        let ret = sddc_enable_bias_tee(std::ptr::null_mut(), 1);
        assert_eq!(ret, -1);
    }

    #[test]
    #[serial]
    fn test_adc_dither() {
        let dev = create_test_device();

        let ret = sddc_enable_adc_dither(dev, 1);
        assert_eq!(ret, 0, "Should enable ADC dither");

        let ret = sddc_enable_adc_dither(dev, 0);
        assert_eq!(ret, 0, "Should disable ADC dither");

        close_test_device(dev);
    }

    #[test]
    fn test_adc_dither_null_device() {
        let ret = sddc_enable_adc_dither(std::ptr::null_mut(), 1);
        assert_eq!(ret, -1);
    }

    #[test]
    #[serial]
    fn test_adc_pga() {
        let dev = create_test_device();

        let ret = sddc_enable_adc_pga(dev, 1);
        assert_eq!(ret, 0, "Should enable ADC PGA");

        let ret = sddc_enable_adc_pga(dev, 0);
        assert_eq!(ret, 0, "Should disable ADC PGA");

        close_test_device(dev);
    }

    #[test]
    fn test_adc_pga_null_device() {
        let ret = sddc_enable_adc_pga(std::ptr::null_mut(), 1);
        assert_eq!(ret, -1);
    }

    #[test]
    #[serial]
    fn test_firmware_version() {
        let dev = create_test_device();

        let version = sddc_get_firmware_version(dev);
        // Version should be non-zero for a valid device
        assert!(version > 0, "Firmware version should be non-zero");

        close_test_device(dev);
    }

    #[test]
    fn test_firmware_version_null_device() {
        let version = sddc_get_firmware_version(std::ptr::null_mut());
        assert_eq!(version, 0);
    }

    #[test]
    #[serial]
    fn test_read_async_null_callback() {
        let dev = create_test_device();

        let ret = sddc_read_async(dev, None, std::ptr::null_mut());
        assert_eq!(ret, -1, "Should fail with null callback");

        close_test_device(dev);
    }

    #[test]
    fn test_read_async_null_device() {
        extern "C" fn dummy_callback(_buf: *const i16, _count: u32, _ctx: *mut c_void) {}

        let ret = sddc_read_async(
            std::ptr::null_mut(),
            Some(dummy_callback),
            std::ptr::null_mut(),
        );
        assert_eq!(ret, -1);
    }
}
