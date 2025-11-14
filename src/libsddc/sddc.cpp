#include <assert.h>
#include <string.h>

#include "sddc.h"
#include "config.h"
#include "fft_mt_r2iq.h"
#include "hardware.h"
#include "fx3class.h"
#include "pffft/pf_mixer.h"

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

    // only change when open the device
    bool raw_mode;

    r2iqControlClass *r2iq;
    shift_limited_unroll_C_sse_data_t stateFineTune;

    sddc_read_async_cb_t callback;
    void *callback_context;

    DeviceStatus status;

    // stats which supports get
    uint64_t center_frequency;
    float rf_attenuator;
    float if_gain;
    uint8_t led;

    std::mutex fc_mutex;
    float fc;

    // only support change when the device is idle
    int direct_sampling;
    uint32_t target_sample_rate;
    int samplerateidx;

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
    if (!dev)
        return -EINVAL;

    if (dev->status != DEVICE_IDLE) {
        return -EBUSY;
    }

    // TODO:

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
    ret_val->target_sample_rate = DEFAULT_ADC_FREQ / 2;

    // start the r2iq processor
    ret_val->r2iq = new fft_mt_r2iq();

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

    if (dev->status != DEVICE_IDLE) {
        return -EBUSY;
    }

    if (dev->hardware) {
        dev->hardware->FX3producerOff();
        delete dev->hardware;
    }

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
        // we only allow 64Msps and 32Msps at raw mode
        if (rate == 32000000)
            dev->target_sample_rate = 32000000;
        else
            return -EINVAL;
    }
    else
    {
        // we only allow 32Msps, 16Msps, 8Msps, 4Msps, 2Msps
        // and various rate under 2Msps
        if (rate == 32000000)
            dev->target_sample_rate = 32000000;
        else if (rate == 16000000)
            dev->target_sample_rate = 16000000;
        else if (rate == 8000000)
            dev->target_sample_rate = 8000000;
        else if (rate == 4000000)
            dev->target_sample_rate = 4000000;
        else if (rate == 2000000)
            dev->target_sample_rate = 2000000;
        else if (rate < 2000000) {
            // TODO: Support more flexible sample rates
            // return 0;
            return -EINVAL;
        }
        else
            return -EINVAL;
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

int sddc_set_center_freq64(sddc_dev_t *dev, uint64_t wishedFreq)
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
        uint64_t actLo = dev->hardware->TuneLo(wishedFreq);
        
        // we need shift the samples
        int64_t offset = wishedFreq - actLo;
        float fc = dev->r2iq->setFreqOffset(offset / (dev->target_sample_rate / 2.0f));
        if (dev->fc != fc)
        {
            std::unique_lock<std::mutex> lk(dev->fc_mutex);
            dev->stateFineTune = shift_limited_unroll_C_sse_init(fc, 0.0F);
            dev->fc = fc;
        }
    }
    else
    {
        // set tuner LO frequency
        dev->hardware->TuneLo(wishedFreq);
    }

    return 0;
}

int sddc_set_rf_attenuator(sddc_dev_t *dev, float value)
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

int sddc_get_rf_attenuator(sddc_dev_t *dev, float *value)
{
    if (!dev || !value)
        return -EINVAL;

    if (value)
        *value = dev->rf_attenuator;

    return 0;
}

int sddc_set_if_gain(sddc_dev_t *dev, float value)
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

int sddc_get_if_gain(sddc_dev_t *dev, float *value)
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

    return dev->target_sample_rate;
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
            buf_num = 64;

        if (buf_len == 0)
            buf_len = 16384;

        // initialize the hardware for raw mode
        dev->hardware->UpdateAdcFreq(dev->target_sample_rate * 2);
        dev->hardware->FX3producerOn(); // FX3 start the producer

        ringbuffer<int16_t> inputbuffer(buf_num);

        inputbuffer.setBlockSize(buf_len);
        inputbuffer.Start();

        dev->hardware->StartStream(inputbuffer, QUEUE_SIZE);

        uint32_t len = inputbuffer.getBlockSize();
        // read the data
        while (dev->status == DEVICE_RUNNING)
        {
            auto ptr = inputbuffer.getReadPtr();
            cb((unsigned char *)ptr, len, ctx);
            inputbuffer.ReadDone();
        }
        inputbuffer.Stop();

        dev->hardware->StopStream();
        dev->hardware->FX3producerOff(); // FX3 stop the producer
    }
    else
    {
        if (buf_num == 0)
            buf_num = 64;

        if (buf_len == 0)
            buf_len = 16384;

        ringbuffer<int16_t> inputbuffer(buf_num);
        ringbuffer<float> outputbuffer(buf_num);

        outputbuffer.setBlockSize(buf_len * 2 * sizeof(float));

        dev->hardware->UpdateAdcFreq(DEFAULT_ADC_FREQ);
        dev->r2iq->Init(dev->hardware->getGain(), &inputbuffer, &outputbuffer);

        // determine decimation index
        int samplerateidx;
        switch(dev->target_sample_rate)
        {
            case 32000000:
                samplerateidx = 1;
                break;
            case 16000000:
                samplerateidx = 2;
                break;
            case 8000000:
                samplerateidx = 3;
                break;
            case 4000000:
                samplerateidx = 4;
                break;
            case 2000000:
                samplerateidx = 5;
                break;
            default:
                samplerateidx = 5; // 2Msps with further decimation
                // TODO: support more flexible sample rates
                break;

        }

        dev->r2iq->setDecimate(samplerateidx); // no decimation
        dev->r2iq->setSideband(dev->direct_sampling == 1); // LSB for tuner mode, USB for direct sampling mode
        dev->r2iq->TurnOn();

        dev->hardware->FX3producerOn(); // FX3 start the producer
        dev->hardware->StartStream(inputbuffer, QUEUE_SIZE);

    	auto len = outputbuffer.getBlockSize() / 2 / sizeof(float);

        while (dev->status == DEVICE_RUNNING)
        {
            auto buf = outputbuffer.getReadPtr();

            if (dev->status != DEVICE_RUNNING)
                break;

            if (dev->fc != 0.0f)
            {
                std::unique_lock<std::mutex> lk(dev->fc_mutex);
                shift_limited_unroll_C_sse_inp_c((complexf*)buf, len, &dev->stateFineTune);
            }

            cb((unsigned char *)buf, len, ctx);

            outputbuffer.ReadDone();
        }

        dev->r2iq->TurnOff();
        outputbuffer.Stop();
        inputbuffer.Stop();
        dev->hardware->StopStream();
        dev->hardware->FX3producerOff(); // FX3 stop the producer
    }

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
