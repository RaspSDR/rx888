/*
 * libsddc - low level functions for wideband SDR receivers like
 *           BBRF103, RX-666, RX888, HF103, etc
 *
 * Copyright (C) 2020 by Franco Venturi
 *
 * this program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * this program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#ifndef __LIBSDDC_H
#define __LIBSDDC_H

#include <stdint.h>
#include <stddef.h>


#if defined __GNUC__
#  if __GNUC__ >= 4
#    define __SDR_EXPORT   __attribute__((visibility("default")))
#    define __SDR_IMPORT   __attribute__((visibility("default")))
#  else
#    define __SDR_EXPORT
#    define __SDR_IMPORT
#  endif
#elif _MSC_VER
#  define __SDR_EXPORT     __declspec(dllexport)
#  define __SDR_IMPORT     __declspec(dllimport)
#else
#  define __SDR_EXPORT
#  define __SDR_IMPORT
#endif

#ifndef sddc_STATIC
#	ifdef sddc_EXPORTS
#	define SDDC_API __SDR_EXPORT
#	else
#	define SDDC_API __SDR_IMPORT
#	endif
#else
#define SDDC_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct sddc_dev sddc_dev_t;

SDDC_API uint32_t sddc_get_device_count(void);

SDDC_API const char* sddc_get_device_name(uint32_t index);

/*!
 * Get USB device strings.
 *
 * NOTE: The string arguments must provide space for up to 256 bytes.
 *
 * \param index the device index
 * \param manufact manufacturer name, may be NULL
 * \param product product name, may be NULL
 * \param serial serial number, may be NULL
 * \return 0 on success
 */
SDDC_API int sddc_get_device_usb_strings(uint32_t index,
					     char *manufact,
					     char *product,
					     char *serial);

/*!
 * Get device index by USB serial string descriptor.
 *
 * \param serial serial string of the device
 * \return device index of first device where the name matched
 * \return -1 if name is NULL
 * \return -2 if no devices were found at all
 * \return -3 if devices were found, but none with matching name
 */
SDDC_API int sddc_get_index_by_serial(const char *serial);

/*!
 * Open the device.
 *
 * \param dev the device handle
 * \param index the device index
 * \return 0 on success
 */
SDDC_API int sddc_open(sddc_dev_t **dev, uint32_t index);

/*!
 * Open the device at the raw mode.
 *
 * At raw mode, the data read from the device will be real int16_t samples
 * at the sampling rate * 2. Also, the device will not support tuning at
 * direct sampling mode (HF Band).
 *
 * \param dev the device handle
 * \param index the device index
 * \return 0 on success
 */
SDDC_API int sddc_open_raw(sddc_dev_t** dev, uint32_t index);

/*!
 * Close the device.
 *
 * \param dev the device handle given by sddc_open()
 * \return 0 on success
 */
SDDC_API int sddc_close(sddc_dev_t *dev);	

/*!
 * Get USB device strings.
 *
 * NOTE: The string arguments must provide space for up to 256 bytes.
 *
 * \param dev the device handle given by sddc_open()
 * \param manufact manufacturer name, may be NULL
 * \param product product name, may be NULL
 * \param serial serial number, may be NULL
 * \return 0 on success
 */
SDDC_API int sddc_get_usb_strings(sddc_dev_t *dev, char *manufact,
				      char *product, char *serial);

/*!
 * Set crystal oscillator frequencies used for the ADC.
 *
 * The default frequency is 62Mhz for most of the devices. This frequency depends on
 * the bandwith and usable frequency range when in the direct sampling mode.
 *
 * NOTE: Call this function only if you fully understand the implications.
 *
 * \param dev the device handle given by rtlsdr_open()
 * \param rtl_freq frequency value used to clock ADC in Hz
 * \return 0 on success
 */
SDDC_API int sddc_set_xtal_freq(sddc_dev_t *dev, uint32_t rtl_freq);

/*!
 * Get crystal oscillator frequencies used for the ADC.
 *
 * Usually both ICs use the same clock.
 *
 * \param dev the device handle given by rtlsdr_open()
 * \param rtl_freq frequency value used to clock the ADC in Hz
  * \return 0 on success
 */
SDDC_API int sddc_get_xtal_freq(sddc_dev_t *dev, uint32_t *rtl_freq);

/*!
 * Set the IF gain value.
 *
 * \param dev the device handle given by sddc_open()
 * \param value gain value
 * \return 0 on success
 */
SDDC_API int sddc_set_if_gain(sddc_dev_t *dev, int value);

/*!
 * Set the RF attenuator value.
 *
 * \param dev the device handle given by sddc_open()
 * \param value attenuator value
 * \return 0 on success
 */
SDDC_API int sddc_set_rf_attenuator(sddc_dev_t *dev, int value);

/*!
 * Get the IF gain value.
 *
 * \param dev the device handle given by sddc_open()
 * \param value pointer to store gain value
 * \return 0 on success
 */
SDDC_API int sddc_get_if_gain(sddc_dev_t *dev, int *value);

/*!
 * Get the RF attenuator value.
 *
 * \param dev the device handle given by sddc_open()
 * \param value pointer to store attenuator value
 * \return 0 on success
 */
SDDC_API int sddc_get_rf_attenuator(sddc_dev_t *dev, int *value);

/*!
 * Get actual frequency the device is tuned to.
 *
 * \param dev the device handle given by sddc_open()
 * \return 0 on error, frequency in Hz otherwise
 */
SDDC_API uint32_t sddc_get_center_freq(sddc_dev_t *dev);
SDDC_API uint64_t sddc_get_center_freq64(sddc_dev_t *dev);

/*!
 * Set the center frequency for the device.
 *
 * \param dev the device handle given by sddc_open()
 * \param freq frequency in Hz
 * \return 0 on success
 * 		   -1 when the frequency is out of range
 * 		   -2 when the frequency setting needs stop read first
 */
SDDC_API int sddc_set_center_freq(sddc_dev_t *dev, uint32_t freq);
SDDC_API int sddc_set_center_freq64(sddc_dev_t *dev, uint64_t freq);

/*!
 * Set the sample rate for the device, also selects the baseband filters
 * according to the requested sample rate for tuners where this is possible.
 * 
 * note: raw mode doesn't support set sample rate.
 *
 * \param dev the device handle given by sddc_open()
 * \param samp_rate the sample rate to be set, possible values are:
 * 		    225001 - 300000 Hz
 * 		    900001 - 3200000 Hz
 * 		    sample loss is to be expected for rates > 2400000
 * \return 0 on success, -EINVAL on invalid rate
 */
SDDC_API int sddc_set_sample_rate(sddc_dev_t *dev, uint32_t rate);

/*!
 * Get actual sample rate the device is configured to.
 *
 * note: sample rate is always equal to ADC XTAL frequency divided by 2 in raw mode.
 * 
 * \param dev the device handle given by sddc_open()
 * \return 0 on error, sample rate in Hz otherwise
 */
SDDC_API uint32_t sddc_get_sample_rate(sddc_dev_t *dev);

/*!
 * Enable or disable the direct sampling mode. When enabled, the input
 * from ADC will be directly send to the application without any mixing or
 * filtering. This mode is useful for using the device as a wideband receiver.
 *
 * \param dev the device handle given by sddc_open()
 * \param on 0 means disabled, 1 enabled
 * \return 0 on success
 */
SDDC_API int sddc_set_direct_sampling(sddc_dev_t *dev, int on);

/*!
 * Get state of the direct sampling mode
 *
 * \param dev the device handle given by sddc_open()
 * \return -1 on error, 0 means disabled, 1 enabled
 */
SDDC_API int sddc_get_direct_sampling(sddc_dev_t *dev);

/* streaming functions */

/*!
 * Reset the internal sample buffer.
 *
 * \param dev the device handle given by sddc_open()
 * \return 0 on success
 */
SDDC_API int sddc_reset_buffer(sddc_dev_t *dev);

/*!
 * Read samples from the device synchronously.
 *
 * \param dev the device handle given by sddc_open()
 * \param buf buffer to store received samples
 * \param len length of buffer in bytes
 * \param n_read pointer to store number of bytes actually read
 * \return 0 on success
 */
SDDC_API int sddc_read_sync(sddc_dev_t *dev, void *buf, int len, int *n_read);

typedef void(*sddc_read_async_cb_t)(unsigned char *buf, uint32_t len, void *ctx);

/*!
 * Read samples from the device asynchronously. This function will block
 * until the read is canceled via sddc_cancel_async().
 *
 * \param dev the device handle given by rtlsdr_open()
 * \param cb callback function to return received samples
 * \param ctx user specific context to pass via the callback function
 * \param buf_num optional buffer count, buf_num * buf_len = overall buffer size
 *		  set to 0 for default buffer count (15)
 * \param buf_len optional buffer length, must be multiple of 512,
 *		  should be a multiple of 16384 (URB size), set to 0
 *		  for default buffer length (16 * 32 * 512)
 * \return 0 on success
 */
SDDC_API int sddc_read_async(sddc_dev_t *dev,
				 sddc_read_async_cb_t cb,
				 void *ctx,
				 uint32_t buf_num,
				 uint32_t buf_len);

/*!
 * Cancel all pending asynchronous operations on the device.
 *
 * \param dev the device handle given by rtlsdr_open()
 * \return 0 on success
 */
SDDC_API int sddc_cancel_async(sddc_dev_t *dev);

/*!
 * Enable or disable the bias tee on GPIO PIN 0.
 *
 * \param dev the device handle given by sddc_open()
 * \param on  0 for Bias T off. 1 for HF Bias T on. 2 for VHF Bias T on. 3 for both on.
 * \return -1 if device is not initialized. 0 otherwise.
 */
SDDC_API int sddc_set_bias_tee(sddc_dev_t *dev, int on);

#ifdef __cplusplus
}
#endif

#endif /* __LIBSDDC_H */