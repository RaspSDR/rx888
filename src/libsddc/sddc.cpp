#include <assert.h>

#include "sddc.h"
#include "config.h"
#include "r2iq.h"
#include "hardware.h"
#include "FX3Class.h"

enum DeviceStatus
{
    DEVICE_IDLE = 0,
    DEVICE_RUNNING,
    DEVICE_CANCELING,
    DEVICE_ERROR
};

struct sddc_dev
{
    RadioHardware *hardware;
    uint8_t led;
    int samplerateidx;
    uint32_t target_sample_rate;

    bool raw_mode;

    r2iqControlClass *r2iq;

    sddc_read_async_cb_t callback;
    void *callback_context;

    DeviceStatus status;

    // stats which supports get
    uint64_t center_frequency;
    uint32_t sample_rate;
    int rf_attenuator;
    int if_gain;
    int direct_sampling;
};

static void Callback(void *context, const float *data, uint32_t len)
{
    struct sddc_dev *dev = (struct sddc_dev *)context;

    assert(!dev->raw_mode);

    if (dev->callback)
    {
        dev->callback((unsigned char *)data, len * 2 * sizeof(float), dev->callback_context);
    }
}

uint32_t sddc_get_device_count(void)
{
    int count = 0;
    while (true)
    {
        if (!EnumerateFx3Devices(count, nullptr, nullptr))
            break;
        count++;
    }

    return count;
}

const char *sddc_get_device_name(uint32_t index)
{
    static char lbuf[256];
    uint8_t idx = (uint8_t)index;
    if (!EnumerateFx3Devices(idx, lbuf, nullptr))
        return nullptr;

    return lbuf;
}

int sddc_get_device_usb_strings(uint32_t index,
                                char *manufact,
                                char *product,
                                char *serial)
{
    uint8_t idx = (uint8_t)index;
    if (!EnumerateFx3Devices(idx, product, serial))
        return 1;

    if (manufact)
        strcpy(manufact, "SDDC");

    return 0;
}

int sddc_get_index_by_serial(const char *serial)
{
    uint8_t idx = 0;
    char sn[256];

    if (serial == nullptr || *serial == '\0')
    {
        return -EINVAL;
    }

    while (EnumerateFx3Devices(idx, nullptr, sn))
    {
        if (strcmp(sn, serial) == 0)
            return idx;
        idx++;
    }

    return -3;
}

int sddc_set_xtal_freq(sddc_dev_t *dev, uint32_t rtl_freq)
{
    return -ENOSYS;
}

int sddc_open(sddc_dev_t **dev, uint32_t index)
{
    auto ret_val = new sddc_dev();

    fx3class *fx3 = CreateUsbHandler(index);
    if (fx3 == nullptr)
    {
        return -EBADF;
    }

    bool openOK = fx3->Open();
    if (!openOK)
    {
        delete fx3;
        delete ret_val;
        return -EBADF;
    }

    ret_val->hardware = CreateRadioHardwareInstance(fx3);
    ret_val->hardware->FX3producerOff();

    // check firmware version

    if (ret_val->hardware->getFirmware() != FIRMWARE_VER_MAJOR * 256 + FIRMWARE_VER_MINOR)
    {
        ret_val->hardware->FX3SetGPIO(LED_RED);

        delete ret_val->hardware;
        delete ret_val;
        return -EPERM;
    }

    ret_val->hardware->FX3SetGPIO(LED_BLUE);
    ret_val->hardware->Initialize(DEFAULT_ADC_FREQ);
    *dev = ret_val;

    return 0;
}

int sddc_open_raw(sddc_dev_t **dev, uint32_t index)
{
    int ret = sddc_open(dev, index);
    if (ret != 0)
        return ret;

    (*dev)->raw_mode = true;
    return ret;
}

int sddc_close(sddc_dev_t *dev)
{
    if (!dev)
        return -EINVAL;

    if (dev->hardware)
        delete dev->hardware;

    delete dev;

    return 0;
}

int sddc_set_bias_tee(sddc_dev_t *dev, int on)
{
    if (!dev)
        return -EINVAL;

    if (on & 0x01)
        dev->hardware->FX3SetGPIO(BIAS_HF);
    else
        dev->hardware->FX3UnsetGPIO(BIAS_HF);

    if (on & 0x02)
        dev->hardware->FX3SetGPIO(BIAS_VHF);
    else
        dev->hardware->FX3UnsetGPIO(BIAS_VHF);

    return 0;
}

int sddc_set_sample_rate(sddc_dev_t *dev, uint32_t rate)
{
    if (!dev)
        return -EINVAL;

    if (dev->status != DEVICE_IDLE)
        return -EPERM;

    if (dev->raw_mode)
    {
        // in the raw mode, we set xtal to 2x of sample rate
        // this is not good way to use the device as the limitation of
        // LPF in front of ADC. However user may already have another LPF
        // after the attenna.
        dev->hardware->Initialize(rate * 2);
        dev->samplerateidx = 0; // not used
    }
    else
    {
        dev->target_sample_rate = 0;
        switch ((int64_t)rate)
        {
        case 32000000:
            dev->samplerateidx = 0;
            break;
        case 16000000:
            dev->samplerateidx = 1;
            break;
        case 8000000:
            dev->samplerateidx = 2;
            break;
        case 4000000:
            dev->samplerateidx = 3;
            break;
        case 2000000:
            dev->samplerateidx = 4;
            break;
        default:
            if (rate < 2000000)
            {
                dev->samplerateidx = 4;
                // we need futher down-sampling
                dev->target_sample_rate = rate;
                break;
            }
            else
            {
                return -EINVAL;
            }
        }
    }

    return 0;
}

uint32_t sddc_get_center_freq(sddc_dev_t *dev)
{
    return (uint32_t)sddc_get_center_freq64(dev);
}

uint64_t sddc_get_center_freq64(sddc_dev_t *dev)
{
    if (!dev)
        return -EINVAL;

    // TODO: Is this correct
    return dev->hardware->TuneLo(0);
}

int sddc_set_center_freq(sddc_dev_t *dev, uint32_t freq)
{
    return sddc_set_center_freq64(dev, (uint64_t)freq);
}

int sddc_set_center_freq64(sddc_dev_t *dev, uint64_t freq)
{
    if (!dev)
        return -EINVAL;

    if (dev->direct_sampling && dev->raw_mode)
    {
        // direct sampling mode at raw mode doesn't support tuning
        return -EPERM;
    }

    if (dev->direct_sampling)
    {
        // set software NCO frequency
    }
    else
    {
        // set tuner LO frequency
        uint64_t actLo = dev->hardware->TuneLo(freq);
    }

    return 0;
}

int sddc_set_rf_attenuator(sddc_dev_t *dev, int value)
{
    if (!dev)
        return -EINVAL;

    const float *steps;
    int nstep = dev->hardware->getRFSteps(&steps);

    int i = 0;
    for (; i < nstep - 1; i++)
    {
        if (steps[i] >= value)
        {
            dev->hardware->UpdateattRF(i);
            break;
        }
    }

    dev->hardware->UpdateattRF(i);
    dev->rf_attenuator = steps[i];

    return 0;
}

int sddc_get_rf_attenuator(sddc_dev_t *dev, int *value)
{
    if (!dev || !value)
        return -EINVAL;

    if (value)
        *value = dev->rf_attenuator;

    return 0;
}

int sddc_set_if_gain(sddc_dev_t *dev, int value)
{
    if (!dev)
        return -EINVAL;

    const float *steps;
    int nstep = dev->hardware->getIFSteps(&steps);

    int i = 0;
    for (; i < nstep - 1; i++)
    {
        if (steps[i] >= value)
        {
            dev->hardware->UpdateGainIF(i);
            break;
        }
    }

    dev->hardware->UpdateGainIF(i);
    dev->if_gain = steps[i];

    return 0;
}

int sddc_get_if_gain(sddc_dev_t *dev, int *value)
{
    if (!dev || !value)
        return -EINVAL;

    if (value)
        *value = dev->if_gain;

    return 0;
}

uint32_t sddc_get_sample_rate(sddc_dev_t *dev)
{
    if (!dev)
        return 0;

    switch (dev->samplerateidx)
    {
    case 0:
        return 32000000;
    case 1:
        return 16000000;
    case 2:
        return 8000000;
    case 3:
        return 4000000;
    case 4:
        if (dev->target_sample_rate != 0)
            return dev->target_sample_rate;
        else
            return 2000000;
    default:
        return 0;
    }
}

/*!
 * Enable or disable the direct sampling mode. When enabled, the input
 * from ADC will be directly send to the application without any mixing or
 * filtering. This mode is useful for using the device as a wideband receiver.
 *
 * \param dev the device handle given by sddc_open()
 * \param on 0 means disabled, 1 enabled
 * \return 0 on success
 */
int sddc_set_direct_sampling(sddc_dev_t *dev, int on)
{
    if (!dev)
        return -EINVAL;

    if (dev->status != DEVICE_IDLE)
        return -EPERM;

    dev->direct_sampling = on;
    return 0;
}

/*!
 * Get state of the direct sampling mode
 *
 * \param dev the device handle given by sddc_open()
 * \return -1 on error, 0 means disabled, 1 enabled
 */
int sddc_get_direct_sampling(sddc_dev_t *dev)
{
    if (!dev)
        return -EINVAL;

    return dev->direct_sampling;
}

int sddc_read_sync(sddc_dev_t *dev, void *buf, int len, int *n_read)
{
    if (dev->status != DEVICE_IDLE)
        return -EBUSY;

    // Not Implemented

    return -1;
}

int sddc_read_async(sddc_dev_t *dev,
                    sddc_read_async_cb_t cb,
                    void *ctx,
                    uint32_t buf_num,
                    uint32_t buf_len)
{
    if (dev->status != DEVICE_IDLE)
        return -EBUSY;

    dev->callback = cb;
    dev->callback_context = ctx;

    dev->status = DEVICE_RUNNING;

    if (dev->raw_mode)
    {
        if (buf_num == 0)
            buf_num = 16;

        if (buf_len == 0)
            buf_len = 16384;

        dev->hardware->FX3producerOn(); // FX3 start the producer

        ringbuffer<int16_t> inputbuffer(buf_num);

        dev->hardware->StartStream(inputbuffer, buf_num);

        uint32_t len = inputbuffer.getBlockSize();
        // read the data
        while (dev->status == DEVICE_RUNNING)
        {
            auto ptr = inputbuffer.getReadPtr();
            cb((unsigned char *)ptr, len, ctx);
            inputbuffer.ReadDone();
        }
    }
    else
    {
    }

    dev->hardware->FX3producerOff(); // FX3 stop the producer
    dev->hardware->StopStream();
    dev->status = DEVICE_IDLE;

    return 0;
}

int sddc_cancel_async(sddc_dev_t *dev)
{
    if (dev->status != DEVICE_RUNNING)
        return -EBUSY;

    dev->status = DEVICE_CANCELING;

    return 0;
}