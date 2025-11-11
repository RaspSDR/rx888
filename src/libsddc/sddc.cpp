#include <assert.h>

#include "sddc.h"
#include "config.h"
#include "r2iq.h"
#include "RadioHandler.h"
#include "FX3Class.h"

enum DeviceStatus {
    DEVICE_IDLE = 0,
    DEVICE_RUNNING,
    DEVICE_CANCELING,
    DEVICE_ERROR
};

struct sddc_dev
{
    RadioHandlerClass* handler;
    uint8_t led;
    int samplerateidx;
    uint32_t target_sample_rate;

    bool raw_mode;

    // if direct sampling, we will return the data of ADC directly
    // otherwise, we will use software to down-convert to baseband
    int direct_sampling;

    sddc_read_async_cb_t callback;
    void *callback_context;

    DeviceStatus status;
};

static void Callback(void* context, const float* data, uint32_t len)
{
    struct sddc_dev* dev = (struct sddc_dev*)context;

    assert(!dev->raw_mode);

    if (dev->callback) {
        dev->callback((unsigned char*)data, len * 2 * sizeof(float), dev->callback_context);
    }
}

class rawdata : public r2iqControlClass
{
public:
    rawdata() : r2iqControlClass()
    {
    }

    void Init(float gain, ringbuffer<int16_t> *buffers, ringbuffer<float> *obuffers) override
    {
    }

    void TurnOn() override
    {
        this->r2iqOn = true;
    }

    void TurnOff(void) override
    {
        this->r2iqOn = false;
    }

private:
    std::thread r2iq_thread;

    void *r2iqThreadf(r2iqThreadArg *th)
    {

    }
};

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

const char* sddc_get_device_name(uint32_t index)
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
        return -EBADF;
    }

    ret_val->handler = new RadioHandlerClass();

    if (ret_val->handler->Init(fx3))
    {
        ret_val->samplerateidx = 0;
    }

    *dev = ret_val;

    return 0;
}

int sddc_open_raw(sddc_dev_t** dev, uint32_t index)
{
    auto ret_val = new sddc_dev();

    fx3class* fx3 = CreateUsbHandler(index);
    if (fx3 == nullptr)
    {
        return -EBADF;
    }

    bool openOK = fx3->Open();
    if (!openOK) {
        delete fx3;
        return -EBADF;
    }

    ret_val->handler = new RadioHandlerClass();

    if (ret_val->handler->Init(fx3, new rawdata()))
    {
        ret_val->samplerateidx = 0;
    }

    ret_val->raw_mode = true;

    *dev = ret_val;

    return 0;
}

int sddc_close(sddc_dev_t *dev)
{
    if (!dev)
        return -EINVAL;

    if (dev->handler)
        delete dev->handler;

    delete dev;

    return 0;
}

int sddc_set_bias_tee(sddc_dev_t *dev, int on)
{
    if (!dev)
        return -EINVAL;

    if (on & 0x01)
        dev->handler->UpdBiasT_HF(true);
    else    
        dev->handler->UpdBiasT_HF(false);
        
    if (on & 0x02)
        dev->handler->UpdBiasT_VHF(true);
    else    
        dev->handler->UpdBiasT_VHF(false);

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
        dev->handler->UpdateSampleRate(rate * 2);
        dev->samplerateidx = 0; // not used
    }
    else
    {
        dev->target_sample_rate = 0;
        switch((int64_t)rate)
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

    //TODO: Is this correct
    return dev->handler->TuneLO(0);
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

    dev->handler->TuneLO(freq);

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
    dev->handler->Start(dev->samplerateidx, Callback, dev);

    return 0;
}


int sddc_cancel_async(sddc_dev_t *dev)
{
    if (dev->status != DEVICE_RUNNING)
        return -EBUSY;

    dev->status = DEVICE_CANCELING;
    dev->handler->Stop();
    dev->status = DEVICE_IDLE;

    return 0;
}