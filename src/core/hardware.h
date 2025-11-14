#pragma once

#include "config.h"
#include "fx3class.h"

class RadioHardware
{
public:
    RadioHardware(fx3class *fx3, RadioModel model) : Fx3(fx3), gpios(0), model(model) {}

    virtual ~RadioHardware();
    virtual const char *getName() = 0;
    virtual rf_mode PrepareLo(uint64_t freq) = 0;
    virtual float getGain() { return BBRF103_GAINFACTOR; }
    virtual void UpdateAdcFreq(uint32_t samplefreq) {}
    virtual bool UpdatemodeRF(rf_mode mode) = 0;
    virtual uint64_t TuneLo(uint64_t freq) = 0;

    virtual int getRFSteps(const float **steps) const
    {
        (void)steps;
        return 0;
    }
    virtual int getIFSteps(const float **steps) const
    {
        (void)steps;
        return 0;
    }
    virtual bool UpdateGainIF(int attIndex)
    {
        (void)attIndex;
        return false;
    }
    virtual bool UpdateattRF(int attIndex)
    {
        (void)attIndex;
        return false;
    }

    RadioModel getModel() const { return model; }

    uint16_t getFirmware() const
    {
        uint8_t rdata[4];
        Fx3->GetHardwareInfo((uint32_t *)rdata);
        return (rdata[1] << 8) + rdata[2];
    }

    bool FX3producerOn() { return Fx3->Control(STARTFX3); }
    bool FX3producerOff() { return Fx3->Control(STOPFX3); }

    void StartStream(ringbuffer<int16_t> &input, int numofblock)
    {
        Fx3->StartStream(input, numofblock);
    }

    void StopStream()
    {
        Fx3->StopStream();
    }

    bool ReadDebugTrace(uint8_t *pdata, uint8_t len) { return Fx3->ReadDebugTrace(pdata, len); }

    bool FX3SetGPIO(uint32_t mask);
    bool FX3UnsetGPIO(uint32_t mask);

protected:
    fx3class *Fx3;
    uint32_t gpios;
    RadioModel model;
};

extern RadioHardware *CreateRadioHardwareInstance(fx3class *fx3);