use core::ptr;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

use crate::sdr::{Radio, SdrError};

/// Opaque handle to an open RX888-family SDR device.
///
/// Obtain a handle via `sddc_open()` and release it with `sddc_close()`.
/// All other API functions require this handle as their first argument.
/// The handle must not be shared across threads without external synchronization.
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
            return SDDC_ERROR;
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
            return SDDC_ERROR;
        }
        unsafe {
            let device = sddc_dev_t::as_device_ref($dev);
            $body(device)
        }
    }};
}

/// Map a typed `SdrError` to a C integer error code.
///
/// Error code table:
/// -  0  SDDC_SUCCESS           – operation succeeded (not an error)
/// - -1  SDDC_ERROR             – null device handle or unspecified error
/// - -2  SDDC_ERROR_BUSY        – setting cannot be changed while device is streaming
/// - -3  SDDC_ERROR_INVALID_PARAM – parameter value out of allowed range
/// - -4  SDDC_ERROR_IO          – USB transfer or register communication failure
/// - -5  SDDC_ERROR_NO_DEVICE   – no device found at the requested index
/// - -6  SDDC_ERROR_USB_SPEED   – device is not connected at SuperSpeed (USB 3.0)
/// - -7  SDDC_ERROR_FIRMWARE    – installed firmware version does not match
/// - -8  SDDC_ERROR_OPEN        – could not open or claim the USB device
fn sdr_error_to_c_int(e: SdrError) -> c_int {
    match e {
        SdrError::DeviceNotFound => SDDC_ERROR_NO_DEVICE,
        SdrError::NotSuperSpeed => SDDC_ERROR_USB_SPEED,
        SdrError::DeviceOpenFailed(_) => SDDC_ERROR_OPEN,
        SdrError::FirmwareVersionMismatch { .. } => SDDC_ERROR_FIRMWARE,
        SdrError::CommunicationError(_) => SDDC_ERROR_IO,
        SdrError::DeviceRunning => SDDC_ERROR_BUSY,
        SdrError::AlreadyRunning => SDDC_ERROR_BUSY,
        SdrError::InvalidParameter(_) => SDDC_ERROR_INVALID_PARAM,
    }
}

/// Operation succeeded.
#[allow(dead_code)]
pub const SDDC_SUCCESS: c_int = 0;
/// Null device handle or generic unspecified error.
pub const SDDC_ERROR: c_int = -1;
/// Setting cannot be changed while the device is streaming; stop with `sddc_cancel_async()` first.
pub const SDDC_ERROR_BUSY: c_int = -2;
/// A parameter value is out of the allowed range.
pub const SDDC_ERROR_INVALID_PARAM: c_int = -3;
/// USB transfer or firmware register communication failure.
pub const SDDC_ERROR_IO: c_int = -4;
/// No RX888-family device found at the requested index.
pub const SDDC_ERROR_NO_DEVICE: c_int = -5;
/// Device is present but not connected at SuperSpeed (USB 3.0).
pub const SDDC_ERROR_USB_SPEED: c_int = -6;
/// Firmware version on the device does not match the required version.
pub const SDDC_ERROR_FIRMWARE: c_int = -7;
/// USB device or interface could not be opened or claimed by the OS.
pub const SDDC_ERROR_OPEN: c_int = -8;

/// Callback function type for `sddc_read_async()`.
///
/// - `buf`: pointer to received samples as signed 16-bit integers (I only in direct-sampling mode)
/// - `count`: number of samples in `buf`; 0 indicates a streaming error or end of stream
/// - `ctx`: user context pointer passed to `sddc_read_async()`
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

    count
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
            return SDDC_ERROR;
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

    SDDC_SUCCESS
}

/// Get device index by USB serial string descriptor.
///
/// - `serial`: serial string of the device
///
/// Returns:
/// - device index of first matching device
/// - SDDC_ERROR_INVALID_PARAM if `serial` is NULL
/// - SDDC_ERROR_NO_DEVICE if no devices were found
/// - SDDC_ERROR if devices were found, but none matched
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_index_by_serial(serial: *const c_char) -> c_int {
    if serial.is_null() {
        return SDDC_ERROR_INVALID_PARAM;
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(serial) };
    let serial_str = match cstr.to_str() {
        Ok(s) => s,
        Err(_) => return SDDC_ERROR_INVALID_PARAM,
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

    if !found_any {
        SDDC_ERROR_NO_DEVICE
    } else {
        SDDC_ERROR
    }
}

/// Open the device.
///
/// - `dev`: output pointer that will receive the device handle on success
/// - `index`: zero-based device index (use `sddc_get_device_count()` to enumerate)
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)           on success
/// - -1 (`SDDC_ERROR`)             if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)          on USB communication failure
/// - -5 (`SDDC_ERROR_NO_DEVICE`)   if no device is present at `index`
/// - -6 (`SDDC_ERROR_USB_SPEED`)   if device is not connected at SuperSpeed (USB 3.0)
/// - -7 (`SDDC_ERROR_FIRMWARE`)    if the firmware version does not match
/// - -8 (`SDDC_ERROR_OPEN`)        if the OS denied access to the USB device
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_open(dev: *mut *mut sddc_dev_t, index: u32) -> c_int {
    if dev.is_null() {
        return SDDC_ERROR;
    }

    if let Some(device) = Radio::find_device(index) {
        match Radio::new(device) {
            Ok(radio) => {
                let boxed: Box<Radio> = Box::new(radio);
                unsafe { *dev = Box::into_raw(Box::new(boxed)) as *mut sddc_dev_t };
                0
            }
            Err(e) => sdr_error_to_c_int(e),
        }
    } else {
        sdr_error_to_c_int(SdrError::DeviceNotFound)
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
        return SDDC_ERROR;
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
    with_device_ref!(dev, |device: &Radio| {
        let info = device.device_info();

        if let Some(m) = info.manufacturer_string() {
            write_cstr_limited(manufact, m, 256);
        } else {
            write_empty_cstr(manufact);
        }
        if let Some(p) = info.product_string() {
            write_cstr_limited(product, p, 256);
        } else {
            write_empty_cstr(product);
        }
        if let Some(s) = info.serial_number() {
            write_cstr_limited(serial, s, 256);
        } else {
            write_empty_cstr(serial);
        }
    });

    0
}

/// Set ADC crystal oscillator frequency.
/// At the SDR device level, xtal_freq equals the sample rate.
///
/// Default is 64 MHz for most models (61.44 MHz for RX888 PRO). Changing
/// this value affects the usable bandwidth and frequency range in direct
/// sampling mode. Must be called before `sddc_read_async()`; the setting
/// cannot be changed while streaming.
///
/// - `dev`: device handle
/// - `rtl_freq`: ADC clock frequency in Hz (equals the ADC sample rate)
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)     on success
/// - -1 (`SDDC_ERROR`)       if `dev` is NULL
/// - -2 (`SDDC_ERROR_BUSY`)  if device is currently streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_xtal_freq(dev: *mut sddc_dev_t, rtl_freq: u32) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.set_xtal_freq(rtl_freq) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Get ADC crystal oscillator frequency.
///
/// - `dev`: device handle
/// - `rtl_freq`: output pointer, receives the ADC clock frequency in Hz
///
/// Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if `dev` or `rtl_freq` is NULL.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_xtal_freq(dev: *mut sddc_dev_t, rtl_freq: *mut u32) -> c_int {
    if rtl_freq.is_null() {
        return SDDC_ERROR;
    }
    with_device_ref!(dev, |device: &Radio| {
        *rtl_freq = device.get_xtal_freq();
        0
    })
}

/// Set the IF (intermediate frequency) gain.
///
/// Value is in dB; valid range depends on device model and sampling mode.
/// Use `sddc_get_if_gain_range()` and `sddc_get_if_gain_steps()` to query
/// the allowed values. Applied immediately when streaming.
///
/// - `dev`: device handle
/// - `value`: gain in dB
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)    on success
/// - -1 (`SDDC_ERROR`)      if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)   on USB communication failure when streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_if_gain(dev: *mut sddc_dev_t, value: f32) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.set_if_gain(value) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Set the RF gain.
///
/// Value is in dB; valid range depends on device model and sampling mode.
/// Use `sddc_get_rf_gain_range()` and `sddc_get_rf_gain_steps()` to query
/// the allowed values. Applied immediately when streaming.
///
/// - `dev`: device handle
/// - `value`: gain in dB
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)    on success
/// - -1 (`SDDC_ERROR`)      if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)   on USB communication failure when streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_rf_gain(dev: *mut sddc_dev_t, value: f32) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.set_rf_gain(value) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Get the current IF gain in dB.
///
/// - `dev`: device handle
/// - `value`: output pointer, receives the IF gain in dB
///
/// Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if `dev` or `value` is NULL.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_if_gain(dev: *mut sddc_dev_t, value: *mut f32) -> c_int {
    if value.is_null() {
        return SDDC_ERROR;
    }
    with_device_ref!(dev, |device: &Radio| {
        *value = device.get_if_gain();
        0
    })
}

/// Get the IF gain range supported by this device and sampling mode.
///
/// - `dev`: device handle
/// - `min`: output pointer, receives the minimum IF gain in dB
/// - `max`: output pointer, receives the maximum IF gain in dB
///
/// Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if any pointer is NULL.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_if_gain_range(
    _dev: *mut sddc_dev_t,
    min: *mut f32,
    max: *mut f32,
) -> c_int {
    if min.is_null() || max.is_null() {
        return SDDC_ERROR;
    }
    with_device_ref!(_dev, |device: &Radio| {
        let (mn, mx) = device.get_if_gain_range();
        *min = mn;
        *max = mx;
        0
    })
}

/// Get the discrete IF gain steps supported by this device and sampling mode.
///
/// The returned pointer points to a static array owned by the library; do not free it.
///
/// - `dev`: device handle
/// - `steps`: output pointer, receives a pointer to an array of gain values in dB
///
/// Returns: number of entries in the steps array on success, -1 (`SDDC_ERROR`) if any pointer is NULL.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_if_gain_steps(_dev: *mut sddc_dev_t, steps: *mut *const f32) -> c_int {
    if steps.is_null() {
        return SDDC_ERROR;
    }
    with_device_ref!(_dev, |device: &Radio| {
        let slice = device.get_if_gain_steps();
        *steps = slice.as_ptr();
        slice.len() as c_int
    })
}

/// Get the current RF gain in dB.
///
/// - `dev`: device handle
/// - `value`: output pointer, receives the RF gain in dB
///
/// Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if `dev` or `value` is NULL.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_rf_gain(dev: *mut sddc_dev_t, value: *mut f32) -> c_int {
    if value.is_null() {
        return SDDC_ERROR;
    }
    with_device_ref!(dev, |device: &Radio| {
        *value = device.get_rf_gain();
        0
    })
}

/// Get the RF gain range supported by this device and sampling mode.
///
/// - `dev`: device handle
/// - `min`: output pointer, receives the minimum RF gain in dB
/// - `max`: output pointer, receives the maximum RF gain in dB
///
/// Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if any pointer is NULL.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_rf_gain_range(
    _dev: *mut sddc_dev_t,
    min: *mut f32,
    max: *mut f32,
) -> c_int {
    if min.is_null() || max.is_null() {
        return SDDC_ERROR;
    }
    with_device_ref!(_dev, |device: &Radio| {
        let (mn, mx) = device.get_rf_gain_range();
        *min = mn;
        *max = mx;
        0
    })
}

/// Get the discrete RF gain steps supported by this device and sampling mode.
///
/// The returned pointer points to a static array owned by the library; do not free it.
///
/// - `dev`: device handle
/// - `steps`: output pointer, receives a pointer to an array of gain values in dB
///
/// Returns: number of entries in the steps array on success, -1 (`SDDC_ERROR`) if any pointer is NULL.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_rf_gain_steps(_dev: *mut sddc_dev_t, steps: *mut *const f32) -> c_int {
    if steps.is_null() {
        return SDDC_ERROR;
    }
    with_device_ref!(_dev, |device: &Radio| {
        let slice = device.get_rf_gain_steps();
        *steps = slice.as_ptr();
        slice.len() as c_int
    })
}

/// Get the current center frequency in Hz (64-bit).
///
/// - `dev`: device handle
///
/// Returns: center frequency in Hz, or 0 if `dev` is NULL.
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

/// Set the center frequency for the device (64-bit).
///
/// May be called while streaming; the new frequency is applied immediately.
/// In direct-sampling mode this parameter is informational only and does not
/// affect hardware — the full ADC bandwidth is always captured.
///
/// - `dev`: device handle
/// - `freq`: center frequency in Hz
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)   on success
/// - -1 (`SDDC_ERROR`)     if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_center_freq64(dev: *mut sddc_dev_t, freq: u64) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.set_center_freq(freq) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Get current ADC sample rate in Hz.
///
/// Equals the crystal oscillator frequency set via `sddc_set_xtal_freq()`.
///
/// - `dev`: device handle
///
/// Returns: sample rate in Hz, or 0 if `dev` is NULL.
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
/// In direct-sampling mode the HF input is routed straight to the ADC,
/// giving wideband coverage from DC to the Nyquist frequency. In tuner
/// mode a downstream mixer/tuner covers VHF/UHF bands.
/// This setting cannot be changed while streaming.
///
/// - `dev`: device handle
/// - `on`: 1 = direct sampling (HF), 0 = tuner path (VHF/UHF)
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)     on success
/// - -1 (`SDDC_ERROR`)       if `dev` is NULL
/// - -2 (`SDDC_ERROR_BUSY`)  if device is currently streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_direct_sampling(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.set_direct_sampling(on != 0) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Get state of direct sampling mode.
///
/// - `dev`: device handle
///
/// Returns: 1 if direct sampling is enabled, 0 if disabled, -1 (`SDDC_ERROR`) if `dev` is NULL.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_get_direct_sampling(dev: *mut sddc_dev_t) -> c_int {
    if dev.is_null() {
        return SDDC_ERROR;
    }
    unsafe {
        let device = sddc_dev_t::as_device_ref(dev);
        if device.get_direct_sampling() { 1 } else { 0 }
    }
}

/// Start asynchronous sample streaming.
///
/// Configures and starts the ADC, then blocks in the calling thread,
/// invoking `cb` repeatedly with sample buffers until `sddc_cancel_async()`
/// is called from another thread. When the callback receives `count == 0`,
/// a streaming error has occurred.
///
/// All configuration (gain, frequency, crystal frequency, direct sampling)
/// must be set before calling this function.
///
/// - `dev`: device handle
/// - `cb`: callback function invoked with each batch of samples
/// - `ctx`: user context pointer forwarded unchanged to every `cb` invocation
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)        on success (after streaming ends)
/// - -1 (`SDDC_ERROR`)          if `dev` or `cb` is NULL
/// - -2 (`SDDC_ERROR_BUSY`)     if streaming is already in progress
/// - -4 (`SDDC_ERROR_IO`)       on USB communication failure during setup
/// - -6 (`SDDC_ERROR_USB_SPEED`) if device is not at SuperSpeed
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_read_async(
    dev: *mut sddc_dev_t,
    cb: sddc_read_async_cb_t,
    ctx: *mut c_void,
) -> c_int {
    if dev.is_null() || cb.is_none() {
        return SDDC_ERROR;
    }

    // To satisfy Send/Sync, cast ctx to usize before moving into the closure.
    let ctx_val = ctx as usize;
    unsafe {
        let device = sddc_dev_t::as_device_mut(dev);
        device
            .as_mut()
            .read_async(Box::new(move |data: Option<&[i16]>| {
                let cb = cb.unwrap();
                let ctx_ptr = ctx_val as *mut c_void;
                // SAFETY: reinterpret i16 slice as u8 for C callback
                let (ptr, count) = match data {
                    Some(slice) => (slice.as_ptr(), slice.len() as u32),
                    None => (std::ptr::null(), 0),
                };
                cb(ptr, count, ctx_ptr);
            }))
            .unwrap();
    }

    0
}

/// Stop asynchronous streaming started by `sddc_read_async()`.
///
/// Signals the streaming thread to stop, joins it, then powers down the ADC.
/// Safe to call even if no streaming is in progress.
///
/// - `dev`: device handle
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)   on success
/// - -1 (`SDDC_ERROR`)     if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)  on USB communication failure during ADC shutdown
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_cancel_async(dev: *mut sddc_dev_t) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.read_cancel() {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Enable or disable the antenna bias-tee voltage.
///
/// The bias-tee supplies DC power to an active antenna or low-noise
/// amplifier through the coax cable.
///
/// - `dev`: device handle
/// - `on`: bitmask — bit 0 = HF port bias, bit 1 = VHF/UHF port bias
///   (0 = both off, 1 = HF on, 2 = VHF on, 3 = both on)
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)   on success
/// - -1 (`SDDC_ERROR`)     if `dev` is NULL
/// - -3 (`SDDC_ERROR_INVALID_PARAM`) if an index is out of range
/// - -4 (`SDDC_ERROR_IO`)  on USB communication failure
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_bias_tee(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        if let Err(e) = device.enable_antenna_bias(0, on & 0x01 != 0) {
            return sdr_error_to_c_int(e);
        }
        if let Err(e) = device.enable_antenna_bias(1, on & 0x02 != 0) {
            return sdr_error_to_c_int(e);
        }
        0
    })
}

/// Enable or disable ADC dither.
///
/// Dither adds a small, shaped noise signal to the ADC input to reduce
/// harmonic spurs at the cost of a slightly elevated noise floor.
/// May be toggled while streaming; applied immediately.
///
/// - `dev`: device handle
/// - `on`: 1 = enable dither, 0 = disable dither
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)   on success
/// - -1 (`SDDC_ERROR`)     if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_adc_dither(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.enable_adc_dither(on != 0) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Enable or disable the ADC programmable gain amplifier (PGA).
///
/// The PGA increases ADC input sensitivity. May be toggled while streaming;
/// applied immediately.
///
/// - `dev`: device handle
/// - `on`: 1 = enable PGA, 0 = disable PGA
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)   on success
/// - -1 (`SDDC_ERROR`)     if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_adc_pga(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.enable_adc_pga(on != 0) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Enable or disable ADC output bit randomization (de-randomization applied on host).
///
/// When enabled, the ADC XORs each sample with a known pattern to reduce
/// spectral leakage from the digital logic. The host driver automatically
/// reverses the randomization before delivering samples to the callback.
/// Must be set before calling `sddc_read_async()`; cannot be changed while streaming.
///
/// - `dev`: device handle
/// - `on`: 1 = enable randomization, 0 = disable
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)     on success
/// - -1 (`SDDC_ERROR`)       if `dev` is NULL
/// - -2 (`SDDC_ERROR_BUSY`)  if device is currently streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_adc_rando(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.enable_adc_rando(on != 0) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Get the installed firmware version as a packed 16-bit value.
///
/// Format: `0xMMmm` where `MM` is the major version and `mm` is the minor version.
/// The current required version is defined at build time and enforced by `sddc_open()`.
///
/// - `dev`: device handle
///
/// Returns: packed firmware version `(major << 8) | minor`, or 0 if `dev` is NULL.
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

/// Enable or disable HF input high-impedance mode.
///
/// In high-Z mode the HF input is switched to a high-impedance termination
/// for use with antennas that include their own preamplifier. In low-Z mode
/// (default) the input is 50 Ω matched. May be toggled while streaming;
/// applied immediately.
///
/// - `dev`: device handle
/// - `on`: 1 = high-Z input, 0 = 50 Ω input
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)   on success
/// - -1 (`SDDC_ERROR`)     if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_hf_highz(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.enable_hf_highz(on != 0) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Enable or disable external clock input (RX888 PRO only).
///
/// When enabled, the ADC clock is derived from a signal applied to the
/// external clock input rather than the on-board crystal oscillator.
/// Use this to phase-lock multiple units or improve long-term frequency
/// accuracy with an external reference. May be changed while streaming;
/// applied immediately.
///
/// - `dev`: device handle
/// - `on`: 1 = use external clock, 0 = use internal crystal
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)   on success
/// - -1 (`SDDC_ERROR`)     if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_ext_clock(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.enable_ext_clock(on != 0) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// Set the ADC anti-aliasing filter mode (RX888 PRO only).
///
/// Selects between the on-board LPF options or bypass mode.
/// May be changed while streaming; applied immediately.
///
/// - `dev`: device handle
/// - `mode`: one of `Freq64MHz`, `Freq32MHz`, `FMUndersample`, or `Bypass`
///
/// Returns:
/// -  0 (`SDDC_SUCCESS`)   on success
/// - -1 (`SDDC_ERROR`)     if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_adc_filter(dev: *mut sddc_dev_t, mode: crate::sdr::FilterMode) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.set_adc_filter(mode) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

/// set externion IO port state, only for RX888 PRO
/// - `dev`: device handle
/// - `state`: state to set (low 7 bits represent the state of 7 pins, high bit is reserved)
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_set_ext_io_port_state(dev: *mut sddc_dev_t, state: u8) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.set_ext_gpio(state) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
}

// Enable or disable the ADC front-end preamplifier (RX888 PRO only).
///
/// The preamp boosts the ADC input sensitivity by about 20 dB at the cost of
/// a slightly elevated noise floor. May be toggled while streaming; applied immediately.
///
/// - `dev`: device handle
/// - `on`: 1 = enable preamp, 0 = disable preamp
///   Returns:
/// -  0 (`SDDC_SUCCESS`)   on success
/// - -1 (`SDDC_ERROR`)     if `dev` is NULL
/// - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn sddc_enable_preamp(dev: *mut sddc_dev_t, on: c_int) -> c_int {
    with_device!(dev, |device: &mut Radio| {
        match device.enable_preamp(on != 0) {
            Ok(_) => 0,
            Err(e) => sdr_error_to_c_int(e),
        }
    })
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
        let mut manufact = [0; 256];
        let mut product = [0; 256];
        let mut serial = [0; 256];

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
        let mut manufact = [0; 256];
        let mut product = [0; 256];
        let mut serial = [0; 256];

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
        assert_eq!(
            ret, SDDC_ERROR_INVALID_PARAM,
            "Null serial should return SDDC_ERROR_INVALID_PARAM"
        );
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

        let mut manufact = [0; 256];
        let mut product = [0; 256];
        let mut serial = [0; 256];

        let ret = sddc_get_usb_strings(
            dev,
            manufact.as_mut_ptr(),
            product.as_mut_ptr(),
            serial.as_mut_ptr(),
        );

        assert_eq!(ret, 0);
        // assert!(manufact[0] != 0, "Manufacturer string should not be empty");
        assert!(product[0] != 0, "Product string should not be empty");
        assert!(serial[0] != 0, "Serial string should not be empty");

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
