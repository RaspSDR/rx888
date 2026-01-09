#ifndef LIBSDDC_H
#define LIBSDDC_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

struct sddc_dev_t;

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
 * - -1 if `serial` is NULL
 * - -2 if no devices were found
 * - -3 if devices were found, but none matched
 */
int sddc_get_index_by_serial(const char *serial);

/**
 * Open the device.
 *
 * - `dev`: output device handle pointer
 * - `index`: device index
 *
 * Returns: 0 on success.
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
 * Default is 62 MHz for most devices. Changing this affects
 * bandwidth and usable frequency in direct sampling mode.
 * Call only if you fully understand the implications.
 *
 * - `dev`: device handle
 * - `rtl_freq`: ADC clock in Hz (sample rate)
 *
 * Returns: 0 on success.
 */
int sddc_set_xtal_freq(struct sddc_dev_t *dev, uint32_t rtl_freq);

/**
 * Get ADC crystal oscillator frequency.
 *
 * - `dev`: device handle
 * - `rtl_freq`: out pointer for ADC clock in Hz
 *
 * Returns: 0 on success.
 */
int sddc_get_xtal_freq(struct sddc_dev_t *dev, uint32_t *rtl_freq);

/**
 * Set the IF gain value.
 *
 * - `dev`: device handle
 * - `value`: gain value
 *
 * Returns: 0 on success.
 */
int sddc_set_if_gain(struct sddc_dev_t *dev, float value);

/**
 * Set the RF gain value.
 *
 * - `dev`: device handle
 * - `value`: gain value
 *
 * Returns: 0 on success.
 */
int sddc_set_rf_gain(struct sddc_dev_t *dev, float value);

/**
 * Get the IF gain value.
 *
 * - `dev`: device handle
 * - `value`: out pointer for gain value
 *
 * Returns: 0 on success.
 */
int sddc_get_if_gain(struct sddc_dev_t *dev, float *value);

/**
 * Get the IF gain range.
 *
 * - `dev`: device handle
 * - `min`: out pointer for minimum value
 * - `max`: out pointer for maximum value
 *
 * Returns: 0 on success.
 */
int sddc_get_if_gain_range(struct sddc_dev_t *_dev, float *min, float *max);

/**
 * Get the IF gain steps.
 *
 * - `dev`: device handle
 * - `steps`: out pointer to a steps array
 *
 * Returns: number of steps on success.
 */
int sddc_get_if_gain_steps(struct sddc_dev_t *_dev, const float **steps);

/**
 * Get the RF gain value.
 *
 * - `dev`: device handle
 * - `value`: out pointer for gain value
 *
 * Returns: 0 on success.
 */
int sddc_get_rf_gain(struct sddc_dev_t *dev, float *value);

/**
 * Get the RF gain range.
 *
 * - `dev`: device handle
 * - `min`: out pointer for minimum value
 * - `max`: out pointer for maximum value
 *
 * Returns: 0 on success.
 */
int sddc_get_rf_gain_range(struct sddc_dev_t *_dev, float *min, float *max);

/**
 * Get the RF gain steps.
 *
 * - `dev`: device handle
 * - `steps`: out pointer to a steps array
 *
 * Returns: number of steps on success.
 */
int sddc_get_rf_gain_steps(struct sddc_dev_t *_dev, const float **steps);

/**
 * Get the current center frequency in Hz.
 *
 * - `dev`: device handle
 *
 * Returns: frequency in Hz, or 0 on error.
 */
uint32_t sddc_get_center_freq(struct sddc_dev_t *dev);

/**
 * Get the current center frequency in Hz (64-bit).
 *
 * - `dev`: device handle
 *
 * Returns: frequency in Hz, or 0 on error.
 */
uint64_t sddc_get_center_freq64(struct sddc_dev_t *dev);

/**
 * Set the center frequency for the device.
 *
 * - `dev`: device handle
 * - `freq`: frequency in Hz
 *
 * Returns:
 * - 0 on success
 * - -1 if frequency is out of range
 * - -2 if setting requires stopping read first
 */
int sddc_set_center_freq(struct sddc_dev_t *dev, uint32_t freq);

/**
 * Set the center frequency for the device (64-bit).
 *
 * - `dev`: device handle
 * - `freq`: frequency in Hz
 *
 * Returns:
 * - 0 on success
 * - -1 if frequency is out of range
 * - -2 if setting requires stopping read first
 */
int sddc_set_center_freq64(struct sddc_dev_t *dev, uint64_t freq);

/**
 * Get current sample rate in Hz.
 *
 * Note: In raw mode, sample rate equals ADC XTAL frequency / 2.
 *
 * - `dev`: device handle
 *
 * Returns: sample rate in Hz, or 0 on error.
 * At SDR device level, sample rate equals xtal_freq.
 */
uint32_t sddc_get_sample_rate(struct sddc_dev_t *dev);

/**
 * Enable or disable direct sampling mode.
 *
 * When enabled, ADC input is sent to the application without mixing
 * or filtering. Useful for wideband reception.
 *
 * - `dev`: device handle
 * - `on`: 0 = disabled, 1 = enabled
 *
 * Returns: 0 on success.
 */
int sddc_set_direct_sampling(struct sddc_dev_t *dev, int on);

/**
 * Get state of direct sampling mode.
 *
 * - `dev`: device handle
 *
 * Returns: -1 on error, 0 = disabled, 1 = enabled.
 */
int sddc_get_direct_sampling(struct sddc_dev_t *dev);

/**
 * Read samples asynchronously; blocks until canceled via `sddc_cancel_async()`.
 *
 * - `dev`: device handle
 * - `cb`: callback invoked with received samples
 * - `ctx`: user context passed to callback
 *
 * Returns: 0 on success.
 */
int sddc_read_async(struct sddc_dev_t *dev, sddc_read_async_cb_t cb, void *ctx);

/**
 * Cancel all pending asynchronous operations on the device.
 *
 * - `dev`: device handle
 *
 * Returns: 0 on success.
 */
int sddc_cancel_async(struct sddc_dev_t *dev);

/**
 * Enable or disable the bias tee on GPIO PIN 0.
 *
 * - `dev`: device handle
 * - `on`: 0 = off, 1 = HF on, 2 = VHF on, 3 = both on
 *
 * Returns: -1 if device is not initialized, 0 otherwise.
 */
int sddc_enable_bias_tee(struct sddc_dev_t *dev, int on);

/**
 * Enable or disable ADC dither.
 *
 * - `dev`: device handle
 * - `on`: 0 = off, 1 = on
 *
 * Returns: -1 if device is not initialized, 0 otherwise.
 */
int sddc_enable_adc_dither(struct sddc_dev_t *dev, int on);

/**
 * Enable or disable ADC PGA
 *
 * - `dev`: device handle
 * - `on`: 0 = off, 1 = on
 *
 * Returns: -1 if device is not initialized, 0 otherwise.
 */
int sddc_enable_adc_pga(struct sddc_dev_t *dev, int on);

/**
 * Enable or disable ADC RANDO, only enable this before start reading
 *
 * - `dev`: device handle
 * - `on`: 0 = off, 1 = on
 *
 * Returns: -1 if device is not initialized or the device is busy, 0 otherwise.
 */
int sddc_enable_adc_rando(struct sddc_dev_t *dev, int on);

/**
 * Get firmware version in format 0xMMmm (MM=major, mm=minor).
 *
 * - `dev`: device handle
 *
 * Returns: version as 16-bit value.
 */
uint16_t sddc_get_firmware_version(struct sddc_dev_t *dev);

/**
 * Enable or disable ADC RANDO, only enable this before start reading
 *
 * - `dev`: device handle
 * - `on`: 0 = off, 1 = on
 *
 * Returns: -1 if device is not initialized or the device is busy, 0 otherwise.
 */
int sddc_enable_hf_highz(struct sddc_dev_t *dev, int on);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* LIBSDDC_H */
