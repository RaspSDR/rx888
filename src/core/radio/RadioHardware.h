#pragma once

#include "../config.h"
#include "../fx3class.h"
#include "../hardware.h"

class BBRF103Radio : public RadioHardware {
public:
    BBRF103Radio(fx3class* fx3);
    BBRF103Radio(fx3class* fx3, RadioModel model);
    const char* getName() override { return "BBRF103"; }
    float getGain() override { return BBRF103_GAINFACTOR; }
    rf_mode PrepareLo(uint64_t freq) override;
    void Initialize(uint32_t samplefreq) override;
    bool UpdatemodeRF(rf_mode mode) override;
    uint64_t TuneLo(uint64_t freq) override;
    bool UpdateattRF(int attIndex) override;
    bool UpdateGainIF(int attIndex) override;

    int getRFSteps(const float** steps ) const override;
    int getIFSteps(const float** steps ) const override;

private:
    static const int step_size = 29;
    static const float steps[step_size];
    static const float hfsteps[3];

    static const int if_step_size = 16;
    static const float if_steps[if_step_size];

    uint32_t SampleRate;
};

class RX888Radio : public BBRF103Radio {
public:
    RX888Radio(fx3class* fx3) : BBRF103Radio(fx3, RX888) {}
    const char* getName() override { return "RX888"; }
    float getGain() override { return RX888_GAINFACTOR; }
};

class RX888R2Radio : public RadioHardware {
public:
    RX888R2Radio(fx3class* fx3);
    const char* getName() override { return "RX888 mkII"; }
    float getGain() override { return RX888mk2_GAINFACTOR; }
    rf_mode PrepareLo(uint64_t freq) override;
    void Initialize(uint32_t samplefreq) override;
    bool UpdatemodeRF(rf_mode mode) override;
    uint64_t TuneLo(uint64_t freq) override;
    bool UpdateattRF(int attIndex) override;
    bool UpdateGainIF(int attIndex) override;

    int getRFSteps(const float** steps ) const override;
    int getIFSteps(const float** steps ) const override;

private:
    static const int  hf_rf_step_size = 64;
    static const int  hf_if_step_size = 127;
    static const int vhf_if_step_size = 16;
    static const int vhf_rf_step_size = 29;

    float  hf_rf_steps[hf_rf_step_size];
    float  hf_if_steps[hf_if_step_size];
    static const float vhf_rf_steps[vhf_rf_step_size];
    static const float vhf_if_steps[vhf_if_step_size];

    uint32_t SampleRate;
};

class RX888R3Radio : public RadioHardware {
public:
    RX888R3Radio(fx3class* fx3);
    const char* getName() override { return "RX888 mkIII"; }
    float getGain() override { return RX888mk2_GAINFACTOR; }
    rf_mode PrepareLo(uint64_t freq) override;
    void Initialize(uint32_t samplefreq) override;
    bool UpdatemodeRF(rf_mode mode) override;
    uint64_t TuneLo(uint64_t freq) override;
    bool UpdateattRF(int attIndex) override;
    bool UpdateGainIF(int attIndex) override;

    int getRFSteps(const float** steps ) const override;
    int getIFSteps(const float** steps ) const override;

private:
    static const int  hf_rf_step_size = 64;
    static const int  hf_if_step_size = 127;
    static const int vhf_if_step_size = 16;
    static const int vhf_rf_step_size = 29;

    float  hf_rf_steps[hf_rf_step_size];
    float  hf_if_steps[hf_if_step_size];
    static const float vhf_rf_steps[vhf_rf_step_size];
    static const float vhf_if_steps[vhf_if_step_size];

    uint32_t SampleRate;
};

class RX999Radio : public RadioHardware {
public:
    RX999Radio(fx3class* fx3);
    const char* getName() override { return "RX999"; }
    float getGain() override { return RX888_GAINFACTOR; }

    rf_mode PrepareLo(uint64_t freq) override;
    void Initialize(uint32_t samplefreq) override;
    bool UpdatemodeRF(rf_mode mode) override;
    uint64_t TuneLo(uint64_t freq) override;
    bool UpdateattRF(int attIndex) override;
    bool UpdateGainIF(int attIndex) override;

    int getRFSteps(const float** steps ) const override;
    int getIFSteps(const float** steps ) const override;

private:
    static const int if_step_size = 127;

    float  if_steps[if_step_size];
    uint32_t SampleRate;
};

class HF103Radio : public RadioHardware {
public:
    HF103Radio(fx3class* fx3);
    const char* getName() override { return "HF103"; }
    float getGain() override { return HF103_GAINFACTOR; }

    rf_mode PrepareLo(uint64_t freq) override;

    void Initialize(uint32_t samplefreq) override {};

    bool UpdatemodeRF(rf_mode mode) override;

    uint64_t TuneLo(uint64_t freq) override { return 0; }

    bool UpdateattRF(int attIndex) override;

    int getRFSteps(const float** steps ) const override;

private:
    static const int step_size = 64;
    float steps[step_size];
};

class RXLucyRadio : public RadioHardware {
public:
    RXLucyRadio(fx3class* fx3);
    const char* getName() override { return "Lucy"; }
    float getGain() override { return HF103_GAINFACTOR; }

    rf_mode PrepareLo(uint64_t freq) override;
    void Initialize(uint32_t samplefreq) override;
    bool UpdatemodeRF(rf_mode mode) override;
    uint64_t TuneLo(uint64_t freq) override ;
    bool UpdateattRF(int attIndex) override;
    bool UpdateGainIF(int attIndex) override;    

    int getRFSteps(const float** steps) const override;
    int getIFSteps(const float** steps) const override;

private:
    static const int step_size = 16;
    float steps[step_size];

    static const int if_step_size = 64;
    float if_steps[if_step_size];
    uint32_t SampleRate;
};

class DummyRadio : public RadioHardware {
public:
    DummyRadio(fx3class* fx3) : RadioHardware(fx3, NORADIO) {}
    const char* getName() override { return "Dummy"; }

    rf_mode PrepareLo(uint64_t freq) override
    { return HFMODE;}
    void Initialize(uint32_t samplefreq) override {}
    bool UpdatemodeRF(rf_mode mode) override { return true; }
    bool UpdateattRF(int attIndex) override { return true; }
    uint64_t TuneLo(uint64_t freq) override {
        if (freq < 64000000 /2)
            return 0;
        else
            return freq;
     }
};
