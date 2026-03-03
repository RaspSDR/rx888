#ifndef LIBSDDC_H
#define LIBSDDC_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Operation succeeded.
 */
#define SDDC_SUCCESS 0

/**
 * Null device handle or generic unspecified error.
 */
#define SDDC_ERROR -1

/**
 * Setting cannot be changed while the device is streaming; stop with `sddc_cancel_async()` first.
 */
#define SDDC_ERROR_BUSY -2

/**
 * A parameter value is out of the allowed range.
 */
#define SDDC_ERROR_INVALID_PARAM -3

/**
 * USB transfer or firmware register communication failure.
 */
#define SDDC_ERROR_IO -4

/**
 * No RX888-family device found at the requested index.
 */
#define SDDC_ERROR_NO_DEVICE -5

/**
 * Device is present but not connected at SuperSpeed (USB 3.0).
 */
#define SDDC_ERROR_USB_SPEED -6

/**
 * Firmware version on the device does not match the required version.
 */
#define SDDC_ERROR_FIRMWARE -7

/**
 * USB device or interface could not be opened or claimed by the OS.
 */
#define SDDC_ERROR_OPEN -8

/**
 * ADC filter for RX888 PRO
 */
typedef enum FilterMode {
  /**
   * 64Mhz LPF Filter
   */
  Freq64MHz = 0,
  /**
   * 32Mhz LPF Filter
   */
  Freq32MHz = 1,
  /**
   * BPF Filter for FM Undersampling
   */
  FMUndersample = 2,
  /**
   * Bypass mode: anti-aliasing must be handled by the input signal
   */
  Bypass = 3,
} FilterMode;

/**
 * Opaque handle to an open RX888-family SDR device.
 *
 * Obtain a handle via `sddc_open()` and release it with `sddc_close()`.
 * All other API functions require this handle as their first argument.
 * The handle must not be shared across threads without external synchronization.
 */
struct sddc_dev_t;


/**
 * Callback function type for `sddc_read_async()`.
 *
 * - `buf`: pointer to received samples as signed 16-bit integers (I only in direct-sampling mode)
 * - `count`: number of samples in `buf`; 0 indicates a streaming error or end of stream
 * - `ctx`: user context pointer passed to `sddc_read_async()`
 */
typedef void (*sddc_read_async_cb_t)(const int16_t *buf, uint32_t count, void *ctx);

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Get the number of available SDR devices.
 *
 * Returns: number of devices detected.
 */
uint32_t sddc_get_device_count(void);

/**
 * Get device name by index.
 *
 * - `index`: device index
 *
 * Returns: pointer to a null-terminated C string or NULL on error.
 */
const char *sddc_get_device_name(uint32_t index);

/**
 * Get USB device strings.
 *
 * NOTE: Each string buffer must provide space for up to 256 bytes.
 *
 * - `index`: device index
 * - `manufact`: manufacturer name buffer, may be NULL
 * - `product`: product name buffer, may be NULL
 * - `serial`: serial number buffer, may be NULL
 *
 * Returns: 0 on success.
 */
int sddc_get_device_usb_strings(uint32_t index, char *manufact, char *product, char *serial);

/**
 * Get device index by USB serial string descriptor.
 *
 * - `serial`: serial string of the device
 *
 * Returns:
 * - device index of first matching device
 * - SDDC_ERROR_INVALID_PARAM if `serial` is NULL
 * - SDDC_ERROR_NO_DEVICE if no devices were found
 * - SDDC_ERROR if devices were found, but none matched
 */
int sddc_get_index_by_serial(const char *serial);

/**
 * Open the device.
 *
 * - `dev`: output pointer that will receive the device handle on success
 * - `index`: zero-based device index (use `sddc_get_device_count()` to enumerate)
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)           on success
 * - -1 (`SDDC_ERROR`)             if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)          on USB communication failure
 * - -5 (`SDDC_ERROR_NO_DEVICE`)   if no device is present at `index`
 * - -6 (`SDDC_ERROR_USB_SPEED`)   if device is not connected at SuperSpeed (USB 3.0)
 * - -7 (`SDDC_ERROR_FIRMWARE`)    if the firmware version does not match
 * - -8 (`SDDC_ERROR_OPEN`)        if the OS denied access to the USB device
 */
int sddc_open(struct sddc_dev_t **dev, uint32_t index);

/**
 * Close the device opened by `sddc_open()`.
 *
 * - `dev`: device handle
 *
 * Returns: 0 on success.
 */
int sddc_close(struct sddc_dev_t *dev);

/**
 * Get USB device strings for an open device.
 *
 * NOTE: Each string buffer must provide space for up to 256 bytes.
 *
 * - `dev`: device handle
 * - `manufact`: manufacturer name buffer, may be NULL
 * - `product`: product name buffer, may be NULL
 * - `serial`: serial number buffer, may be NULL
 *
 * Returns: 0 on success.
 */
int sddc_get_usb_strings(struct sddc_dev_t *dev, char *manufact, char *product, char *serial);

/**
 * Set ADC crystal oscillator frequency.
 * At the SDR device level, xtal_freq equals the sample rate.
 *
 * Default is 64 MHz for most models (61.44 MHz for RX888 PRO). Changing
 * this value affects the usable bandwidth and frequency range in direct
 * sampling mode. Must be called before `sddc_read_async()`; the setting
 * cannot be changed while streaming.
 *
 * - `dev`: device handle
 * - `rtl_freq`: ADC clock frequency in Hz (equals the ADC sample rate)
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)     on success
 * - -1 (`SDDC_ERROR`)       if `dev` is NULL
 * - -2 (`SDDC_ERROR_BUSY`)  if device is currently streaming
 */
int sddc_set_xtal_freq(struct sddc_dev_t *dev, uint32_t rtl_freq);

/**
 * Get ADC crystal oscillator frequency.
 *
 * - `dev`: device handle
 * - `rtl_freq`: output pointer, receives the ADC clock frequency in Hz
 *
 * Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if `dev` or `rtl_freq` is NULL.
 */
int sddc_get_xtal_freq(struct sddc_dev_t *dev, uint32_t *rtl_freq);

/**
 * Set the IF (intermediate frequency) gain.
 *
 * Value is in dB; valid range depends on device model and sampling mode.
 * Use `sddc_get_if_gain_range()` and `sddc_get_if_gain_steps()` to query
 * the allowed values. Applied immediately when streaming.
 *
 * - `dev`: device handle
 * - `value`: gain in dB
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)    on success
 * - -1 (`SDDC_ERROR`)      if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)   on USB communication failure when streaming
 */
int sddc_set_if_gain(struct sddc_dev_t *dev, float value);

/**
 * Set the RF gain.
 *
 * Value is in dB; valid range depends on device model and sampling mode.
 * Use `sddc_get_rf_gain_range()` and `sddc_get_rf_gain_steps()` to query
 * the allowed values. Applied immediately when streaming.
 *
 * - `dev`: device handle
 * - `value`: gain in dB
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)    on success
 * - -1 (`SDDC_ERROR`)      if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)   on USB communication failure when streaming
 */
int sddc_set_rf_gain(struct sddc_dev_t *dev, float value);

/**
 * Get the current IF gain in dB.
 *
 * - `dev`: device handle
 * - `value`: output pointer, receives the IF gain in dB
 *
 * Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if `dev` or `value` is NULL.
 */
int sddc_get_if_gain(struct sddc_dev_t *dev, float *value);

/**
 * Get the IF gain range supported by this device and sampling mode.
 *
 * - `dev`: device handle
 * - `min`: output pointer, receives the minimum IF gain in dB
 * - `max`: output pointer, receives the maximum IF gain in dB
 *
 * Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if any pointer is NULL.
 */
int sddc_get_if_gain_range(struct sddc_dev_t *_dev, float *min, float *max);

/**
 * Get the discrete IF gain steps supported by this device and sampling mode.
 *
 * The returned pointer points to a static array owned by the library; do not free it.
 *
 * - `dev`: device handle
 * - `steps`: output pointer, receives a pointer to an array of gain values in dB
 *
 * Returns: number of entries in the steps array on success, -1 (`SDDC_ERROR`) if any pointer is NULL.
 */
int sddc_get_if_gain_steps(struct sddc_dev_t *_dev,
                           const float **steps);

/**
 * Get the current RF gain in dB.
 *
 * - `dev`: device handle
 * - `value`: output pointer, receives the RF gain in dB
 *
 * Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if `dev` or `value` is NULL.
 */
int sddc_get_rf_gain(struct sddc_dev_t *dev, float *value);

/**
 * Get the RF gain range supported by this device and sampling mode.
 *
 * - `dev`: device handle
 * - `min`: output pointer, receives the minimum RF gain in dB
 * - `max`: output pointer, receives the maximum RF gain in dB
 *
 * Returns: 0 (`SDDC_SUCCESS`) on success, -1 (`SDDC_ERROR`) if any pointer is NULL.
 */
int sddc_get_rf_gain_range(struct sddc_dev_t *_dev, float *min, float *max);

/**
 * Get the discrete RF gain steps supported by this device and sampling mode.
 *
 * The returned pointer points to a static array owned by the library; do not free it.
 *
 * - `dev`: device handle
 * - `steps`: output pointer, receives a pointer to an array of gain values in dB
 *
 * Returns: number of entries in the steps array on success, -1 (`SDDC_ERROR`) if any pointer is NULL.
 */
int sddc_get_rf_gain_steps(struct sddc_dev_t *_dev,
                           const float **steps);

/**
 * Get the current center frequency in Hz (64-bit).
 *
 * - `dev`: device handle
 *
 * Returns: center frequency in Hz, or 0 if `dev` is NULL.
 */
uint64_t sddc_get_center_freq64(struct sddc_dev_t *dev);

/**
 * Set the center frequency for the device (64-bit).
 *
 * May be called while streaming; the new frequency is applied immediately.
 * In direct-sampling mode this parameter is informational only and does not
 * affect hardware — the full ADC bandwidth is always captured.
 *
 * - `dev`: device handle
 * - `freq`: center frequency in Hz
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)   on success
 * - -1 (`SDDC_ERROR`)     if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
 */
int sddc_set_center_freq64(struct sddc_dev_t *dev, uint64_t freq);

/**
 * Get current ADC sample rate in Hz.
 *
 * Equals the crystal oscillator frequency set via `sddc_set_xtal_freq()`.
 *
 * - `dev`: device handle
 *
 * Returns: sample rate in Hz, or 0 if `dev` is NULL.
 */
uint32_t sddc_get_sample_rate(struct sddc_dev_t *dev);

/**
 * Enable or disable direct sampling mode.
 *
 * In direct-sampling mode the HF input is routed straight to the ADC,
 * giving wideband coverage from DC to the Nyquist frequency. In tuner
 * mode a downstream mixer/tuner covers VHF/UHF bands.
 * This setting cannot be changed while streaming.
 *
 * - `dev`: device handle
 * - `on`: 1 = direct sampling (HF), 0 = tuner path (VHF/UHF)
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)     on success
 * - -1 (`SDDC_ERROR`)       if `dev` is NULL
 * - -2 (`SDDC_ERROR_BUSY`)  if device is currently streaming
 */
int sddc_set_direct_sampling(struct sddc_dev_t *dev, int on);

/**
 * Get state of direct sampling mode.
 *
 * - `dev`: device handle
 *
 * Returns: 1 if direct sampling is enabled, 0 if disabled, -1 (`SDDC_ERROR`) if `dev` is NULL.
 */
int sddc_get_direct_sampling(struct sddc_dev_t *dev);

/**
 * Start asynchronous sample streaming.
 *
 * Configures and starts the ADC, then blocks in the calling thread,
 * invoking `cb` repeatedly with sample buffers until `sddc_cancel_async()`
 * is called from another thread. When the callback receives `count == 0`,
 * a streaming error has occurred.
 *
 * All configuration (gain, frequency, crystal frequency, direct sampling)
 * must be set before calling this function.
 *
 * - `dev`: device handle
 * - `cb`: callback function invoked with each batch of samples
 * - `ctx`: user context pointer forwarded unchanged to every `cb` invocation
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)        on success (after streaming ends)
 * - -1 (`SDDC_ERROR`)          if `dev` or `cb` is NULL
 * - -2 (`SDDC_ERROR_BUSY`)     if streaming is already in progress
 * - -4 (`SDDC_ERROR_IO`)       on USB communication failure during setup
 * - -6 (`SDDC_ERROR_USB_SPEED`) if device is not at SuperSpeed
 */
int sddc_read_async(struct sddc_dev_t *dev, sddc_read_async_cb_t cb, void *ctx);

/**
 * Stop asynchronous streaming started by `sddc_read_async()`.
 *
 * Signals the streaming thread to stop, joins it, then powers down the ADC.
 * Safe to call even if no streaming is in progress.
 *
 * - `dev`: device handle
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)   on success
 * - -1 (`SDDC_ERROR`)     if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)  on USB communication failure during ADC shutdown
 */
int sddc_cancel_async(struct sddc_dev_t *dev);

/**
 * Enable or disable the antenna bias-tee voltage.
 *
 * The bias-tee supplies DC power to an active antenna or low-noise
 * amplifier through the coax cable.
 *
 * - `dev`: device handle
 * - `on`: bitmask — bit 0 = HF port bias, bit 1 = VHF/UHF port bias
 *   (0 = both off, 1 = HF on, 2 = VHF on, 3 = both on)
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)   on success
 * - -1 (`SDDC_ERROR`)     if `dev` is NULL
 * - -3 (`SDDC_ERROR_INVALID_PARAM`) if an index is out of range
 * - -4 (`SDDC_ERROR_IO`)  on USB communication failure
 */
int sddc_enable_bias_tee(struct sddc_dev_t *dev, int on);

/**
 * Enable or disable ADC dither.
 *
 * Dither adds a small, shaped noise signal to the ADC input to reduce
 * harmonic spurs at the cost of a slightly elevated noise floor.
 * May be toggled while streaming; applied immediately.
 *
 * - `dev`: device handle
 * - `on`: 1 = enable dither, 0 = disable dither
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)   on success
 * - -1 (`SDDC_ERROR`)     if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
 */
int sddc_enable_adc_dither(struct sddc_dev_t *dev, int on);

/**
 * Enable or disable the ADC programmable gain amplifier (PGA).
 *
 * The PGA increases ADC input sensitivity. May be toggled while streaming;
 * applied immediately.
 *
 * - `dev`: device handle
 * - `on`: 1 = enable PGA, 0 = disable PGA
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)   on success
 * - -1 (`SDDC_ERROR`)     if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
 */
int sddc_enable_adc_pga(struct sddc_dev_t *dev, int on);

/**
 * Enable or disable ADC output bit randomization (de-randomization applied on host).
 *
 * When enabled, the ADC XORs each sample with a known pattern to reduce
 * spectral leakage from the digital logic. The host driver automatically
 * reverses the randomization before delivering samples to the callback.
 * Must be set before calling `sddc_read_async()`; cannot be changed while streaming.
 *
 * - `dev`: device handle
 * - `on`: 1 = enable randomization, 0 = disable
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)     on success
 * - -1 (`SDDC_ERROR`)       if `dev` is NULL
 * - -2 (`SDDC_ERROR_BUSY`)  if device is currently streaming
 */
int sddc_enable_adc_rando(struct sddc_dev_t *dev, int on);

/**
 * Get the installed firmware version as a packed 16-bit value.
 *
 * Format: `0xMMmm` where `MM` is the major version and `mm` is the minor version.
 * The current required version is defined at build time and enforced by `sddc_open()`.
 *
 * - `dev`: device handle
 *
 * Returns: packed firmware version `(major << 8) | minor`, or 0 if `dev` is NULL.
 */
uint16_t sddc_get_firmware_version(struct sddc_dev_t *dev);

/**
 * Enable or disable HF input high-impedance mode.
 *
 * In high-Z mode the HF input is switched to a high-impedance termination
 * for use with antennas that include their own preamplifier. In low-Z mode
 * (default) the input is 50 Ω matched. May be toggled while streaming;
 * applied immediately.
 *
 * - `dev`: device handle
 * - `on`: 1 = high-Z input, 0 = 50 Ω input
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)   on success
 * - -1 (`SDDC_ERROR`)     if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
 */
int sddc_enable_hf_highz(struct sddc_dev_t *dev, int on);

/**
 * Enable or disable external clock input (RX888 PRO only).
 *
 * When enabled, the ADC clock is derived from a signal applied to the
 * external clock input rather than the on-board crystal oscillator.
 * Use this to phase-lock multiple units or improve long-term frequency
 * accuracy with an external reference. May be changed while streaming;
 * applied immediately.
 *
 * - `dev`: device handle
 * - `on`: 1 = use external clock, 0 = use internal crystal
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)   on success
 * - -1 (`SDDC_ERROR`)     if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
 */
int sddc_enable_ext_clock(struct sddc_dev_t *dev, int on);

/**
 * Set the ADC anti-aliasing filter mode (RX888 PRO only).
 *
 * Selects between the on-board LPF options or bypass mode.
 * May be changed while streaming; applied immediately.
 *
 * - `dev`: device handle
 * - `mode`: one of `Freq64MHz`, `Freq32MHz`, `FMUndersample`, or `Bypass`
 *
 * Returns:
 * -  0 (`SDDC_SUCCESS`)   on success
 * - -1 (`SDDC_ERROR`)     if `dev` is NULL
 * - -4 (`SDDC_ERROR_IO`)  on USB communication failure when streaming
 */
int sddc_set_adc_filter(struct sddc_dev_t *dev, enum FilterMode mode);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* LIBSDDC_H */
