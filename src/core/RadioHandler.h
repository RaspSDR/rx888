#ifndef RADIOHANDLER_H
#define RADIOHANDLER_H

#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <stdint.h>
#include "FX3Class.h"
#include "radio/RadioHardware.h"
#include "dsp/ringbuffer.h"
#include "../Interface.h"

class RadioHardware;
class r2iqControlClass;

enum {
    RESULT_OK,
    RESULT_BIG_STEP,
    RESULT_TOO_HIGH,
    RESULT_TOO_LOW,
    RESULT_NOT_POSSIBLE
};

struct shift_limited_unroll_C_sse_data_s;
typedef struct shift_limited_unroll_C_sse_data_s shift_limited_unroll_C_sse_data_t;

class RadioHandlerClass {
public:
    RadioHandlerClass();
    virtual ~RadioHandlerClass();
    bool Init(fx3class* Fx3, r2iqControlClass *r2iqCntrl = nullptr);
    bool Start(int srate_idx,  void (*callback)(void* context, const float*, uint32_t), void* context = nullptr);
    bool Stop();
    bool Close();
    bool IsReady(){return true;}

    int GetRFAttSteps(const float **steps) const;
    int UpdateattRF(int attIdx);

    int GetIFGainSteps(const float **steps) const;
    int UpdateIFGain(int attIdx);

    bool UpdatemodeRF(rf_mode mode);
    rf_mode GetmodeRF() const {return (rf_mode)modeRF;}
    bool UptDither (bool b);
    bool GetDither () {return dither;}
    bool UptPga(bool b);
    bool GetPga() { return pga;}
    bool UptRand (bool b);
    bool GetRand () {return randout;}
    uint16_t GetFirmware() { return hardware->getFirmware(); }

    uint32_t getSampleRate() { return adcrate; }
    bool UpdateSampleRate(uint32_t samplerate);

    float getBps() const { return mBps; }
    float getSpsIF() const {return mSpsIF; }

    const char* getName() const;
    RadioModel getModel() { return hardware->getModel(); }

    bool GetBiasT_HF() { return biasT_HF; }
    void UpdBiasT_HF(bool flag);
    bool GetBiasT_VHF() { return biasT_VHF; }
    void UpdBiasT_VHF(bool flag);

    uint64_t TuneLO(uint64_t lo);
    rf_mode PrepareLo(uint64_t lo);

    void uptLed(int led, bool on);

    void EnableDebug(void (*dbgprintFX3)(const char* fmt, ...), bool (*getconsolein)(char* buf, int maxlen)) 
        { 
          this->DbgPrintFX3 = dbgprintFX3; 
          this->GetConsoleIn = getconsolein;
        };

    bool ReadDebugTrace(uint8_t* pdata, uint8_t len) { return hardware->ReadDebugTrace(pdata, len); }

private:
    void AdcSamplesProcess();
    void AbortXferLoop(int qidx);
    void CaculateStats();
    void OnDataPacket();
    r2iqControlClass* r2iqCntrl;

    void (*Callback)(void* context, const float *data, uint32_t length);
    void *callbackContext;
    void (*DbgPrintFX3)(const char* fmt, ...);
    bool (*GetConsoleIn)(char* buf, int maxlen);

    bool run;
    unsigned long count;    // absolute index

    bool pga;
    bool dither;
    bool randout;
    bool biasT_HF;
    bool biasT_VHF;
    rf_mode modeRF;

    // transfer variables
    ringbuffer<int16_t> inputbuffer;
    ringbuffer<float> outputbuffer;

    // threads
    std::thread show_stats_thread;
    std::thread submit_thread;

    // stats
    unsigned long BytesXferred;
    unsigned long SamplesXIF;
    float	mBps;
    float	mSpsIF;

    uint32_t adcrate;

    std::mutex fc_mutex;
    std::mutex stop_mutex;
    float fc;
    RadioHardware* hardware;
    shift_limited_unroll_C_sse_data_t* stateFineTune;
};

extern unsigned long Failures;


#endif
