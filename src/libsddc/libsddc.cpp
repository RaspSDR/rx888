#include "libsddc.h"
#include "config.h"
#include "r2iq.h"
#include "RadioHandler.h"
#include "FX3Class.h"

struct sddc_dev
{
    RadioHandlerClass* handler;
    uint8_t led;
    int samplerateidx;
    uint32_t target_sample_rate;

    // if direct sampling, we will return the data of ADC directly
    // otherwise, we will use software to down-convert to baseband
    int direct_sampling;

    sddc_read_async_cb_t callback;
    void *callback_context;
};

static void Callback(void* context, const float* data, uint32_t len)
{
    
}

class rawdata : public r2iqControlClass {
    void Init(float gain, ringbuffer<int16_t>* buffers, ringbuffer<float>* obuffers) override
    {
        idx = 0;
    }

    void TurnOn() override
    {
        this->r2iqOn = true;
        idx = 0;
    }

private:
    int idx;
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
    strcpy(manufact, "SDDC");

    return 0;
}

int sddc_get_index_by_serial(const char *serial)
{
    uint8_t idx = 0;
    char sn[256];

    while (EnumerateFx3Devices(idx, nullptr, sn))
    {
        if (strcmp(sn, serial) == 0)
            return idx;
        idx++;
    }

    return -3;
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
        return -EBADF;

    ret_val->handler = new RadioHandlerClass();

    if (ret_val->handler->Init(fx3))
    {
        ret_val->samplerateidx = 0;
    }

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

    return 0;
}

uint32_t sddc_get_sample_rate(sddc_dev_t *dev)
{
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
    return dev->direct_sampling;
}

